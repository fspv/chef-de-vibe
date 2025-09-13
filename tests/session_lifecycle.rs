mod helpers;

use chef_de_vibe::{
    api::handlers::AppState,
    config::Config,
    models::{
        CreateSessionRequest, GetSessionResponse, ListSessionsResponse,
    },
    session_manager::SessionManager,
};
use helpers::mock_claude::MockClaude;
use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;
use futures_util::SinkExt;


struct TestServer {
    pub base_url: String,
    pub ws_url: String,
    pub mock: MockClaude,
    server_handle: tokio::task::JoinHandle<()>,
    session_manager: Arc<SessionManager>,
}

impl TestServer {
    async fn new() -> Self {
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
        }).join().ok();
    }
}

#[tokio::test]
#[serial]
async fn test_session_lifecycle() {
    let server = TestServer::new().await;
    let client = Client::new();

    // 1. List sessions (should be empty)
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();
    let body: ListSessionsResponse = response.json().await.unwrap();
    assert_eq!(body.sessions.len(), 0);

    // 2. Create working directory
    let working_dir = server.mock.temp_dir.path().join("lifecycle_work");
    fs::create_dir_all(&working_dir).unwrap();

    // 3. Create new session
    let request = CreateSessionRequest {
        session_id: "lifecycle-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message: r#"{"role": "user", "content": "Hello"}"#.to_string(),
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    // 4. List sessions (should have 1 active)
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();
    let body: ListSessionsResponse = response.json().await.unwrap();
    assert_eq!(body.sessions.len(), 1);
    assert!(body.sessions[0].active);

    // 5. Get session details
    let response = client
        .get(format!("{}/api/v1/sessions/lifecycle-session", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: GetSessionResponse = response.json().await.unwrap();
    assert!(body.websocket_url.is_some());
    assert!(body.approval_websocket_url.is_some());

    // 6. Connect to WebSocket and send message using URL from API response
    let ws_url = format!("{}{}", server.ws_url, body.websocket_url.unwrap());
    let url = Url::parse(&ws_url).unwrap();
    let (mut ws_stream, _) = connect_async(url).await.unwrap();

    let test_message = r#"{"role": "user", "content": "Test lifecycle message"}"#;
    ws_stream
        .send(Message::Text(test_message.to_string()))
        .await
        .unwrap();

    // 7. Clean up
    let _ = ws_stream.close(None).await;
}

#[tokio::test]
#[serial]
async fn test_corrupted_session_file() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create a corrupted session file
    let project_dir = server.mock.projects_dir.join("corrupted");
    fs::create_dir_all(&project_dir).unwrap();
    
    let session_file = project_dir.join("corrupted-session.jsonl");
    let invalid_content = r#"{"sessionId": "corrupted-session", "cwd": "/invalid/path"
{invalid json line without closing brace
{"type": "user", "message": malformed content}"#;
    
    fs::write(session_file, invalid_content).unwrap();

    // List sessions should handle the corrupted file gracefully by ignoring it
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    // Should return 200 and succeed, ignoring the corrupted file
    assert_eq!(response.status(), 200);
    
    let body: ListSessionsResponse = response.json().await.unwrap();
    // The corrupted file should be ignored, so no sessions should be returned
    assert!(body.sessions.is_empty());
}

#[tokio::test]
#[serial]
async fn test_session_id_mismatch_in_file() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create a session file where the filename doesn't match the sessionId in content
    let project_dir = server.mock.projects_dir.join("mismatch");
    fs::create_dir_all(&project_dir).unwrap();
    
    let session_file = project_dir.join("file-session-id.jsonl");
    let content_with_different_id = r#"{"sessionId": "different-session-id", "cwd": "/home/user/project", "type": "start"}
{"type": "user", "message": {"role": "user", "content": "Hello"}}"#;
    
    fs::write(session_file, content_with_different_id).unwrap();

    // List sessions should handle the mismatch gracefully by ignoring the file
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    // Should return 200 and succeed, ignoring the mismatched file
    assert_eq!(response.status(), 200);
    
    let body: ListSessionsResponse = response.json().await.unwrap();
    // The mismatched file should be ignored, so no sessions should be returned
    assert!(body.sessions.is_empty());
}

#[tokio::test]
#[serial]
async fn test_complete_session_content_preservation() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create session file with complete JSON structure
    server
        .mock
        .create_complete_session_file("project1", "complete-session", "/home/user/project1");

    let response = client
        .get(format!("{}/api/v1/sessions/complete-session", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: GetSessionResponse = response.json().await.unwrap();
    assert_eq!(body.session_id, "complete-session");
    assert_eq!(body.websocket_url, None); // Not active
    assert_eq!(body.approval_websocket_url, None); // Not active

    // Verify we got all 3 entries
    assert_eq!(body.content.len(), 3);

    // Verify first entry preserves all fields
    let first_entry = &body.content[0];
    assert!(first_entry.get("parentUuid").is_some());
    assert_eq!(first_entry.get("parentUuid"), Some(&serde_json::Value::Null));
    assert_eq!(first_entry.get("isSidechain"), Some(&serde_json::Value::Bool(false)));
    assert_eq!(first_entry.get("userType"), Some(&serde_json::Value::String("external".to_string())));
    assert_eq!(first_entry.get("sessionId"), Some(&serde_json::Value::String("complete-session".to_string())));
    assert_eq!(first_entry.get("version"), Some(&serde_json::Value::String("1.0.65".to_string())));
    assert_eq!(first_entry.get("gitBranch"), Some(&serde_json::Value::String("master".to_string())));
    assert_eq!(first_entry.get("uuid"), Some(&serde_json::Value::String("30644cc3-c5cd-4e9e-953a-bbe299394703".to_string())));
    assert!(first_entry.get("timestamp").is_some());
    assert!(first_entry.get("message").is_some());

    // Verify second entry preserves complex fields
    let second_entry = &body.content[1];
    assert_eq!(second_entry.get("parentUuid"), Some(&serde_json::Value::String("30644cc3-c5cd-4e9e-953a-bbe299394703".to_string())));
    assert_eq!(second_entry.get("requestId"), Some(&serde_json::Value::String("req_011CSyZqUfBhmGuMy8ymqeNp".to_string())));
    assert!(second_entry.get("message").is_some());
    
    // Verify message contains usage info
    let message = second_entry.get("message").unwrap();
    assert!(message.get("usage").is_some());
    let usage = message.get("usage").unwrap();
    assert!(usage.get("input_tokens").is_some());
    assert!(usage.get("cache_creation_input_tokens").is_some());

    // Verify third entry (system message)
    let third_entry = &body.content[2];
    assert_eq!(third_entry.get("type"), Some(&serde_json::Value::String("system".to_string())));
    assert_eq!(third_entry.get("message"), Some(&serde_json::Value::String("Session started".to_string())));
}