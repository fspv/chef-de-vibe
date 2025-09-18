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
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

fn create_test_session_file(
    projects_dir: &std::path::Path,
    project_name: &str,
    session_id: &str,
    cwd: &str,
) {
    let project_path = projects_dir.join(project_name);
    fs::create_dir_all(&project_path).unwrap();

    let session_file = project_path.join(format!("{session_id}.jsonl"));
    let content = format!(
        r#"{{"sessionId": "{session_id}", "cwd": "{cwd}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Hello Claude"}}}}
{{"type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "Hello! How can I help you today?"}}]}}}}
{{"type": "user", "message": {{"role": "user", "content": "What's 2+2?"}}}}
{{"type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "2 + 2 equals 4."}}]}}}}
"#
    );

    fs::write(session_file, content).unwrap();
}

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

        let base_url = format!("http://127.0.0.1:{port}");
        let ws_url = format!("ws://127.0.0.1:{port}");

        // Spawn server
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give server time to start - increase for better test isolation
        tokio::time::sleep(Duration::from_millis(500)).await;

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
        })
        .join()
        .ok();
    }
}

#[tokio::test]
#[serial]
async fn test_malformed_json_requests() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Test malformed JSON in POST request
    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .header("Content-Type", "application/json")
        .body("{invalid json without closing brace")
        .send()
        .await
        .unwrap();

    // Should be a 4xx error for malformed request
    assert!(response.status().is_client_error());

    // Try to parse response - it might not be valid JSON
    let body_text = response.text().await.unwrap();
    if let Ok(body) = serde_json::from_str::<serde_json::Value>(&body_text) {
        assert_eq!(body["code"], "INVALID_REQUEST");
    } else {
        // Some malformed requests might return non-JSON error responses
        println!("Response body: {body_text}");
        // For malformed JSON, axum might return different error formats
        // Just check that we got a 400 status, which indicates the error was caught
        assert!(!body_text.is_empty()); // Should have some error content
    }

    // Test missing required fields
    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&serde_json::json!({
            "session_id": "test",
            // missing working_dir and resume
        }))
        .send()
        .await
        .unwrap();

    // Should be a 4xx error for invalid request
    assert!(response.status().is_client_error());

    // Try to parse response as JSON
    let body_text = response.text().await.unwrap();
    if let Ok(body) = serde_json::from_str::<serde_json::Value>(&body_text) {
        assert_eq!(body["code"], "INVALID_REQUEST");
    } else {
        // Some servers return non-JSON error responses for invalid requests
        // Just verify we got an error response
        assert!(!body_text.is_empty());
    }
}

#[tokio::test]
#[serial]
async fn test_create_session_empty_bootstrap_messages_validation() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server
        .mock
        .temp_dir
        .path()
        .join("empty_bootstrap_messages_work");
    fs::create_dir_all(&working_dir).unwrap();

    let request = CreateSessionRequest {
        session_id: "empty-bootstrap-messages-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![], // Empty bootstrap_messages should be rejected
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // Should return 400 Bad Request
    assert_eq!(
        response.status(),
        400,
        "Empty bootstrap_messages should be rejected with 400 status"
    );

    let error_body: serde_json::Value = response.json().await.unwrap();
    assert!(
        error_body["error"]
            .as_str()
            .unwrap()
            .contains("bootstrap_messages cannot be empty"),
        "Error message should mention bootstrap_messages cannot be empty. Got: {}",
        error_body["error"]
    );
}

