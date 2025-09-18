mod helpers;

use chef_de_vibe::{
    api::handlers::AppState,
    config::Config,
    models::{CreateSessionRequest, CreateSessionResponse},
    session_manager::SessionManager,
};
use futures_util::{SinkExt, StreamExt};
use helpers::logging::init_logging;
use helpers::mock_claude::MockClaude;
use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

fn generate_unique_session_id(test_name: &str) -> String {
    format!(
        "{}-{}-{}",
        test_name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    )
}

struct TestServer {
    pub base_url: String,
    pub ws_url: String,
    pub mock: MockClaude,
    server_handle: tokio::task::JoinHandle<()>,
    session_manager: Arc<SessionManager>,
}

impl TestServer {
    async fn new() -> Self {
        init_logging();
        let mock = MockClaude::new();
        mock.setup_env_vars();
        Self::new_internal(mock).await
    }

    async fn new_with_approval_binary() -> Self {
        init_logging();
        // For the new design, approval binary is the same as regular binary
        // Tests will send approval requests and responses as needed
        let mock = MockClaude::new();
        mock.setup_env_vars();
        Self::new_internal(mock).await
    }

    async fn new_internal(mock: MockClaude) -> Self {
        let config = Config::from_env().expect("Failed to load config");
        let session_manager = Arc::new(SessionManager::new(config.clone()));
        let state = AppState {
            session_manager: session_manager.clone(),
            config: Arc::new(config),
        };

        // Build router
        let app = axum::Router::new()
            .route(
                "/api/v1/sessions",
                axum::routing::get(chef_de_vibe::api::handlers::list_sessions),
            )
            .route(
                "/api/v1/sessions",
                axum::routing::post(chef_de_vibe::api::handlers::create_session),
            )
            .route(
                "/api/v1/sessions/:id",
                axum::routing::get(chef_de_vibe::api::handlers::get_session),
            )
            .route(
                "/api/v1/sessions/:id/claude_ws",
                axum::routing::get(chef_de_vibe::api::websocket::websocket_handler),
            )
            .route(
                "/api/v1/sessions/:id/claude_approvals_ws",
                axum::routing::get(chef_de_vibe::api::websocket::approval_websocket_handler),
            )
            .with_state(state);

        // Find a free port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        let base_url = format!("http://127.0.0.1:{}", port);
        let ws_url = format!("ws://127.0.0.1:{}", port);

        // Spawn server
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give server time to start - increase for better test isolation
        tokio::time::sleep(Duration::from_millis(500)).await;

        Self {
            base_url,
            ws_url,
            mock,
            server_handle,
            session_manager,
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // First abort the server to stop accepting new connections
        self.server_handle.abort();

        // Use thread-based cleanup to avoid runtime nesting issues
        let session_manager = self.session_manager.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                // Give more time for ongoing operations to complete
                tokio::time::sleep(Duration::from_millis(200)).await;

                // Shutdown all sessions
                session_manager.shutdown().await;

                // Additional time for WebSocket connections and processes to clean up
                tokio::time::sleep(Duration::from_millis(300)).await;
            });
        });
    }
}

