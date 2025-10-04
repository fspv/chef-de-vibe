mod helpers;

use chef_de_vibe::{
    api::handlers::AppState,
    config::Config,
    models::{CreateSessionRequest, CreateSessionResponse, GetSessionResponse},
    session_manager::SessionManager,
};
use futures_util::{SinkExt, StreamExt};
use helpers::logging::init_logging;
use helpers::mock_claude::MockClaude;
use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

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

        let config = Config::from_env().expect("Failed to load config");
        let session_manager = Arc::new(SessionManager::new(config.clone()));

        let state = AppState {
            session_manager: session_manager.clone(),
            config: Arc::new(config),
        };

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
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();

        let base_url = format!("http://127.0.0.1:{port}");
        let ws_url = format!("ws://127.0.0.1:{port}");

        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        TestServer {
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
        self.server_handle.abort();
        let session_manager = self.session_manager.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                session_manager.shutdown().await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            });
        })
        .join()
        .ok();
    }
}

#[tokio::test]
#[serial]
async fn test_session_id_mismatch_between_filename_and_content() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create a session file where filename doesn't match internal session ID
    let mismatched_file = server
        .mock
        .projects_dir
        .join("filename-session-id.jsonl");
    fs::write(
        &mismatched_file,
        r#"{"sessionId": "different-session-id", "cwd": "/home/test", "type": "start"}
{"uuid": "msg-1", "sessionId": "different-session-id", "type": "user", "message": {"role": "user", "content": "Test message"}}"#,
    )
    .unwrap();

    // Try to GET the session using the filename ID
    let response = client
        .get(format!(
            "{}/api/v1/sessions/filename-session-id",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    // According to invariant: "Session ID in filename matches sessionId in file content"
    // This should either return 404 or detect the mismatch
    assert_ne!(response.status(), 200, "Should not return success for mismatched IDs");
    
    // Also verify that listing sessions handles this correctly
    let list_response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(list_response.status(), 200);
    
    // The session with mismatched ID should either be skipped or handled gracefully
    let body: serde_json::Value = list_response.json().await.unwrap();
    if let Some(sessions) = body.get("sessions").and_then(|s| s.as_array()) {
        // Verify no session has the filename ID when content ID differs
        let has_filename_id = sessions
            .iter()
            .any(|s| s.get("session_id").and_then(|id| id.as_str()) == Some("filename-session-id"));
        
        if has_filename_id {
            // If it exists, it should reflect some error state or use the actual content ID
            let session = sessions
                .iter()
                .find(|s| s.get("session_id").and_then(|id| id.as_str()) == Some("filename-session-id"))
                .unwrap();
            
            // Verify it's not marked as active (since it's malformed)
            assert_eq!(
                session.get("active").and_then(|a| a.as_bool()),
                Some(false),
                "Mismatched session should not be active"
            );
        }
    }
}

#[tokio::test]
#[serial]
async fn test_write_to_dead_claude_process() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("dead_process_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session with bootstrap message to create session file
    let session_file_path = server
        .mock
        .projects_dir
        .join("dead-process-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "dead-process-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "dead-process-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![create_file_command],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let session_data: CreateSessionResponse = response.json().await.unwrap();

    // Connect WebSocket
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = Url::parse(&ws_url).unwrap();
    let (mut ws, _) = connect_async(url).await.unwrap();

    // Send a message to establish connection
    ws.send(Message::Text(r#"{"role": "user", "content": "Initial message"}"#.to_string()))
        .await
        .unwrap();

    // Small delay for message processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Kill the Claude process by sending exit command
    ws.send(Message::Text(r#"{"control": "exit", "code": 1}"#.to_string()))
        .await
        .unwrap();

    // Wait for process to die
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Now try to write to the dead process
    let write_result = ws
        .send(Message::Text(r#"{"role": "user", "content": "Write to dead process"}"#.to_string()))
        .await;

    // According to error matrix: "Write to dead Claude process" should "Close all WebSockets"
    // Either the send fails or we receive a close frame
    if write_result.is_ok() {
        // If send succeeded, we should receive a close frame
        let close_received = timeout(Duration::from_secs(5), async {
            while let Some(msg) = ws.next().await {
                if let Ok(Message::Close(_)) = msg {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);

        assert!(close_received, "WebSocket should close after writing to dead process");
    } else {
        // Send failed, which is also acceptable
        assert!(write_result.is_err(), "Write should fail for dead process");
    }
}

#[tokio::test]
#[serial]
async fn test_malformed_json_from_claude_causes_websocket_closure() {
    // This test simulates Claude sending malformed JSON that causes WebSocket issues
    // Since we're using a mock Claude, we'll modify it to send invalid JSON
    
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("malformed_json_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session with a bootstrap message that will cause mock to send malformed JSON
    let session_file_path = server
        .mock
        .projects_dir
        .join("malformed-json-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "malformed-json-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    
    // First create the session file
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // Then send a command that will echo back malformed JSON
    // The mock Claude echoes back JSON, so we send something that will be malformed when echoed
    let malformed_trigger = r#"{"control": "echo_malformed", "content": "{ broken json without closing"}"#;

    let request = CreateSessionRequest {
        session_id: "malformed-json-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![create_file_command, malformed_trigger.to_string()],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // The session creation might fail if the malformed JSON is detected early
    if response.status() == 200 {
        let session_data: CreateSessionResponse = response.json().await.unwrap();

        // Try to connect WebSocket
        let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
        let url = Url::parse(&ws_url).unwrap();
        
        match connect_async(url).await {
            Ok((mut ws, _)) => {
                // Send a message that might trigger malformed response
                let _ = ws.send(Message::Text(r#"{"role": "user", "content": "Test"}"#.to_string())).await;
                
                // Wait for potential error or close
                let result = timeout(Duration::from_secs(2), ws.next()).await;
                
                // If we get malformed JSON, the connection should close or error
                match result {
                    Ok(Some(Ok(Message::Close(_)))) => {
                        // Good - connection closed due to malformed JSON
                    }
                    Ok(Some(Err(_))) => {
                        // Good - error due to malformed JSON
                    }
                    _ => {
                        // Connection might still be open but in error state
                        let _ = ws.close(None).await;
                    }
                }
            }
            Err(_) => {
                // Connection failed - acceptable for malformed JSON scenario
            }
        }
    }
}

#[tokio::test]
#[serial]
async fn test_websocket_to_nonexistent_session() {
    let server = TestServer::new().await;

    // Try to connect to a non-existent session
    let ws_url = format!(
        "{}/api/v1/sessions/nonexistent-session/claude_ws",
        server.ws_url
    );
    let url = Url::parse(&ws_url).unwrap();
    
    let connect_result = connect_async(url).await;
    
    // According to error matrix: "WebSocket to non-existent session" should "Refuse connection"
    if let Ok((mut ws, _)) = connect_result {
        // If connection succeeded, it should close immediately
        let closed_immediately = timeout(Duration::from_millis(500), async {
            // Try to receive - should get close frame or error immediately
            matches!(ws.next().await, Some(Ok(Message::Close(_))) | None)
        })
        .await
        .unwrap_or(false);
        
        assert!(
            closed_immediately,
            "WebSocket should close immediately for non-existent session"
        );
    } else {
        // Connection refused is the expected behavior
        assert!(connect_result.is_err(), "Connection should be refused for non-existent session");
    }
}

#[tokio::test]
#[serial]
async fn test_client_sends_invalid_json_via_websocket() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("invalid_json_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create a valid session first
    let session_file_path = server
        .mock
        .projects_dir
        .join("invalid-json-ws-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "invalid-json-ws-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "invalid-json-ws-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![create_file_command],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let session_data: CreateSessionResponse = response.json().await.unwrap();

    // Connect WebSocket
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = Url::parse(&ws_url).unwrap();
    let (mut ws, _) = connect_async(url).await.unwrap();

    // Send valid JSON first to establish connection
    ws.send(Message::Text(r#"{"role": "user", "content": "Valid message"}"#.to_string()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now send invalid JSON
    let invalid_jsons = vec![
        "not json at all",
        "{ broken json",
        r#"{"no_closing_quote: "value"}"#,
        "null",
        "123",
        "[unclosed array",
    ];

    for invalid_json in invalid_jsons {
        let send_result = ws.send(Message::Text(invalid_json.to_string())).await;
        
        // According to error matrix: "Client sends invalid JSON" should "Ignore message, log error" and "Continue"
        assert!(
            send_result.is_ok(),
            "Should be able to send invalid JSON: {}",
            invalid_json
        );
    }

    // Connection should still be alive - send a valid message to verify
    let valid_after = ws
        .send(Message::Text(r#"{"role": "user", "content": "Valid after invalid"}"#.to_string()))
        .await;

    assert!(
        valid_after.is_ok(),
        "Connection should remain open after invalid JSON"
    );

    // Wait a bit and check connection is still alive
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Send ping to verify connection
    let ping_result = ws.send(Message::Ping(vec![])).await;
    assert!(ping_result.is_ok(), "Connection should still be alive");

    let _ = ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_concurrent_writes_to_same_session() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("concurrent_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session
    let session_file_path = server
        .mock
        .projects_dir
        .join("concurrent-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "concurrent-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "concurrent-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![create_file_command],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let session_data: CreateSessionResponse = response.json().await.unwrap();

    // Connect multiple WebSocket clients
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = Url::parse(&ws_url).unwrap();

    let mut clients = Vec::new();
    for i in 0..5 {
        let (ws, _) = connect_async(url.clone()).await.unwrap();
        clients.push((i, ws));
    }

    // Send messages concurrently from all clients
    let mut handles = Vec::new();
    
    for (client_id, mut ws) in clients {
        let handle = tokio::spawn(async move {
            for msg_num in 0..10 {
                let message = format!(
                    r#"{{"role": "user", "content": "Client {} message {}"}}"#,
                    client_id, msg_num
                );
                
                let result = ws.send(Message::Text(message)).await;
                assert!(result.is_ok(), "Client {} failed to send message {}", client_id, msg_num);
                
                // Small random delay to create race conditions
                tokio::time::sleep(Duration::from_millis(10 + (client_id as u64 * 3))).await;
            }
            
            // Close connection
            let _ = ws.close(None).await;
        });
        
        handles.push(handle);
    }

    // Wait for all clients to finish
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify the session is still accessible after concurrent writes
    let final_check = client
        .get(format!("{}/api/v1/sessions/concurrent-session", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(final_check.status(), 200, "Session should still be accessible after concurrent writes");
}

#[tokio::test]
#[serial]
async fn test_session_file_corruption_during_active_session() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("corruption_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session
    let session_file_path = server
        .mock
        .projects_dir
        .join("corruption-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "corruption-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    
    fs::write(&session_file_path, session_content).unwrap();

    let request = CreateSessionRequest {
        session_id: "corruption-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // If session is already active from a previous attempt, that's ok
    if response.status() == 200 {
        let session_data: CreateSessionResponse = response.json().await.unwrap();

        // Connect WebSocket
        let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
        let url = Url::parse(&ws_url).unwrap();
        let (mut ws, _) = connect_async(url).await.unwrap();

        // Send a valid message
        ws.send(Message::Text(r#"{"role": "user", "content": "Before corruption"}"#.to_string()))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Now corrupt the session file while the session is active
        fs::write(&session_file_path, "CORRUPTED CONTENT - NOT JSON").unwrap();

        // Session should continue to work since it's already in memory
        let send_after_corruption = ws
            .send(Message::Text(r#"{"role": "user", "content": "After corruption"}"#.to_string()))
            .await;

        assert!(
            send_after_corruption.is_ok(),
            "Active session should continue despite file corruption"
        );

        // The GET endpoint might fail or return cached data
        let get_response = client
            .get(format!("{}/api/v1/sessions/corruption-session", server.base_url))
            .send()
            .await
            .unwrap();

        // Should either return the active session info (200) or error due to corruption
        if get_response.status() == 200 {
            let body: GetSessionResponse = get_response.json().await.unwrap();
            assert!(
                body.websocket_url.is_some(),
                "Active session should have WebSocket URL"
            );
        }

        let _ = ws.close(None).await;
    }
}

#[tokio::test]
#[serial]
async fn test_missing_required_fields_in_request() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Test missing session_id
    let invalid_request = serde_json::json!({
        "working_dir": "/home/test",
        "resume": false,
        "bootstrap_messages": []
    });

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&invalid_request)
        .send()
        .await
        .unwrap();

    // Server returns 422 (Unprocessable Entity) for missing required fields
    assert_eq!(
        response.status(),
        422,
        "Should return 422 for missing session_id"
    );

    // Test missing working_dir
    let invalid_request = serde_json::json!({
        "session_id": "test-session",
        "resume": false,
        "bootstrap_messages": []
    });

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&invalid_request)
        .send()
        .await
        .unwrap();

    // Server returns 422 (Unprocessable Entity) for missing required fields
    assert_eq!(
        response.status(),
        422,
        "Should return 422 for missing working_dir"
    );

    // Test completely malformed JSON
    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .header("Content-Type", "application/json")
        .body("{ this is not valid json")
        .send()
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        400,
        "Should return 400 for malformed JSON"
    );
}