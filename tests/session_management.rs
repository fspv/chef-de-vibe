mod helpers;

use chef_de_vibe::{
    api::handlers::AppState,
    config::Config,
    models::{
        CreateSessionRequest, CreateSessionResponse, GetSessionResponse, ListSessionsResponse,
    },
    session_manager::SessionManager,
};
use helpers::logging::init_logging;
use helpers::mock_claude::MockClaude;
use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;


// Helper function to create test session files on disk
// These are used to test that the service can find and list historical sessions
fn create_test_session_file(
    projects_dir: &std::path::Path,
    project_name: &str,
    session_id: &str,
    cwd: &str,
) {
    let project_dir = projects_dir.join(project_name);
    fs::create_dir_all(&project_dir).unwrap();

    let session_file = project_dir.join(format!("{}.jsonl", session_id));
    let content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Hello Claude"}}}}
{{"type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "Hello! How can I help you today?"}}]}}}}
{{"type": "user", "message": {{"role": "user", "content": "What's 2+2?"}}}}
{{"type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "2 + 2 equals 4."}}]}}}}
"#,
        session_id, cwd
    );

    fs::write(session_file, content).unwrap();
}

struct TestServer {
    pub base_url: String,
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

        // Spawn server
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give server time to start - increase for better test isolation
        tokio::time::sleep(Duration::from_millis(500)).await;

        Self {
            base_url,
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
    create_test_session_file(
        &server.mock.projects_dir,
        "project1",
        "session-123",
        "/home/user/project1",
    );
    create_test_session_file(
        &server.mock.projects_dir,
        "project2",
        "session-456",
        "/home/user/project2",
    );

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

    // Create session file control command
    let session_file_path = server.mock.projects_dir.join("test-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "test-session", "cwd": "{}", "type": "start"}}"#,
        working_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "test-session".to_string(),
        working_dir: working_dir.clone(),
        resume: false,
        first_message: vec![
            create_file_command,
            r#"{"role": "user", "content": "Hello"}"#.to_string(),
        ],
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
    assert_eq!(
        body.websocket_url,
        "/api/v1/sessions/test-session/claude_ws"
    );
    assert_eq!(
        body.approval_websocket_url,
        "/api/v1/sessions/test-session/claude_approvals_ws"
    );

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
        first_message: vec![r#"{"role": "user", "content": "Hello"}"#.to_string()],
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
    create_test_session_file(
        &server.mock.projects_dir,
        "work",
        "old-session",
        working_dir.to_str().unwrap(),
    );

    // For resume mode, we need to:
    // 1. Return a session initialization with a NEW session ID
    // 2. Create a session file with that new session ID
    let new_session_id = "resumed-session-123";
    let session_file_path = server
        .mock
        .projects_dir
        .join(format!("{}.jsonl", new_session_id));
    let session_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        new_session_id,
        working_dir.display()
    );

    // Create control commands to return the new session ID and create its file
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let session_init_response = serde_json::json!({
        "session_id": new_session_id,
        "type": "start"
    })
    .to_string();

    let request = CreateSessionRequest {
        session_id: "old-session".to_string(),
        working_dir: working_dir.clone(),
        resume: true,
        first_message: vec![
            create_file_command,
            session_init_response,
            r#"{"role": "user", "content": "Resume session"}"#.to_string(),
        ],
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
        first_message: vec![r#"{"role": "user", "content": "Hello"}"#.to_string()],
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
    create_test_session_file(
        &server.mock.projects_dir,
        "project1",
        "disk-session",
        "/home/user/project1",
    );

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
