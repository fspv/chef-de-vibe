mod helpers;

use chef_de_vibe::{
    api::handlers::AppState,
    config::Config,
    models::{
        CreateSessionRequest, CreateSessionResponse, GetSessionResponse, ListSessionsResponse,
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
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::debug;
use url::Url;
use axum;

fn generate_unique_session_id(test_name: &str) -> String {
    format!("{}-{}-{}", 
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
        let mock = MockClaude::new();
        mock.setup_env_vars();
        Self::new_internal(mock).await
    }

    async fn new_with_approval_binary() -> Self {
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

#[tokio::test]
#[serial]
async fn test_list_empty_sessions() {
    let server = TestServer::new().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: ListSessionsResponse = response.json().await.unwrap();
    assert_eq!(body.sessions.len(), 0);
}

#[tokio::test]
#[serial]
async fn test_list_sessions_with_disk_sessions() {
    let server = TestServer::new().await;

    // Create test session files on disk
    server
        .mock
        .create_test_session_file("project1", "session-123", "/home/user/project1");
    server
        .mock
        .create_test_session_file("project2", "session-456", "/home/user/project2");

    let client = Client::new();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: ListSessionsResponse = response.json().await.unwrap();
    assert_eq!(body.sessions.len(), 2);

    let session_ids: Vec<String> = body.sessions.iter().map(|s| s.session_id.clone()).collect();
    assert!(session_ids.contains(&"session-123".to_string()));
    assert!(session_ids.contains(&"session-456".to_string()));

    // All sessions should be inactive since no processes are running
    for session in &body.sessions {
        assert!(!session.active);
    }
}

#[tokio::test]
#[serial]
async fn test_create_new_session() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("work");
    fs::create_dir_all(&working_dir).unwrap();

    let request = CreateSessionRequest {
        session_id: "test-session".to_string(),
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

    let body: CreateSessionResponse = response.json().await.unwrap();
    assert_eq!(body.session_id, "test-session");
    assert_eq!(body.websocket_url, "/api/v1/sessions/test-session/claude_ws");
    assert_eq!(body.approval_websocket_url, "/api/v1/sessions/test-session/claude_approvals_ws");

    // Verify session appears in list as active
    let list_response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    let list_body: ListSessionsResponse = list_response.json().await.unwrap();
    assert_eq!(list_body.sessions.len(), 1);
    assert_eq!(list_body.sessions[0].session_id, "test-session");
    assert!(list_body.sessions[0].active);
}

#[tokio::test]
#[serial]
async fn test_create_session_invalid_working_dir() {
    let server = TestServer::new().await;
    let client = Client::new();

    let request = CreateSessionRequest {
        session_id: "test-session".to_string(),
        working_dir: server.mock.temp_dir.path().join("non-existent"),
        resume: false,
        first_message: r#"{"role": "user", "content": "Hello"}"#.to_string(),
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
#[serial]
async fn test_resume_session() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create an existing session file
    server
        .mock
        .create_test_session_file("work", "old-session", working_dir.to_str().unwrap());

    let request = CreateSessionRequest {
        session_id: "old-session".to_string(),
        working_dir: working_dir.clone(),
        resume: true,
        first_message: r#"{"role": "user", "content": "Resume session"}"#.to_string(),
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: CreateSessionResponse = response.json().await.unwrap();
    // Should get a new session ID that starts with "resumed-"
    assert!(body.session_id.starts_with("resumed-"));
    assert!(body.websocket_url.contains(&body.session_id));
    assert!(body.approval_websocket_url.contains(&body.session_id));
}

#[tokio::test]
#[serial]
async fn test_get_session_active() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory
    let working_dir = server.mock.temp_dir.path().join("work");
    fs::create_dir_all(&working_dir).unwrap();

    // Create session first
    let request = CreateSessionRequest {
        session_id: "test-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message: r#"{"role": "user", "content": "Hello"}"#.to_string(),
    };

    client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request)
        .send()
        .await
        .unwrap();

    // Now get the session
    let response = client
        .get(format!("{}/api/v1/sessions/test-session", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: GetSessionResponse = response.json().await.unwrap();
    assert_eq!(body.session_id, "test-session");
    assert_eq!(
        body.websocket_url,
        Some("/api/v1/sessions/test-session/claude_ws".to_string())
    );
    assert_eq!(
        body.approval_websocket_url,
        Some("/api/v1/sessions/test-session/claude_approvals_ws".to_string())
    );
}

#[tokio::test]
#[serial]
async fn test_get_session_from_disk() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create session file on disk (inactive session)
    server
        .mock
        .create_test_session_file("project1", "disk-session", "/home/user/project1");

    let response = client
        .get(format!("{}/api/v1/sessions/disk-session", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: GetSessionResponse = response.json().await.unwrap();
    assert_eq!(body.session_id, "disk-session");
    assert_eq!(body.websocket_url, None); // Not active
    assert_eq!(body.approval_websocket_url, None); // Not active
    assert!(!body.content.is_empty()); // Should have content from file
}

#[tokio::test]
#[serial]
async fn test_get_session_not_found() {
    let server = TestServer::new().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/api/v1/sessions/non-existent", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}