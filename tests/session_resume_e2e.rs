mod helpers;

use chef_de_vibe::{
    api::handlers::AppState,
    config::Config,
    models::{CreateSessionRequest, CreateSessionResponse, GetSessionResponse},
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
use tracing::{debug, info};

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
        let base_url = format!("http://127.0.0.1:{port}");

        // Spawn server
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give server time to start
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
                // Give time for ongoing operations to complete
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

/// Test that session resume works when session ID comes in the first message
#[tokio::test]
#[serial]
async fn test_session_resume_with_session_id_in_first_message() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory for the session
    let work_dir = server.mock.temp_dir.path().join("resume_first");
    fs::create_dir_all(&work_dir).unwrap();

    info!("Created test directory: {:?}", work_dir);

    // Create an existing session file on disk to simulate a session to resume
    let original_session_file = server.mock.projects_dir.join("original-session-1.jsonl");
    let original_content = format!(
        r#"{{"sessionId": "original-session-1", "cwd": "{}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Original message"}}}}
"#,
        work_dir.display()
    );
    fs::write(original_session_file, original_content).unwrap();

    // Prepare resume request where the first message contains the session ID
    let new_session_id = "resumed-session-first-123";
    let session_file_path = server
        .mock
        .projects_dir
        .join(format!("{new_session_id}.jsonl"));

    // Create control command to write the new session file
    let session_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        new_session_id,
        work_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // First message from Claude contains the session ID
    let session_response = serde_json::json!({
        "session_id": new_session_id,
        "type": "start",
        "message": "Session resumed successfully"
    })
    .to_string();

    let resume_request = CreateSessionRequest {
        session_id: "original-session-1".to_string(),
        working_dir: work_dir.clone(),
        resume: true,
        bootstrap_messages: vec![
            create_file_command,
            session_response, // Session ID in first message
            r#"{"role": "user", "content": "Resume this session"}"#.to_string(),
        ],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&resume_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let response_body: CreateSessionResponse = response.json().await.unwrap();
    assert_eq!(response_body.session_id, new_session_id);

    debug!(
        "Successfully resumed session with ID in first message: {}",
        new_session_id
    );

    // Verify the resumed session is active
    let get_response = client
        .get(format!(
            "{}/api/v1/sessions/{}",
            server.base_url, new_session_id
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), 200);
    let session: GetSessionResponse = get_response.json().await.unwrap();
    assert_eq!(session.session_id, new_session_id);
    assert_eq!(session.working_directory, work_dir);
    assert!(session.websocket_url.is_some());
    assert!(session.approval_websocket_url.is_some());

    info!("Successfully tested resume with session ID in first message");
}

/// Test that session resume works when session ID comes in the second message after a mode control request
#[tokio::test]
#[serial]
async fn test_session_resume_with_session_id_in_second_message() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory for the session
    let work_dir = server.mock.temp_dir.path().join("resume_second");
    fs::create_dir_all(&work_dir).unwrap();

    info!("Created test directory: {:?}", work_dir);

    // Create an existing session file on disk to simulate a session to resume
    let original_session_file = server.mock.projects_dir.join("original-session-2.jsonl");
    let original_content = format!(
        r#"{{"sessionId": "original-session-2", "cwd": "{}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Original message"}}}}
"#,
        work_dir.display()
    );
    fs::write(original_session_file, original_content).unwrap();

    // Prepare resume request where the session ID comes in the second message
    let new_session_id = "resumed-session-second-456";
    let session_file_path = server
        .mock
        .projects_dir
        .join(format!("{new_session_id}.jsonl"));

    // Create control command to write the new session file
    let session_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        new_session_id,
        work_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // First message is a mode control response (no session_id)
    let mode_response = serde_json::json!({
        "type": "control_response",
        "status": "mode_set",
        "mode": "default"
    })
    .to_string();

    // Second message contains the session ID
    let session_response = serde_json::json!({
        "session_id": new_session_id,
        "type": "start",
        "message": "Session resumed after mode set"
    })
    .to_string();

    let resume_request = CreateSessionRequest {
        session_id: "original-session-2".to_string(),
        working_dir: work_dir.clone(),
        resume: true,
        bootstrap_messages: vec![
            create_file_command,
            mode_response,    // First message - no session ID
            session_response, // Second message - contains session ID
            r#"{"role": "user", "content": "Resume this session"}"#.to_string(),
        ],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&resume_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let response_body: CreateSessionResponse = response.json().await.unwrap();
    assert_eq!(response_body.session_id, new_session_id);

    debug!(
        "Successfully resumed session with ID in second message: {}",
        new_session_id
    );

    // Verify the resumed session is active
    let get_response = client
        .get(format!(
            "{}/api/v1/sessions/{}",
            server.base_url, new_session_id
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), 200);
    let session: GetSessionResponse = get_response.json().await.unwrap();
    assert_eq!(session.session_id, new_session_id);
    assert_eq!(session.working_directory, work_dir);
    assert!(session.websocket_url.is_some());
    assert!(session.approval_websocket_url.is_some());

    info!("Successfully tested resume with session ID in second message");
}

/// Test that session resume works when multiple non-session messages precede the session ID
#[tokio::test]
#[serial]
async fn test_session_resume_with_multiple_messages_before_session_id() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory for the session
    let work_dir = server.mock.temp_dir.path().join("resume_multiple");
    fs::create_dir_all(&work_dir).unwrap();

    info!("Created test directory: {:?}", work_dir);

    // Create an existing session file on disk to simulate a session to resume
    let original_session_file = server.mock.projects_dir.join("original-session-3.jsonl");
    let original_content = format!(
        r#"{{"sessionId": "original-session-3", "cwd": "{}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Original message"}}}}
"#,
        work_dir.display()
    );
    fs::write(original_session_file, original_content).unwrap();

    // Prepare resume request where the session ID comes after multiple messages
    let new_session_id = "resumed-session-multiple-789";
    let session_file_path = server
        .mock
        .projects_dir
        .join(format!("{new_session_id}.jsonl"));

    // Create control command to write the new session file
    let session_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        new_session_id,
        work_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // Multiple messages before the session ID
    let mode_response = serde_json::json!({
        "type": "control_response",
        "status": "mode_set",
        "mode": "default"
    })
    .to_string();

    let status_response = serde_json::json!({
        "type": "status",
        "message": "Loading session data"
    })
    .to_string();

    let progress_response = serde_json::json!({
        "type": "progress",
        "percentage": 50,
        "message": "Processing history"
    })
    .to_string();

    // Finally, the message with session ID
    let session_response = serde_json::json!({
        "session_id": new_session_id,
        "type": "start",
        "message": "Session resumed after multiple messages"
    })
    .to_string();

    let resume_request = CreateSessionRequest {
        session_id: "original-session-3".to_string(),
        working_dir: work_dir.clone(),
        resume: true,
        bootstrap_messages: vec![
            create_file_command,
            mode_response,     // Message 1 - no session ID
            status_response,   // Message 2 - no session ID
            progress_response, // Message 3 - no session ID
            session_response,  // Message 4 - contains session ID
            r#"{"role": "user", "content": "Resume this session"}"#.to_string(),
        ],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&resume_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let response_body: CreateSessionResponse = response.json().await.unwrap();
    assert_eq!(response_body.session_id, new_session_id);

    debug!(
        "Successfully resumed session with ID after multiple messages: {}",
        new_session_id
    );

    // Verify the resumed session is active
    let get_response = client
        .get(format!(
            "{}/api/v1/sessions/{}",
            server.base_url, new_session_id
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), 200);
    let session: GetSessionResponse = get_response.json().await.unwrap();
    assert_eq!(session.session_id, new_session_id);
    assert_eq!(session.working_directory, work_dir);
    assert!(session.websocket_url.is_some());
    assert!(session.approval_websocket_url.is_some());

    info!("Successfully tested resume with session ID after multiple messages");
}

/// Test error handling when session ID is not found within the maximum message limit
#[tokio::test]
#[serial]
async fn test_session_resume_fails_when_no_session_id_found() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory for the session
    let work_dir = server.mock.temp_dir.path().join("resume_fail");
    fs::create_dir_all(&work_dir).unwrap();

    info!("Created test directory: {:?}", work_dir);

    // Create an existing session file on disk to simulate a session to resume
    let original_session_file = server.mock.projects_dir.join("original-session-fail.jsonl");
    let original_content = format!(
        r#"{{"sessionId": "original-session-fail", "cwd": "{}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Original message"}}}}
"#,
        work_dir.display()
    );
    fs::write(original_session_file, original_content).unwrap();

    // Prepare resume request with only messages that don't contain session_id
    // and then exit, so the process will timeout waiting for session_id
    let mode_response = serde_json::json!({
        "type": "control_response",
        "status": "mode_set",
        "mode": "default"
    })
    .to_string();

    let status_response = serde_json::json!({
        "type": "status",
        "message": "Loading..."
    })
    .to_string();

    // Exit command to make the mock Claude process exit without sending session_id
    let exit_command = serde_json::json!({
        "control": "exit",
        "code": 0
    })
    .to_string();

    let resume_request = CreateSessionRequest {
        session_id: "original-session-fail".to_string(),
        working_dir: work_dir.clone(),
        resume: true,
        bootstrap_messages: vec![
            mode_response,
            status_response,
            exit_command, // This will make the process exit without sending session_id
        ],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&resume_request)
        .send()
        .await
        .unwrap();

    // The request should fail since no session_id was found
    assert_eq!(response.status(), 500);

    let error_text = response.text().await.unwrap();
    assert!(
        error_text.contains("closed stdout without sending session ID")
            || error_text.contains("Failed to spawn Claude process")
    );

    info!("Successfully tested error handling when session ID is not found");
}