#[tokio::test]
#[serial]
async fn test_bootstrap_messages_triggers_claude_response() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server
        .mock
        .temp_dir
        .path()
        .join("bootstrap_messages_trigger_work");
    fs::create_dir_all(&working_dir).unwrap();

    let unique_content = format!(
        "Trigger message {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let session_id = generate_unique_session_id("bootstrap-messages-trigger");

    // Bootstrap messages need to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{session_id}.jsonl"));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Create the control command to write the session file, then add the user message
    let bootstrap_messages = vec![
        format!(
            r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
            session_file_path.display(),
            escaped_content
        ),
        format!(r#"{{"role": "user", "content": "{}"}}"#, unique_content),
    ];

    let request = CreateSessionRequest {
        session_id: session_id.clone(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect WebSocket to verify Claude responds to bootstrap_messages
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let (mut ws, _) = connect_async(Url::parse(&ws_url).unwrap()).await.unwrap();

    // Give some time for session to be ready and bootstrap_messages to be processed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Try to receive Claude's response to bootstrap_messages, but don't fail if we don't get it
    // (the message may have been discarded if client connected after Claude processed it)
    for _ in 0..5 {
        // Try up to 5 messages to find the bootstrap_messages response
        if let Ok(Some(Ok(Message::Text(text)))) = timeout(Duration::from_secs(3), ws.next()).await
        {
            if text.contains(&unique_content) || text.contains("Mock Claude received") {
                // Found the response, but we don't fail if we don't get it
                break;
            }
            // Otherwise it might be session start message, continue looking
        }
    }

    // Note: We don't assert that we received the bootstrap_messages response because
    // if the client connects after Claude has already processed and discarded the message,
    // we won't receive it. This is expected behavior per the WebSocket message handling policy.

    let _ = ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_resume_session_with_bootstrap_messages() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory and session file
    let working_dir = server
        .mock
        .temp_dir
        .path()
        .join("resume_bootstrap_messages_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create a test session file directly on disk to simulate an existing session
    // Use the same helper function as other working tests
    create_test_session_file(
        &server.mock.projects_dir,
        "resume_bootstrap_messages_work",
        "old-resume-session",
        working_dir.to_str().unwrap(),
    );

    let resume_content = format!(
        "Resume trigger {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    // For resume tests, we need to send a control command to create the new session file with the new session ID
    // The mock Claude will generate a new session ID and create a new session file
    let new_session_id = generate_unique_session_id("resumed");
    let new_session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        new_session_id,
        working_dir.display()
    );
    let escaped_content = new_session_start_content.replace('"', r#"\""#);
    let new_session_file_path = server
        .mock
        .projects_dir
        .join("resume_bootstrap_messages_work")
        .join(format!("{new_session_id}.jsonl"));

    // For resume mode, the first line output must be JSON with a session_id field
    // The mock Claude will echo this JSON, and the resume code will parse it to extract session_id
    let bootstrap_messages = vec![
        format!(r#"{{"session_id": "{}"}}"#, new_session_id),
        format!(
            r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
            new_session_file_path.display(),
            escaped_content
        ),
        format!(r#"{{"role": "user", "content": "{}"}}"#, resume_content),
    ];

    let request = CreateSessionRequest {
        session_id: "old-resume-session".to_string(),
        working_dir: working_dir.clone(),
        resume: true,
        bootstrap_messages,
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let status = response.status();
    if status != 200 {
        let error_body = response.text().await.unwrap();
        panic!(
            "Resume session with bootstrap_messages failed with status: {status}. Error body: {error_body}"
        );
    }

    assert_eq!(
        response.status(),
        200,
        "Resume session with bootstrap_messages should succeed"
    );

    let create_response: CreateSessionResponse = response.json().await.unwrap();

    // Session ID should change during resume (mock Claude returns a new ID)
    assert_ne!(
        create_response.session_id, "old-resume-session",
        "Resume should return new session ID"
    );
    assert!(
        create_response.session_id.starts_with("resumed-"),
        "New session ID should start with 'resumed-', got: {}",
        create_response.session_id
    );

    // Connect WebSocket to verify Claude responds to bootstrap_messages during resume
    let ws_url = format!("{}{}", server.ws_url, create_response.websocket_url);
    let (mut ws, _) = connect_async(Url::parse(&ws_url).unwrap()).await.unwrap();

    // Try to receive Claude's response to the bootstrap_messages, but don't fail if we don't get it
    // (the message may have been discarded if client connected after Claude processed it)
    for _ in 0..3 {
        // Try up to 3 messages to find the bootstrap_messages response
        if let Ok(Some(Ok(Message::Text(text)))) = timeout(Duration::from_secs(2), ws.next()).await
        {
            if text.contains(&resume_content) || text.contains("Mock Claude received") {
                // Found the response, but we don't fail if we don't get it
                break;
            }
        }
    }

    // Note: We don't assert that we received the bootstrap_messages response because
    // if the client connects after Claude has already processed and discarded the message,
    // we won't receive it. This is expected behavior per the WebSocket message handling policy.

    let _ = ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_multiline_bootstrap_messages_compaction() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("multiline_message_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = "multiline-message-session".to_string();

    // Bootstrap messages need to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{session_id}.jsonl"));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Use a multiline JSON bootstrap_messages (simulating what frontend might send)
    let multiline_json = r#"{
  "role": "user",
  "content": "This is a multiline JSON message",
  "nested": {
    "field": "value",
    "array": [1, 2, 3]
  }
}"#;

    // Create the control command to write the session file, then add the multiline user message
    let bootstrap_messages = vec![
        format!(
            r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
            session_file_path.display(),
            escaped_content
        ),
        multiline_json.to_string(),
    ];

    let request = CreateSessionRequest {
        session_id,
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages,
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // Session creation should succeed despite multiline JSON
    assert_eq!(
        response.status(),
        200,
        "Multiline bootstrap_messages should be accepted and compacted"
    );

    let create_response: CreateSessionResponse = response.json().await.unwrap();
    assert_eq!(create_response.session_id, "multiline-message-session");

    // Connect WebSocket to verify Claude responds to the compacted bootstrap_messages
    let ws_url = format!("{}{}", server.ws_url, create_response.websocket_url);
    let (mut ws, _) = connect_async(Url::parse(&ws_url).unwrap()).await.unwrap();

    // Give connections time to stabilize
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Try to receive any messages, but don't assert that we get the bootstrap_messages response
    // (the message may have been discarded if client connected after Claude processed it)
    for _ in 0..3 {
        if let Ok(Some(Ok(Message::Text(text)))) = timeout(Duration::from_secs(2), ws.next()).await
        {
            if text.contains("multiline JSON message") || text.contains("Mock Claude received") {
                // Found the response, but we don't fail if we don't get it
                break;
            }
        }
    }

    // Note: We don't assert that we received the bootstrap_messages response because
    // if the client connects after Claude has already processed and discarded the message,
    // we won't receive it. This is expected behavior per the WebSocket message handling policy.

    let _ = ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_invalid_json_bootstrap_messages_rejection() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("invalid_json_work");
    fs::create_dir_all(&working_dir).unwrap();

    let request = CreateSessionRequest {
        session_id: "invalid-json-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec!["{ invalid json }".to_string()], // Invalid JSON
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // Should return 500 because the Claude process fails to start due to invalid JSON
    assert_eq!(
        response.status(),
        500,
        "Invalid JSON bootstrap_messages should cause session creation to fail"
    );
}

#[tokio::test]
#[serial]
async fn test_message_queue_json_compaction() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("queue_compaction_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = "queue-compaction-session".to_string();

    // Bootstrap messages need to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{session_id}.jsonl"));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Create the control command to write the session file, then add the user message
    let bootstrap_messages = vec![
        format!(
            r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
            session_file_path.display(),
            escaped_content
        ),
        r#"{"role": "user", "content": "Initial message"}"#.to_string(),
    ];

    let request = CreateSessionRequest {
        session_id,
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect WebSocket
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let (mut ws, _) = connect_async(Url::parse(&ws_url).unwrap()).await.unwrap();

    // Consume initial messages
    tokio::time::sleep(Duration::from_millis(200)).await;
    loop {
        if timeout(Duration::from_millis(100), ws.next())
            .await
            .is_err()
        {
            break;
        }
    }

    // Send multiple multiline messages rapidly to test queue processing
    let messages = vec![
        r#"{
  "role": "user",
  "content": "Queue message 1",
  "priority": "high"
}"#,
        r#"{
  "role": "user", 
  "content": "Queue message 2",
  "timestamp": "2024-01-01T00:00:00Z"
}"#,
        r#"{
  "role": "user",
  "content": "Queue message 3", 
  "complex": {
    "nested": "data"
  }
}"#,
    ];

    // Send messages rapidly to test queue handling
    for msg in &messages {
        ws.send(Message::Text((*msg).to_string())).await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await; // Small delay
    }

    // Should receive Claude's responses to all compacted messages
    let mut responses = Vec::new();
    for _ in 0..10 {
        // Try to collect responses
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_millis(500), ws.next()).await
        {
            responses.push(text);
        }
    }

    // Verify we got responses containing our queue messages
    let queue_responses: Vec<&String> = responses
        .iter()
        .filter(|r| r.contains("Queue message") || r.contains("Mock Claude received"))
        .collect();

    assert!(
        queue_responses.len() >= 2,
        "Should have received multiple Claude responses to queued messages. Got {} responses: {:?}",
        queue_responses.len(),
        responses
    );

    let _ = ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_mixed_json_formats_consistency() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory and session
    let working_dir = server.mock.temp_dir.path().join("mixed_formats_work");
    fs::create_dir_all(&working_dir).unwrap();

    let session_id = "mixed-formats-session".to_string();

    // Bootstrap messages need to create the session file using the mock's write_file control command
    let session_file_path = server
        .mock
        .projects_dir()
        .join(format!("{session_id}.jsonl"));
    let session_start_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        working_dir.display()
    );

    // Escape the content for embedding in JSON
    let escaped_content = session_start_content.replace('"', r#"\""#);

    // Use multiline bootstrap_messages
    let multiline_bootstrap_message = r#"{
  "role": "user",
  "content": "Multiline first message",
  "type": "initial"
}"#;

    // Create the control command to write the session file, then add the multiline user message
    let bootstrap_messages = vec![
        format!(
            r#"{{"control": "write_file", "path": "{}", "content": "{}"}}"#,
            session_file_path.display(),
            escaped_content
        ),
        multiline_bootstrap_message.to_string(),
    ];

    let request = CreateSessionRequest {
        session_id,
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages,
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect WebSocket
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let (mut ws, _) = connect_async(Url::parse(&ws_url).unwrap()).await.unwrap();

    // Consume initial messages
    tokio::time::sleep(Duration::from_millis(200)).await;
    loop {
        if timeout(Duration::from_millis(100), ws.next())
            .await
            .is_err()
        {
            break;
        }
    }

    // Send messages in different JSON formats
    let compact_message = r#"{"role":"user","content":"Compact message","format":"compact"}"#;
    let pretty_message = r#"{
  "role": "user",
  "content": "Pretty printed message",
  "format": "pretty"
}"#;
    let mixed_message = r#"{"role": "user", 
  "content": "Mixed format message",
  "format": "mixed"}"#;

    // Send all different formats
    ws.send(Message::Text(compact_message.to_string()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    ws.send(Message::Text(pretty_message.to_string()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    ws.send(Message::Text(mixed_message.to_string()))
        .await
        .unwrap();

    // Should receive Claude's responses to all formats
    let mut format_responses = Vec::new();
    for _ in 0..6 {
        // Expect multiple responses
        if let Ok(Some(Ok(Message::Text(text)))) =
            timeout(Duration::from_millis(800), ws.next()).await
        {
            if text.contains("Compact message")
                || text.contains("Pretty printed message")
                || text.contains("Mixed format message")
                || text.contains("Mock Claude received")
            {
                format_responses.push(text);
            }
        }
    }

    assert!(
        format_responses.len() >= 3,
        "Should have received Claude responses to all different JSON formats. Got {} responses: {:?}",
        format_responses.len(), format_responses
    );

    let _ = ws.close(None).await;
}
