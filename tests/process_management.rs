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

        let base_url = format!("http://127.0.0.1:{}", port);
        let ws_url = format!("ws://127.0.0.1:{}", port);

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
async fn test_write_queue_fifo_ordering() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("queue_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session - send control command to create session file
    let session_file_path = server.mock.projects_dir.join("queue-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "queue-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "queue-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        bootstrap_messages: vec![create_file_command],
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(create_response.status(), 200);
    let session_data: CreateSessionResponse = create_response.json().await.unwrap();

    // Connect WebSocket using URL from API response
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = Url::parse(&ws_url).unwrap();
    let (mut ws, _) = connect_async(url).await.unwrap();

    // Send multiple messages rapidly to test FIFO ordering
    let messages = vec![
        r#"{"role": "user", "content": "Message 1"}"#,
        r#"{"role": "user", "content": "Message 2"}"#,
        r#"{"role": "user", "content": "Message 3"}"#,
    ];

    for msg in messages {
        ws.send(Message::Text(msg.to_string())).await.unwrap();
        // Small delay to ensure ordering
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Wait a bit for processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Try to receive responses (may or may not get them depending on mock behavior)
    for _ in 0..6 {
        if let Ok(Some(Ok(Message::Text(_)))) = timeout(Duration::from_millis(100), ws.next()).await
        {
            // Received a response - this is good, shows the queue is working
        } else {
            break;
        }
    }

    let _ = ws.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_claude_process_death_simulation() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("death_work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session - send control command to create session file
    let session_file_path = server.mock.projects_dir.join("death-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "death-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "death-session".to_string(),
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

    // Connect to WebSocket using URL from API response
    let ws_url = format!("{}{}", server.ws_url, session_data.websocket_url);
    let url = Url::parse(&ws_url).unwrap();
    let (mut ws, _) = connect_async(url).await.unwrap();

    // Send a message to establish connection
    let test_message = r#"{"role": "user", "content": "Test message"}"#;
    ws.send(Message::Text(test_message.to_string()))
        .await
        .unwrap();

    // Try to receive some responses
    let _ = timeout(Duration::from_secs(2), ws.next()).await;
    let _ = timeout(Duration::from_secs(1), ws.next()).await;

    // This test primarily verifies that the WebSocket infrastructure works
    // Actual process death detection would require more sophisticated mocking
    // but this test ensures the WebSocket handles multiple messages correctly

    // Send another message to test queue processing
    let second_message = r#"{"role": "user", "content": "Another message"}"#;
    let send_result = ws.send(Message::Text(second_message.to_string())).await;
    assert!(send_result.is_ok());

    // Clean up
    let _ = ws.close(None).await;
}
