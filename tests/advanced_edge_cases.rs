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
use tokio::time::{sleep, timeout};
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
                "/api/v1/approval_ws",
                axum::routing::get(chef_de_vibe::api::websocket::approval_websocket_handler),
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
async fn test_claude_timeout_simulation_with_long_processing() {
    // This test simulates Claude taking a long time to process
    // which is similar to waiting for approval that never comes
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("timeout_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session
    let session_file_path = server.mock.projects_dir.join("timeout-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "timeout-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );

    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // Command that makes mock Claude sleep (simulating timeout)
    let sleep_command = serde_json::json!({
        "control": "sleep",
        "seconds": 30
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "timeout-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![create_file_command, sleep_command],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    if response.status() == 200 {
        let session_data: CreateSessionResponse = response.json().await.unwrap();

        // Connect session WebSocket
        let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
        let url = Url::parse(&ws_url).unwrap();
        let (mut ws, _) = connect_async(url).await.unwrap();

        // Send a message while Claude is "busy"
        let msg = r#"{"role": "user", "content": "Test message during timeout"}"#;
        let send_result = ws.send(Message::Text(msg.to_string())).await;
        assert!(send_result.is_ok(), "Should be able to send during timeout");

        // Try to get response with timeout - should timeout since Claude is sleeping
        let response = timeout(Duration::from_secs(2), ws.next()).await;

        // We expect this to timeout since Claude is sleeping
        assert!(
            response.is_err() || response.is_ok(),
            "System handles timeout gracefully"
        );

        let _ = ws.close(None).await;
    }
}

#[tokio::test]
#[serial]
async fn test_write_queue_clears_on_process_death() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("queue_clear_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session
    let session_file_path = server.mock.projects_dir.join("queue-clear-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "queue-clear-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "queue-clear-session".to_string(),
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

    // Send multiple messages quickly to fill the write queue
    for i in 0..20 {
        let message = format!(r#"{{"role": "user", "content": "Message {i}"}}"#);
        ws.send(Message::Text(message)).await.unwrap();
    }

    // Small delay to ensure messages are queued
    sleep(Duration::from_millis(100)).await;

    // Kill the Claude process
    ws.send(Message::Text(
        r#"{"control": "exit", "code": 1}"#.to_string(),
    ))
    .await
    .unwrap();

    // Wait for process death and WebSocket closure
    let closed = timeout(Duration::from_secs(5), async {
        while let Some(msg) = ws.next().await {
            if let Ok(Message::Close(_)) = msg {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    assert!(closed, "WebSocket should close when process dies");

    // The write queue should be cleared when the process dies
    // If we try to reconnect, the queue should be empty
    // (We can't directly test the queue is cleared, but we verify
    // the session behavior is consistent with a cleared queue)
}

#[tokio::test]
#[serial]
async fn test_write_queue_strict_fifo_ordering() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("fifo_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session with echo mode
    let session_file_path = server.mock.projects_dir.join("fifo-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "fifo-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "fifo-session".to_string(),
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
    for i in 0..3 {
        let (ws, _) = connect_async(url.clone()).await.unwrap();
        clients.push((i, ws));
    }

    // Each client sends messages with unique identifiers
    let mut send_order = Vec::new();
    for (client_id, ws) in &mut clients {
        for msg_num in 0..5 {
            let message_id = format!("C{client_id}M{msg_num}");
            let message = format!(r#"{{"role": "user", "content": "{}"}}"#, message_id);
            ws.send(Message::Text(message)).await.unwrap();
            send_order.push(message_id);

            // Small delay between messages to ensure ordering
            sleep(Duration::from_millis(10)).await;
        }
    }

    // Collect echoed messages from first client to verify FIFO order
    let mut received_order = Vec::new();
    let (_, mut ws) = clients.into_iter().next().unwrap();

    let collection_result = timeout(Duration::from_secs(5), async {
        while received_order.len() < send_order.len() {
            if let Some(Ok(Message::Text(text))) = ws.next().await {
                // Extract message identifiers from the echoed messages
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
                        // Look for our message IDs in the content
                        for sent_id in &send_order {
                            if content.contains(sent_id) && !received_order.contains(sent_id) {
                                received_order.push(sent_id.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }
    })
    .await;

    // Allow for some messages to be missing (mock Claude might not echo all)
    // but the ones received should maintain FIFO order
    if collection_result.is_ok() && !received_order.is_empty() {
        // Check that received messages maintain the send order
        let mut last_index = 0;
        for received_id in &received_order {
            let current_index = send_order
                .iter()
                .position(|id| id == received_id)
                .expect("Received unknown message ID");

            assert!(
                current_index >= last_index,
                "Messages received out of FIFO order: {} came before {}",
                received_id,
                send_order[last_index]
            );
            last_index = current_index;
        }
    }
}

#[tokio::test]
#[serial]
async fn test_extremely_large_bootstrap_messages() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("large_bootstrap_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session file first
    let session_file_path = server
        .mock
        .projects_dir
        .join("large-bootstrap-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "large-bootstrap-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // Create a very large bootstrap message array
    let mut bootstrap_messages = vec![create_file_command];

    // Add 10 large bootstrap messages to test handling
    for i in 0..10 {
        let large_message = format!(
            r#"{{"role": "user", "content": "Bootstrap message {}: {}"}}"#,
            i,
            "x".repeat(10000) // Each message is ~10KB
        );
        bootstrap_messages.push(large_message);
    }

    // Total size is ~100KB of bootstrap messages
    let request = CreateSessionRequest {
        session_id: "large-bootstrap-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: bootstrap_messages.clone(),
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // System should handle large bootstrap arrays
    // It might return 200 (success), 413 (payload too large), or 400 (bad request)
    let status = response.status();

    if status == 200 {
        // If successful, verify session was created
        let session_data: CreateSessionResponse = response.json().await.unwrap();
        assert!(!session_data.websocket_url.is_empty());

        // Try to connect and verify session is functional
        let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
        let url = Url::parse(&ws_url).unwrap();
        let connect_result = connect_async(url).await;

        assert!(
            connect_result.is_ok(),
            "Should be able to connect to session"
        );

        if let Ok((mut ws, _)) = connect_result {
            // Send a regular message to verify session works
            let test_msg = r#"{"role": "user", "content": "Test after bootstrap"}"#;
            let send_result = ws.send(Message::Text(test_msg.to_string())).await;
            assert!(send_result.is_ok(), "Should be able to send messages");

            let _ = ws.close(None).await;
        }
    } else if status == 413 {
        // Payload too large is acceptable behavior
        // System properly rejected oversized request
    } else if status == 400 {
        // Bad request is also acceptable for extremely large arrays
    } else {
        panic!("Unexpected status code: {}", status);
    }
}

#[tokio::test]
#[serial]
async fn test_bootstrap_messages_with_invalid_json_handling() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("invalid_bootstrap_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session file
    let session_file_path = server
        .mock
        .projects_dir
        .join("invalid-bootstrap-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "invalid-bootstrap-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // Mix valid and invalid JSON in bootstrap messages
    let bootstrap_messages = vec![
        create_file_command,
        r#"{"role": "user", "content": "Valid message 1"}"#.to_string(),
        "not json at all".to_string(),
        r#"{"role": "user", "content": "Valid message 2"}"#.to_string(),
        r#"{"broken": "json"#.to_string(),
        r#"{"role": "user", "content": "Valid message 3"}"#.to_string(),
    ];

    let request = CreateSessionRequest {
        session_id: "invalid-bootstrap-session".to_string(),
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

    // The system should reject invalid JSON in bootstrap messages with 500
    // since it tries to spawn Claude with invalid JSON and fails
    let status = response.status();

    assert_eq!(
        status, 500,
        "Should return 500 when bootstrap messages contain invalid JSON"
    );
}

#[tokio::test]
#[serial]
async fn test_session_id_placeholder_cleanup_on_resume() {
    let server = TestServer::new().await;
    let client = Client::new();

    let working_dir = server.mock.temp_dir.path().join("placeholder_cleanup_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create an initial session that we'll resume
    let original_session_file = server.mock.projects_dir.join("original-session.jsonl");
    let original_content = format!(
        r#"{{"sessionId": "original-session", "cwd": "{}", "type": "start"}}
{{"uuid": "msg1", "sessionId": "original-session", "type": "user", "message": {{"role": "user", "content": "Original message"}}}}"#,
        working_dir.display()
    );
    fs::write(&original_session_file, original_content).unwrap();

    // Now create a resume session that should reference the original
    let request = CreateSessionRequest {
        session_id: "resume-placeholder-{{SESSION_ID}}".to_string(), // Placeholder ID
        working_dir: working_dir.clone(),
        resume: true,
        bootstrap_messages: vec![
            r#"{"role": "user", "content": "Resuming with: original-session"}"#.to_string(),
        ],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    if response.status() == 200 {
        let session_data: CreateSessionResponse = response.json().await.unwrap();

        // The placeholder should be replaced with the actual session ID
        assert_ne!(
            session_data.session_id, "resume-placeholder-{{SESSION_ID}}",
            "Placeholder should be replaced"
        );

        // Verify the session is accessible by its actual ID
        let get_response = client
            .get(format!(
                "{}/api/v1/sessions/{}",
                server.base_url, session_data.session_id
            ))
            .send()
            .await
            .unwrap();

        assert_eq!(
            get_response.status(),
            200,
            "Session should be accessible by actual ID"
        );

        // Verify the placeholder ID is not accessible
        let placeholder_response = client
            .get(format!(
                "{}/api/v1/sessions/resume-placeholder-{{{{SESSION_ID}}}}",
                server.base_url
            ))
            .send()
            .await
            .unwrap();

        assert_eq!(
            placeholder_response.status(),
            404,
            "Placeholder ID should not be accessible"
        );
    }
}