/// Test that non-resume sessions still work correctly (session ID not needed from Claude)
#[tokio::test]
#[serial]
async fn test_non_resume_session_creation() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory for the session
    let work_dir = server.mock.temp_dir.path().join("non_resume");
    fs::create_dir_all(&work_dir).unwrap();

    info!("Created test directory: {:?}", work_dir);

    let session_id = "new-session-non-resume";
    let session_file_path = server.mock.projects_dir.join(format!("{session_id}.jsonl"));

    // Create control command to write the session file
    let session_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        session_id,
        work_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // Claude's responses don't need to contain session_id for non-resume mode
    let mode_response = serde_json::json!({
        "type": "control_response",
        "status": "mode_set",
        "mode": "default"
    })
    .to_string();

    let ready_response = serde_json::json!({
        "type": "ready",
        "message": "Claude is ready"
    })
    .to_string();

    let create_request = CreateSessionRequest {
        session_id: session_id.to_string(),
        working_dir: work_dir.clone(),
        resume: false, // Non-resume mode
        bootstrap_messages: vec![
            create_file_command,
            mode_response,  // No session_id needed
            ready_response, // No session_id needed
            r#"{"role": "user", "content": "Start new session"}"#.to_string(),
        ],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let response_body: CreateSessionResponse = response.json().await.unwrap();
    assert_eq!(response_body.session_id, session_id); // Should use the provided session ID

    debug!("Successfully created non-resume session: {}", session_id);

    // Verify the session is active
    let get_response = client
        .get(format!(
            "{}/api/v1/sessions/{}",
            server.base_url, session_id
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), 200);
    let session: GetSessionResponse = get_response.json().await.unwrap();
    assert_eq!(session.session_id, session_id);
    assert_eq!(session.working_directory, work_dir);
    assert!(session.websocket_url.is_some());
    assert!(session.approval_websocket_url.is_some());

    info!("Successfully tested non-resume session creation");
}