// Helper functions
async fn connect_approval_websocket(
    ws_url: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Box<dyn std::error::Error>,
> {
    let url = Url::parse(ws_url)?;
    let (ws_stream, _) = connect_async(url).await?;
    Ok(ws_stream)
}

async fn expect_approval_request(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected_tool_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let response = timeout(Duration::from_secs(5), ws.next())
        .await?
        .ok_or("WebSocket stream ended unexpectedly")?;
    if let Ok(Message::Text(text)) = response {
        let parsed: serde_json::Value = serde_json::from_str(&text)?;

        // New simplified format - no "type" field needed since approval WebSocket only handles approval requests
        if parsed.get("id").is_some() && parsed.get("request").is_some() {
            let tool_name = parsed["request"]["tool_name"].as_str().unwrap_or("");
            if tool_name == expected_tool_name {
                if let Some(request_id) = parsed["id"].as_str() {
                    return Ok(request_id.to_string());
                } else {
                    return Err("No request_id found in approval_request".into());
                }
            } else {
                return Err(format!(
                    "Expected tool_name '{}', got '{}'",
                    expected_tool_name, tool_name
                )
                .into());
            }
        } else {
            return Err(format!(
                "Expected approval request with id and request fields, got message: {:?}",
                parsed
            )
            .into());
        }
    }

    Err("Expected text message with approval request".into())
}

async fn send_approval_response(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    request_id: &str,
    decision: &str,
    updated_input: Option<serde_json::Value>,
    updated_permissions: Option<Vec<serde_json::Value>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use the new format with nested response object
    let response_obj = if decision == "allow" {
        let mut obj = serde_json::json!({
            "behavior": "allow",
            "updatedInput": updated_input.unwrap_or_else(|| serde_json::json!({}))
        });

        if let Some(permissions) = updated_permissions {
            obj["updatedPermissions"] = serde_json::Value::Array(permissions);
        }

        obj
    } else {
        serde_json::json!({
            "behavior": "deny",
            "message": "Tool usage denied by user"
        })
    };

    let response = serde_json::json!({
        "id": request_id,
        "response": response_obj
    });

    let response_text = serde_json::to_string(&response)?;
    ws.send(Message::Text(response_text)).await?;

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_approval_websocket_basic_connection() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("approval_basic_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = generate_unique_session_id("approval-basic");

    // First message needs to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{}.jsonl", session_id));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Create the control command to write the session file
    let first_message = vec![format!(
        r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
        session_file_path.display(),
        escaped_content
    )];

    let request = CreateSessionRequest {
        session_id: session_id.clone(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // Debug: Check response status and headers
    let status = create_response.status();
    let headers = create_response.headers().clone();
    println!("Response status: {}", status);
    println!("Response headers: {:?}", headers);

    // Get the response body as text first
    let response_text = create_response.text().await.unwrap();
    println!("Raw response body: {}", response_text);

    assert_eq!(status, 200);

    // Try to parse as JSON to see the structure
    match serde_json::from_str::<serde_json::Value>(&response_text) {
        Ok(json_value) => {
            println!("Parsed JSON structure: {:#}", json_value);
        }
        Err(e) => {
            println!("Failed to parse as JSON: {}", e);
        }
    }

    // Try to parse the response into the expected structure
    let session_data: CreateSessionResponse = match serde_json::from_str(&response_text) {
        Ok(data) => {
            println!("Successfully parsed as CreateSessionResponse");
            data
        }
        Err(e) => {
            println!("Failed to parse response as CreateSessionResponse: {}", e);
            println!("Expected fields for CreateSessionResponse (add these if missing from your struct definition):");
            panic!("Response parsing failed - check the debug output above");
        }
    };

    // Connect to approval WebSocket
    let approval_ws_url = format!("{}{}", server.ws_url, session_data.approval_websocket_url);
    let approval_ws_result = connect_approval_websocket(&approval_ws_url).await;
    assert!(
        approval_ws_result.is_ok(),
        "Should be able to connect to approval WebSocket"
    );

    let mut approval_ws = approval_ws_result.unwrap();

    // Should not receive any initial pending approvals (empty session)
    let timeout_result = timeout(Duration::from_millis(500), approval_ws.next()).await;
    match timeout_result {
        Err(_) => {
            // Timeout is expected - no pending approvals
        }
        Ok(Some(Ok(Message::Text(text)))) => {
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
            if parsed["type"] == "pending_approvals" {
                let requests = parsed["requests"].as_array().unwrap();
                assert_eq!(
                    requests.len(),
                    0,
                    "Should have no pending approvals initially"
                );
            }
        }
        _ => {
            // Other outcomes are acceptable
        }
    }

    let _ = approval_ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_approval_websocket_connection_to_nonexistent_session() {
    let server = TestServer::new().await;

    // Try to connect to approval WebSocket for non-existent session
    let approval_ws_url = format!(
        "{}/api/v1/sessions/nonexistent-session/claude_approvals_ws",
        server.ws_url
    );
    let result = connect_approval_websocket(&approval_ws_url).await;

    match result {
        Err(_) => {
            // Connection failed as expected
        }
        Ok(mut ws) => {
            // Connection succeeded, but server should close it immediately
            let close_result = timeout(Duration::from_secs(2), ws.next()).await;
            match close_result {
                Ok(Some(Ok(Message::Close(_)))) => {
                    // Server closed connection as expected
                }
                Ok(None) => {
                    // WebSocket stream ended, which is also expected
                }
                Ok(Some(Ok(other))) => {
                    panic!("Expected close message or end of stream, got: {:?}", other);
                }
                Ok(Some(Err(e))) => {
                    // WebSocket error, which is acceptable for this test
                    println!("WebSocket error (expected): {:?}", e);
                }
                Err(_) => {
                    panic!("Expected server to close connection for nonexistent session");
                }
            }
            let _ = ws.close(None).await;
        }
    }
}

#[tokio::test]
#[serial]
async fn test_single_tool_approval_allow_flow() {
    let server = TestServer::new_with_approval_binary().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("approval_allow_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = generate_unique_session_id("approval-allow");

    // First message needs to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{}.jsonl", session_id));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Create the control command to write the session file
    let first_message = vec![format!(
        r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
        session_file_path.display(),
        escaped_content
    )];

    let request = CreateSessionRequest {
        session_id: session_id.clone(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect to approval WebSocket
    let approval_ws_url = format!("{}{}", server.ws_url, session_data.approval_websocket_url);
    let mut approval_ws = connect_approval_websocket(&approval_ws_url).await.unwrap();

    // Connect to main WebSocket and trigger a tool request
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = tokio_tungstenite::tungstenite::http::Uri::try_from(ws_url).unwrap();
    let (mut main_ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();

    // Consume any initial messages (session start, first_message response, etc.)
    tokio::time::sleep(Duration::from_millis(200)).await;
    while let Ok(Some(_)) = timeout(Duration::from_millis(100), main_ws.next()).await {
        // Drain all pending messages
    }

    // Send a control request for tool approval
    let control_request = r#"{"type": "control_request", "request_id": "test-123", "request": {"subtype": "can_use_tool", "tool_name": "Read"}}"#;
    main_ws
        .send(Message::Text(control_request.to_string()))
        .await
        .unwrap();

    // The system will process the echoed control_request and generate an approval request
    // Should receive approval request on approval WebSocket
    let request_id = expect_approval_request(&mut approval_ws, "Read")
        .await
        .unwrap();

    // Send allow response
    send_approval_response(&mut approval_ws, &request_id, "allow", None, None)
        .await
        .unwrap();

    // Send the expected assistant response
    let assistant_response = serde_json::json!({
        "type": "assistant",
        "message": {
            "content": [{
                "text": "APPROVAL_TEST_SUCCESS_READ"
            }]
        }
    });
    main_ws
        .send(Message::Text(
            serde_json::to_string(&assistant_response).unwrap(),
        ))
        .await
        .unwrap();

    // First consume the automatic control_response generated by the system
    let control_response_msg = timeout(Duration::from_secs(2), main_ws.next())
        .await
        .expect("Should receive control_response")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid message");

    // Verify it's a control_response
    if let Message::Text(text) = control_response_msg {
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            parsed.get("type").and_then(|v| v.as_str()),
            Some("control_response")
        );
    }

    // Now should receive the echoed assistant message
    let response = timeout(Duration::from_secs(5), main_ws.next())
        .await
        .expect("Should receive response within timeout")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid WebSocket message");

    match response {
        Message::Text(text) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&text).expect("Response should be valid JSON");

            // Verify this is an assistant message
            assert_eq!(
                parsed.get("type").and_then(|v| v.as_str()),
                Some("assistant"),
                "Expected assistant message, got: {}",
                text
            );

            // Extract the message content
            let message = parsed
                .get("message")
                .expect("Response should have 'message' field");
            let content = message
                .get("content")
                .expect("Message should have 'content' field");
            let content_array = content.as_array().expect("Content should be an array");
            let first_content = content_array
                .first()
                .expect("Content array should not be empty");
            let text_content = first_content
                .get("text")
                .and_then(|v| v.as_str())
                .expect("First content item should have 'text' field");

            assert_eq!(
                text_content, "APPROVAL_TEST_SUCCESS_READ",
                "Response should indicate Read tool approval was successful. Got: '{}'",
                text_content
            );
        }
        _ => panic!("Expected text message, got: {:?}", response),
    }

    let _ = approval_ws.close(None).await;
    let _ = main_ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_single_tool_approval_deny_flow() {
    let server = TestServer::new_with_approval_binary().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("approval_deny_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = generate_unique_session_id("approval-deny");

    // First message needs to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{}.jsonl", session_id));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Create the control command to write the session file
    let first_message = vec![format!(
        r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
        session_file_path.display(),
        escaped_content
    )];

    let request = CreateSessionRequest {
        session_id: session_id.clone(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect to approval WebSocket
    let approval_ws_url = format!("{}{}", server.ws_url, session_data.approval_websocket_url);
    let mut approval_ws = connect_approval_websocket(&approval_ws_url).await.unwrap();

    // Connect to main WebSocket and trigger a tool request
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = tokio_tungstenite::tungstenite::http::Uri::try_from(ws_url).unwrap();
    let (mut main_ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();

    // Consume any initial messages (session start, first_message response, etc.)
    tokio::time::sleep(Duration::from_millis(200)).await;
    while let Ok(Some(_)) = timeout(Duration::from_millis(100), main_ws.next()).await {
        // Drain all pending messages
    }

    // Send a control request for tool approval
    let control_request = r#"{"type": "control_request", "request_id": "test-bash-123", "request": {"subtype": "can_use_tool", "tool_name": "Bash"}}"#;
    main_ws
        .send(Message::Text(control_request.to_string()))
        .await
        .unwrap();

    // The system will process the echoed control_request and generate an approval request
    // Should receive approval request on approval WebSocket
    let request_id = expect_approval_request(&mut approval_ws, "Bash")
        .await
        .unwrap();

    // Send deny response
    send_approval_response(&mut approval_ws, &request_id, "deny", None, None)
        .await
        .unwrap();

    // Send the expected assistant response for denial
    let assistant_response = serde_json::json!({
        "type": "assistant",
        "message": {
            "content": [{
                "text": "APPROVAL_TEST_DENIED_BASH"
            }]
        }
    });
    main_ws
        .send(Message::Text(
            serde_json::to_string(&assistant_response).unwrap(),
        ))
        .await
        .unwrap();

    // First consume the automatic control_response generated by the system
    let control_response_msg = timeout(Duration::from_secs(2), main_ws.next())
        .await
        .expect("Should receive control_response")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid message");

    // Verify it's a control_response
    if let Message::Text(text) = control_response_msg {
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            parsed.get("type").and_then(|v| v.as_str()),
            Some("control_response")
        );
    }

    // Now should receive the echoed assistant message
    let response = timeout(Duration::from_secs(5), main_ws.next())
        .await
        .expect("Should receive response within timeout")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid WebSocket message");

    match response {
        Message::Text(text) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&text).expect("Response should be valid JSON");

            // Verify this is an assistant message
            assert_eq!(
                parsed.get("type").and_then(|v| v.as_str()),
                Some("assistant"),
                "Expected assistant message, got: {}",
                text
            );

            // Extract the message content
            let message = parsed
                .get("message")
                .expect("Response should have 'message' field");
            let content = message
                .get("content")
                .expect("Message should have 'content' field");
            let content_array = content.as_array().expect("Content should be an array");
            let first_content = content_array
                .first()
                .expect("Content array should not be empty");
            let text_content = first_content
                .get("text")
                .and_then(|v| v.as_str())
                .expect("First content item should have 'text' field");

            assert_eq!(
                text_content, "APPROVAL_TEST_DENIED_BASH",
                "Response should indicate Bash tool was denied. Got: '{}'",
                text_content
            );
        }
        _ => panic!("Expected text message, got: {:?}", response),
    }

    let _ = approval_ws.close(None).await;
    let _ = main_ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_debug_mock_claude_directly() {
    init_logging();
    let mock = MockClaude::new();
    let mock_binary = &mock.binary_path;

    println!("Testing mock binary directly: {}", mock_binary.display());

    let mut child = Command::new("python3")
        .arg(mock_binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mock Claude");

    let stdin = child.stdin.as_mut().unwrap();

    // Test 1: Send a control request (as if from the service) and verify it's echoed back
    let control_request = r#"{"type": "control_request", "request_id": "test-123", "request": {"subtype": "can_use_tool", "tool_name": "Read"}}"#;
    stdin.write_all(control_request.as_bytes()).unwrap();
    stdin.write_all(b"\n").unwrap();

    // Test 2: Send exit command
    stdin
        .write_all(b"{\"control\": \"exit\", \"code\": 0}\n")
        .unwrap();
    stdin.flush().unwrap();

    let output = child.wait_with_output().unwrap();

    println!(
        "Mock Claude stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    println!(
        "Mock Claude stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!("Mock Claude exit code: {:?}", output.status.code());

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    // Verify the control request was echoed back
    assert!(
        stdout_str.contains("control_request"),
        "Mock Claude should echo control_request"
    );
    assert!(
        stdout_str.contains("test-123"),
        "Mock Claude should echo request_id"
    );
    assert_eq!(output.status.code(), Some(0), "Should exit with code 0");
}

#[tokio::test]
#[serial]
async fn test_multiple_pending_approvals_accumulation() {
    let server = TestServer::new_with_approval_binary().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("approval_multiple_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = generate_unique_session_id("approval-multiple");

    // First message needs to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{}.jsonl", session_id));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Create the control command to write the session file
    let first_message = vec![format!(
        r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
        session_file_path.display(),
        escaped_content
    )];

    let request = CreateSessionRequest {
        session_id: session_id.clone(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect to approval WebSocket first
    let approval_ws_url = format!("{}{}", server.ws_url, session_data.approval_websocket_url);
    let mut approval_ws1 = connect_approval_websocket(&approval_ws_url).await.unwrap();

    // Connect to main WebSocket
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = tokio_tungstenite::tungstenite::http::Uri::try_from(ws_url).unwrap();
    let (mut main_ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();

    // Consume any initial messages (session start, first_message response, etc.)
    tokio::time::sleep(Duration::from_millis(200)).await;
    while let Ok(Some(_)) = timeout(Duration::from_millis(100), main_ws.next()).await {
        // Drain all pending messages
    }

    // Send a control request for tool approval
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    println!("Sending control request to main WebSocket");
    let control_request = r#"{"type": "control_request", "request_id": "multi-test-123", "request": {"subtype": "can_use_tool", "tool_name": "Read"}}"#;
    main_ws
        .send(Message::Text(control_request.to_string()))
        .await
        .unwrap();

    println!("Waiting for first approval client to receive approval request...");
    // First approval client should receive the request
    let response1 = timeout(Duration::from_secs(3), approval_ws1.next()).await;
    let response1 = match response1 {
        Ok(Some(Ok(Message::Text(text)))) => {
            println!("First approval client received: {}", text);
            text
        }
        Ok(Some(Ok(other))) => {
            panic!(
                "First approval client received unexpected message type: {:?}",
                other
            );
        }
        Ok(Some(Err(e))) => {
            panic!("First approval client WebSocket error: {:?}", e);
        }
        Ok(None) => {
            panic!("First approval client WebSocket closed");
        }
        Err(_) => {
            panic!("First approval client timeout - no approval request received");
        }
    };

    let request_data: serde_json::Value = serde_json::from_str(&response1).unwrap();
    assert!(
        request_data.get("id").is_some() && request_data.get("request").is_some(),
        "Should receive approval request with id and request fields"
    );
    assert_eq!(
        request_data["request"]["tool_name"], "Read",
        "Should be Read tool request"
    );

    // DON'T respond to the approval yet - let it stay pending

    // Now connect a second approval client - it should immediately receive pending approvals
    let mut approval_ws2 = connect_approval_websocket(&approval_ws_url).await.unwrap();

    let response2 = timeout(Duration::from_secs(3), approval_ws2.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match response2 {
        Message::Text(text) => {
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();

            // With new format, pending approvals are sent as individual approval messages
            if parsed.get("id").is_some() && parsed.get("request").is_some() {
                assert_eq!(
                    parsed["request"]["tool_name"], "Read",
                    "Should have request for Read tool"
                );
                assert_eq!(
                    parsed["id"], request_data["id"],
                    "Should be the same request"
                );
                println!(
                    "✓ Second client correctly received pending approval as individual message"
                );
            } else {
                panic!(
                    "Expected individual approval request message, got: {:?}",
                    parsed
                );
            }
        }
        other => panic!("Expected text message, got: {:?}", other),
    }

    // Now respond to the approval from the first client
    let request_id = request_data["id"].as_str().unwrap();
    send_approval_response(&mut approval_ws1, request_id, "allow", None, None)
        .await
        .unwrap();

    // Send the expected assistant response
    let assistant_response = serde_json::json!({
        "type": "assistant",
        "message": {
            "content": [{
                "text": "APPROVAL_TEST_SUCCESS_READ"
            }]
        }
    });
    main_ws
        .send(Message::Text(
            serde_json::to_string(&assistant_response).unwrap(),
        ))
        .await
        .unwrap();

    // First consume the automatic control_response generated by the system
    let control_response_msg = timeout(Duration::from_secs(2), main_ws.next())
        .await
        .expect("Should receive control_response")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid message");

    // Verify it's a control_response
    if let Message::Text(text) = control_response_msg {
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            parsed.get("type").and_then(|v| v.as_str()),
            Some("control_response")
        );
    }

    // Now should receive the echoed assistant message
    let response = timeout(Duration::from_secs(5), main_ws.next())
        .await
        .expect("Should receive response within timeout")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid WebSocket message");

    match response {
        Message::Text(text) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&text).expect("Response should be valid JSON");

            // Verify this is an assistant message
            assert_eq!(
                parsed.get("type").and_then(|v| v.as_str()),
                Some("assistant"),
                "Expected assistant message, got: {}",
                text
            );

            // Extract the message content
            let message = parsed
                .get("message")
                .expect("Response should have 'message' field");
            let content = message
                .get("content")
                .expect("Message should have 'content' field");
            let content_array = content.as_array().expect("Content should be an array");
            let first_content = content_array
                .first()
                .expect("Content array should not be empty");
            let text_content = first_content
                .get("text")
                .and_then(|v| v.as_str())
                .expect("First content item should have 'text' field");

            assert_eq!(
                text_content, "APPROVAL_TEST_SUCCESS_READ",
                "Response should indicate Read tool approval was successful. Got: '{}'",
                text_content
            );
            println!("✓ Tool request was properly approved and executed");
        }
        _ => panic!("Expected text message, got: {:?}", response),
    }

    let _ = approval_ws1.close(None).await;
    let _ = approval_ws2.close(None).await;
    let _ = main_ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_multiple_approval_clients_broadcast() {
    let server = TestServer::new_with_approval_binary().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("approval_broadcast_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = generate_unique_session_id("approval-broadcast");

    // First message needs to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{}.jsonl", session_id));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Create the control command to write the session file
    let first_message = vec![format!(
        r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
        session_file_path.display(),
        escaped_content
    )];

    let request = CreateSessionRequest {
        session_id: session_id.clone(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect multiple approval WebSocket clients
    let approval_ws_url = format!("{}{}", server.ws_url, session_data.approval_websocket_url);
    let mut approval_ws1 = connect_approval_websocket(&approval_ws_url).await.unwrap();
    let mut approval_ws2 = connect_approval_websocket(&approval_ws_url).await.unwrap();
    let mut approval_ws3 = connect_approval_websocket(&approval_ws_url).await.unwrap();

    // Give clients time to connect and stabilize
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Connect to main WebSocket and trigger a tool request
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = tokio_tungstenite::tungstenite::http::Uri::try_from(ws_url).unwrap();
    let (mut main_ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();

    // Consume any initial messages
    tokio::time::sleep(Duration::from_millis(200)).await;
    while let Ok(Some(_)) = timeout(Duration::from_millis(100), main_ws.next()).await {
        // Drain all pending messages
    }

    // Send a control request for tool approval
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    let control_request = r#"{"type": "control_request", "request_id": "broadcast-test-123", "request": {"subtype": "can_use_tool", "tool_name": "Read"}}"#;
    main_ws
        .send(Message::Text(control_request.to_string()))
        .await
        .unwrap();

    // All three approval clients should receive the same approval request
    let mut responses = Vec::new();
    let mut request_ids = Vec::new();

    // Collect responses from all three clients
    for (i, ws) in [&mut approval_ws1, &mut approval_ws2, &mut approval_ws3]
        .iter_mut()
        .enumerate()
    {
        let response = timeout(Duration::from_secs(3), ws.next()).await;
        match response {
            Ok(Some(Ok(Message::Text(text)))) => {
                println!("Client {} received: {}", i + 1, text);
                let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert!(
                    parsed.get("id").is_some() && parsed.get("request").is_some(),
                    "Client {} should receive approval request with id and request fields",
                    i + 1
                );
                assert_eq!(
                    parsed["request"]["tool_name"],
                    "Read",
                    "Client {} should receive Read tool request",
                    i + 1
                );

                if let Some(request_id) = parsed["id"].as_str() {
                    request_ids.push(request_id.to_string());
                }
                responses.push(parsed);
            }
            Ok(Some(Ok(other))) => {
                panic!(
                    "Client {} received unexpected message type: {:?}",
                    i + 1,
                    other
                );
            }
            Ok(Some(Err(e))) => {
                panic!("Client {} WebSocket error: {:?}", i + 1, e);
            }
            Ok(None) => {
                panic!("Client {} WebSocket closed", i + 1);
            }
            Err(_) => {
                panic!("Client {} timeout waiting for approval request", i + 1);
            }
        }
    }

    // All clients should have received the same request_id
    assert_eq!(
        request_ids.len(),
        3,
        "Should have received request_id from all 3 clients"
    );
    assert_eq!(
        request_ids[0], request_ids[1],
        "Client 1 and 2 should receive same request_id"
    );
    assert_eq!(
        request_ids[1], request_ids[2],
        "Client 2 and 3 should receive same request_id"
    );
    println!(
        "✓ All clients received the same approval request with request_id: {}",
        request_ids[0]
    );

    // Send approval response from first client only
    send_approval_response(&mut approval_ws1, &request_ids[0], "allow", None, None)
        .await
        .unwrap();

    // Send the expected assistant response
    let assistant_response = serde_json::json!({
        "type": "assistant",
        "message": {
            "content": [{
                "text": "APPROVAL_TEST_SUCCESS_READ"
            }]
        }
    });
    main_ws
        .send(Message::Text(
            serde_json::to_string(&assistant_response).unwrap(),
        ))
        .await
        .unwrap();

    // First consume the automatic control_response generated by the system
    let control_response_msg = timeout(Duration::from_secs(2), main_ws.next())
        .await
        .expect("Should receive control_response")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid message");

    // Verify it's a control_response
    if let Message::Text(text) = control_response_msg {
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            parsed.get("type").and_then(|v| v.as_str()),
            Some("control_response")
        );
    }

    // Now should receive the echoed assistant message
    let response = timeout(Duration::from_secs(5), main_ws.next())
        .await
        .expect("Should receive response within timeout")
        .expect("WebSocket stream should not end")
        .expect("Should receive valid WebSocket message");

    match response {
        Message::Text(text) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&text).expect("Response should be valid JSON");

            // Verify this is an assistant message
            assert_eq!(
                parsed.get("type").and_then(|v| v.as_str()),
                Some("assistant"),
                "Expected assistant message, got: {}",
                text
            );

            // Extract the message content
            let message = parsed
                .get("message")
                .expect("Response should have 'message' field");
            let content = message
                .get("content")
                .expect("Message should have 'content' field");
            let content_array = content.as_array().expect("Content should be an array");
            let first_content = content_array
                .first()
                .expect("Content array should not be empty");
            let text_content = first_content
                .get("text")
                .and_then(|v| v.as_str())
                .expect("First content item should have 'text' field");

            assert_eq!(
                text_content, "APPROVAL_TEST_SUCCESS_READ",
                "Response should indicate Read tool approval was successful. Got: '{}'",
                text_content
            );
            println!("✓ Tool request was properly approved and executed");
        }
        _ => panic!("Expected text message, got: {:?}", response),
    }

    let _ = approval_ws1.close(None).await;
    let _ = approval_ws2.close(None).await;
    let _ = approval_ws3.close(None).await;
    let _ = main_ws.close(None).await;
}
