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
    let project_path = projects_dir.join(project_name);
    fs::create_dir_all(&project_path).unwrap();

    let session_file = project_path.join(format!("{session_id}.jsonl"));
    let content = format!(
        r#"{{"sessionId": "{session_id}", "cwd": "{cwd}", "type": "start"}}
{{"sessionId": "{session_id}", "type": "user", "message": {{"role": "user", "content": "Hello Claude"}}}}
{{"sessionId": "{session_id}", "type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "Hello! How can I help you today?"}}]}}}}
{{"sessionId": "{session_id}", "type": "user", "message": {{"role": "user", "content": "What's 2+2?"}}}}
{{"sessionId": "{session_id}", "type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "2 + 2 equals 4."}}]}}}}
"#
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
        let base_url = format!("http://127.0.0.1:{port}");

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

    // Create test session files on disk WITHOUT summaries
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
    // Sessions without summaries should now be listed with fallback summaries
    assert_eq!(
        body.sessions.len(),
        2,
        "Sessions without summaries should be listed with fallback summaries"
    );

    // All sessions should be inactive since no processes are running
    for session in &body.sessions {
        assert!(!session.active);
        // Should have fallback summary from first user message
        assert_eq!(
            session.summary,
            Some("Hello Claude".to_string()),
            "Session should have fallback summary from first user message"
        );
    }
}

#[tokio::test]
#[serial]
async fn test_list_sessions_with_summaries() {
    let server = TestServer::new().await;

    // Create a more complex scenario with summaries
    let project_path = server.mock.projects_dir.join("project-with-summary");
    fs::create_dir_all(&project_path).unwrap();

    // File 1: Contains a summary pointing to a message in another file
    let summary_file = project_path.join("summary-uuid.jsonl");
    let summary_content =
        r#"{"type":"summary","summary":"API Design Discussion","leafUuid":"msg-uuid-123"}"#;
    fs::write(summary_file, summary_content).unwrap();

    // File 2: Contains the actual session with messages
    let session_file = project_path.join("session-789.jsonl");
    let session_content = r#"{"sessionId": "session-789", "cwd": "/home/user/api-project", "type": "start"}
{"sessionId": "session-789", "type": "user", "message": {"role": "user", "content": "Let's discuss API design"}, "timestamp": "2025-09-19T10:00:00Z"}
{"sessionId": "session-789", "uuid": "msg-uuid-123", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Great! Let's talk about RESTful principles..."}]}, "timestamp": "2025-09-19T10:01:00Z"}"#;
    fs::write(session_file, session_content).unwrap();

    // Also add a session without summary (simulating one that hasn't ended yet)
    let active_session_file = project_path.join("no-summary.jsonl");
    let active_content = r#"{"sessionId": "active-session", "cwd": "/home/user/current", "type": "start"}
{"sessionId": "active-session", "type": "user", "message": {"role": "user", "content": "First user message here"}, "timestamp": "2025-09-19T11:00:00Z"}"#;
    fs::write(active_session_file, active_content).unwrap();

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: ListSessionsResponse = response.json().await.unwrap();

    // Find the session with summary
    let session_with_summary = body
        .sessions
        .iter()
        .find(|s| s.session_id == "session-789")
        .expect("Session 789 should be found");

    assert_eq!(
        session_with_summary.summary,
        Some("API Design Discussion".to_string())
    );
    assert_eq!(
        session_with_summary.earliest_message_date,
        Some("2025-09-19T10:00:00Z".to_string())
    );
    assert_eq!(
        session_with_summary.latest_message_date,
        Some("2025-09-19T10:01:00Z".to_string())
    );

    // The session without summary should now be found with fallback summary
    let session_without_summary = body
        .sessions
        .iter()
        .find(|s| s.session_id == "active-session")
        .expect("Session without summary should now be listed with fallback");

    assert_eq!(
        session_without_summary.summary,
        Some("First user message here".to_string()),
        "Session should have fallback summary from first user message"
    );
}

#[tokio::test]
#[serial]
async fn test_active_sessions_always_listed() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create an active session
    let active_work_dir = server.mock.temp_dir.path().join("active_test");
    fs::create_dir_all(&active_work_dir).unwrap();

    // Create session file on disk (with or without summary)
    let project_path = server.mock.projects_dir.join("active-project");
    fs::create_dir_all(&project_path).unwrap();

    // Prepare session content without a summary (simulating an active session)
    let session_content = format!(
        r#"{{"sessionId": "active-test-session", "cwd": "{}", "type": "start"}}
{{"sessionId": "active-test-session", "type": "user", "message": {{"role": "user", "content": "Hello active"}}, "timestamp": "2025-09-19T12:00:00Z"}}
{{"sessionId": "active-test-session", "uuid": "msg-active-123", "type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "Response"}}]}}, "timestamp": "2025-09-19T12:01:00Z"}}"#,
        active_work_dir.display()
    );

    // Start the active session
    let session_file_path = project_path.join("active-test-session.jsonl");
    let write_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let create_request = CreateSessionRequest {
        session_id: "active-test-session".to_string(),
        working_dir: active_work_dir.clone(),
        resume: false,
        bootstrap_messages: vec![write_command],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&create_request)
        .send()
        .await
        .unwrap();

    // Should be 200 since session already exists on disk
    assert_eq!(response.status(), 200);

    // Now list sessions - the active session should be included
    let list_response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(list_response.status(), 200);

    let body: ListSessionsResponse = list_response.json().await.unwrap();

    // Find the active session
    let active_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "active-test-session")
        .expect("Active session should be listed");

    assert!(active_session.active, "Session should be marked as active");
    assert_eq!(active_session.working_directory, active_work_dir);

    // The session might have the first user message as fallback summary
    if let Some(summary) = &active_session.summary {
        assert_eq!(summary, "Hello active");
    }

    // Also test with a session that has a summary
    let summary_file = project_path.join("summary-active.jsonl");
    let summary_content =
        r#"{"type":"summary","summary":"Active Session with Summary","leafUuid":"msg-active-123"}"#;
    fs::write(summary_file, summary_content).unwrap();

    // List again - should still show the active session
    let list_response2 = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(list_response2.status(), 200);

    let body2: ListSessionsResponse = list_response2.json().await.unwrap();

    let active_session_with_summary = body2
        .sessions
        .iter()
        .find(|s| s.session_id == "active-test-session")
        .expect("Active session should still be listed");

    assert!(active_session_with_summary.active);
    assert_eq!(
        active_session_with_summary.summary,
        Some("Active Session with Summary".to_string())
    );
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
        bootstrap_messages: vec![
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
        bootstrap_messages: vec![r#"{"role": "user", "content": "Hello"}"#.to_string()],
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
        .join(format!("{new_session_id}.jsonl"));
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
        bootstrap_messages: vec![
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
        bootstrap_messages: vec![r#"{"role": "user", "content": "Hello"}"#.to_string()],
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

#[tokio::test]
#[serial]
async fn test_ping_pong_active_session_first_user_message_as_summary() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create a temporary work directory
    let work_dir = server.mock.temp_dir.path().join("ping_pong_test");
    fs::create_dir_all(&work_dir).unwrap();

    let session_id = "ping-pong-session-test";

    // Calculate the project folder path that Claude will use
    let project_folder = format!("-{}", work_dir.display().to_string().replace('/', "-"));
    let project_path = server.mock.projects_dir.join(&project_folder);
    fs::create_dir_all(&project_path).unwrap();

    // Path where the session file should be written
    let session_file_path = project_path.join(format!("{session_id}.jsonl"));

    // Create the session content that simulates the ping-pong conversation
    let session_content = format!(
        r#"{{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"{}","sessionId":"{}","version":"1.0.108","gitBranch":"master","type":"user","message":{{"role":"user","content":"ping"}},"uuid":"0136315c-067d-410c-abdd-95aeadcf7e82","timestamp":"2025-09-20T09:39:07.234Z"}}
{{"parentUuid":"0136315c-067d-410c-abdd-95aeadcf7e82","isSidechain":false,"userType":"external","cwd":"{}","sessionId":"{}","version":"1.0.108","gitBranch":"master","message":{{"id":"msg_01N1Xnc57sfsbeiXaf4aGdXK","type":"message","role":"assistant","model":"claude-opus-4-1-20250805","content":[{{"type":"text","text":"pong"}}],"stop_reason":null,"stop_sequence":null,"usage":{{"input_tokens":4,"cache_creation_input_tokens":19557,"cache_read_input_tokens":0,"cache_creation":{{"ephemeral_5m_input_tokens":19557,"ephemeral_1h_input_tokens":0}},"output_tokens":5,"service_tier":"standard"}}}},"requestId":"req_011CTKRuD4pxvgMZSghPgRxH","type":"assistant","uuid":"8d68843d-4578-471c-aeaa-83a47083329b","timestamp":"2025-09-20T09:39:10.179Z"}}"#,
        work_dir.display(),
        session_id,
        work_dir.display(),
        session_id
    );

    // Create bootstrap messages that will:
    // 1. First respond with the session ID to complete the handshake
    // 2. Then use write_file control command to create the journal file
    let session_id_response = format!(r#"{{"sessionId": "{session_id}"}}"#);

    let write_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    // Create the active session via the API
    let create_request = CreateSessionRequest {
        session_id: session_id.to_string(),
        working_dir: work_dir.clone(),
        resume: false,
        bootstrap_messages: vec![
            session_id_response, // First, respond with session ID for handshake
            write_command,       // Then, write the journal file
        ],
    };

    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let create_response: CreateSessionResponse = response.json().await.unwrap();
    assert!(create_response.websocket_url.contains(session_id));

    // Give a moment for the file to be written
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Now list sessions - the active session should appear with "ping" as summary
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let list_response: ListSessionsResponse = response.json().await.unwrap();

    // Find our ping-pong session
    let ping_pong_session = list_response
        .sessions
        .iter()
        .find(|s| s.session_id == session_id)
        .expect("Ping-pong session should be in the list");

    // Verify the session is active
    assert!(
        ping_pong_session.active,
        "Session should be marked as active"
    );

    // Verify the first user message "ping" appears as the summary
    assert_eq!(
        ping_pong_session.summary,
        Some("ping".to_string()),
        "Active session should show first user message 'ping' as summary"
    );

    // Verify timestamps are present
    assert_eq!(
        ping_pong_session.earliest_message_date,
        Some("2025-09-20T09:39:07.234Z".to_string())
    );
    assert_eq!(
        ping_pong_session.latest_message_date,
        Some("2025-09-20T09:39:10.179Z".to_string())
    );
}

// ============== Edge Case Tests ==============

#[tokio::test]
#[serial]
async fn test_list_sessions_multiple_user_messages_chronological_order() {
    let server = TestServer::new().await;

    // Create session with multiple user messages with different timestamps
    let project_path = server.mock.projects_dir.join("project-multi-msg");
    fs::create_dir_all(&project_path).unwrap();

    let session_file = project_path.join("multi-msg-session.jsonl");
    let content = r#"{"sessionId": "multi-msg", "cwd": "/home/user/project", "type": "start"}
{"sessionId": "multi-msg", "type": "user", "message": {"role": "user", "content": "Third message"}, "timestamp": "2025-09-19T12:00:00Z"}
{"sessionId": "multi-msg", "type": "user", "message": {"role": "user", "content": "First message"}, "timestamp": "2025-09-19T10:00:00Z"}
{"sessionId": "multi-msg", "type": "user", "message": {"role": "user", "content": "Second message"}, "timestamp": "2025-09-19T11:00:00Z"}"#;
    fs::write(session_file, content).unwrap();

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    let session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "multi-msg")
        .expect("Session should be found");

    // Fallback summary should use the chronologically FIRST user message
    assert_eq!(
        session.summary,
        Some("First message".to_string()),
        "Should use chronologically first user message as fallback"
    );
}

#[tokio::test]
#[serial]
async fn test_list_sessions_duplicate_session_ids_across_projects() {
    let server = TestServer::new().await;

    // Create same session ID in different projects
    create_test_session_file(
        &server.mock.projects_dir,
        "project1",
        "duplicate-session",
        "/home/user/project1",
    );

    // Create another file with same session ID but different project
    let project2_path = server.mock.projects_dir.join("project2");
    fs::create_dir_all(&project2_path).unwrap();
    let session_file2 = project2_path.join("duplicate-session.jsonl");
    let content2 = r#"{"sessionId": "duplicate-session", "cwd": "/home/user/project2", "type": "start"}
{"sessionId": "duplicate-session", "type": "user", "message": {"role": "user", "content": "Different message"}, "timestamp": "2025-09-19T14:00:00Z"}"#;
    fs::write(session_file2, content2).unwrap();

    // Also add a summary in project2
    let summary_file = project2_path.join("summary.jsonl");
    let summary_content = r#"{"type":"summary","summary":"Duplicate Session Summary","leafUuid":"dup-uuid"}
{"sessionId": "duplicate-session", "uuid": "dup-uuid", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Response"}]}}"#;
    fs::write(summary_file, summary_content).unwrap();

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    // Should only have one session with merged data
    let duplicate_sessions: Vec<_> = body
        .sessions
        .iter()
        .filter(|s| s.session_id == "duplicate-session")
        .collect();

    assert_eq!(
        duplicate_sessions.len(),
        1,
        "Duplicate session IDs should be merged into one"
    );

    let merged_session = duplicate_sessions[0];
    // Should have the summary from project2
    assert_eq!(
        merged_session.summary,
        Some("Duplicate Session Summary".to_string())
    );
}

#[tokio::test]
#[serial]
async fn test_list_sessions_orphaned_summaries() {
    let server = TestServer::new().await;

    let project_path = server.mock.projects_dir.join("project-orphaned");
    fs::create_dir_all(&project_path).unwrap();

    // Create orphaned summary (points to non-existent UUID)
    let orphan_file = project_path.join("orphan.jsonl");
    let orphan_content =
        r#"{"type":"summary","summary":"Orphaned Summary","leafUuid":"non-existent-uuid"}"#;
    fs::write(orphan_file, orphan_content).unwrap();

    // Create a valid session
    create_test_session_file(
        &server.mock.projects_dir,
        "project-orphaned",
        "valid-session",
        "/home/user/valid",
    );

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    // Should only find the valid session, not crash on orphaned summary
    assert_eq!(body.sessions.len(), 1);
    assert_eq!(body.sessions[0].session_id, "valid-session");
}

#[tokio::test]
#[serial]
async fn test_list_sessions_mixed_valid_invalid_json() {
    let server = TestServer::new().await;

    let project_path = server.mock.projects_dir.join("project-mixed");
    fs::create_dir_all(&project_path).unwrap();

    let mixed_file = project_path.join("mixed-json.jsonl");
    let mixed_content = r#"{"sessionId": "mixed-session", "cwd": "/home/user/mixed", "type": "start"}
INVALID JSON LINE HERE
{"sessionId": "mixed-session", "type": "user", "message": {"role": "user", "content": "Valid message"}}
{broken json
{"sessionId": "mixed-session", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Response"}]}}"#;
    fs::write(mixed_file, mixed_content).unwrap();

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    // Should process valid lines and skip invalid ones
    let session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "mixed-session")
        .expect("Should process valid lines despite invalid ones");

    assert_eq!(
        session.summary,
        Some("Valid message".to_string()),
        "Should extract valid user message despite invalid lines"
    );
}

#[tokio::test]
#[serial]
async fn test_list_sessions_empty_user_messages() {
    let server = TestServer::new().await;

    let project_path = server.mock.projects_dir.join("project-empty");
    fs::create_dir_all(&project_path).unwrap();

    // Session with empty user message
    let empty_msg_file = project_path.join("empty-msg.jsonl");
    let empty_content = r#"{"sessionId": "empty-msg", "cwd": "/home/user/empty", "type": "start"}
{"sessionId": "empty-msg", "type": "user", "message": {"role": "user", "content": ""}}"#;
    fs::write(empty_msg_file, empty_content).unwrap();

    // Session with missing content field
    let missing_content_file = project_path.join("missing-content.jsonl");
    let missing_content = r#"{"sessionId": "missing-content", "cwd": "/home/user/missing", "type": "start"}
{"sessionId": "missing-content", "type": "user", "message": {"role": "user"}}"#;
    fs::write(missing_content_file, missing_content).unwrap();

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    // Find sessions with empty/missing content
    let empty_msg_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "empty-msg")
        .expect("Should find empty message session");

    assert_eq!(
        empty_msg_session.summary,
        Some(String::new()),
        "Empty message should result in empty summary"
    );

    let missing_content_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "missing-content")
        .expect("Should find missing content session");

    assert_eq!(
        missing_content_session.summary,
        Some("No content".to_string()),
        "Missing content should fallback to 'No content'"
    );
}

#[tokio::test]
#[serial]
async fn test_list_sessions_special_characters() {
    let server = TestServer::new().await;

    let project_path = server.mock.projects_dir.join("project-special");
    fs::create_dir_all(&project_path).unwrap();

    // Session with special characters in various fields
    let special_file = project_path.join("special-chars.jsonl");
    let special_content = r#"{"sessionId": "session-with-émojis-🎉", "cwd": "/home/user/path with spaces/项目", "type": "start"}
{"sessionId": "session-with-émojis-🎉", "type": "user", "message": {"role": "user", "content": "Hello 世界! 🌍 How's the weather?"}}
{"type":"summary","summary":"Unicode Summary: 测试 🔥","leafUuid":"special-uuid"}
{"sessionId": "session-with-émojis-🎉", "uuid": "special-uuid", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Response with émojis 🎨"}]}}"#;
    fs::write(special_file, special_content).unwrap();

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    let special_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "session-with-émojis-🎉")
        .expect("Should handle special characters in session ID");

    assert_eq!(
        special_session.summary,
        Some("Unicode Summary: 测试 🔥".to_string()),
        "Should handle Unicode and emojis in summary"
    );

    assert_eq!(
        special_session.working_directory.to_string_lossy(),
        "/home/user/path with spaces/项目",
        "Should handle spaces and Unicode in paths"
    );
}

#[tokio::test]
#[serial]
async fn test_list_sessions_missing_timestamps() {
    let server = TestServer::new().await;

    let project_path = server.mock.projects_dir.join("project-no-timestamps");
    fs::create_dir_all(&project_path).unwrap();

    // Session without any timestamps
    let no_timestamp_file = project_path.join("no-timestamps.jsonl");
    let no_timestamp_content = r#"{"sessionId": "no-timestamps", "cwd": "/home/user/project", "type": "start"}
{"sessionId": "no-timestamps", "type": "user", "message": {"role": "user", "content": "Message without timestamp"}}
{"sessionId": "no-timestamps", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Response"}]}}"#;
    fs::write(no_timestamp_file, no_timestamp_content).unwrap();

    let client = Client::new();
    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    let no_timestamp_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "no-timestamps")
        .expect("Should handle sessions without timestamps");

    // Should still have the fallback summary from first user message
    assert_eq!(
        no_timestamp_session.summary,
        Some("Message without timestamp".to_string())
    );

    // Timestamp fields should be None
    assert_eq!(no_timestamp_session.earliest_message_date, None);
    assert_eq!(no_timestamp_session.latest_message_date, None);
}

#[tokio::test]
#[serial]
async fn test_list_sessions_large_scale() {
    let server = TestServer::new().await;

    // Create 50+ sessions across multiple projects
    for i in 0..55 {
        let project_name = format!("project-{}", i % 10); // Distribute across 10 projects
        let session_id = format!("session-{i:03}");
        let cwd = format!("/home/user/project{i}");

        if i % 3 == 0 {
            // Every third session gets a summary
            let project_path = server.mock.projects_dir.join(&project_name);
            fs::create_dir_all(&project_path).unwrap();

            let summary_file = project_path.join(format!("summary-{i}.jsonl"));
            let summary_content = format!(
                r#"{{"type":"summary","summary":"Summary for session {i}","leafUuid":"uuid-{i}"}}"#
            );
            fs::write(summary_file, summary_content).unwrap();

            let session_file = project_path.join(format!("{session_id}.jsonl"));
            let session_content = format!(
                r#"{{"sessionId": "{session_id}", "cwd": "{cwd}", "type": "start"}}
{{"sessionId": "{session_id}", "type": "user", "message": {{"role": "user", "content": "Message {i}"}}}}
{{"sessionId": "{session_id}", "uuid": "uuid-{i}", "type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "Response"}}]}}}}"#
            );
            fs::write(session_file, session_content).unwrap();
        } else {
            // Others don't have summaries
            create_test_session_file(&server.mock.projects_dir, &project_name, &session_id, &cwd);
        }
    }

    let client = Client::new();
    let start = std::time::Instant::now();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    let duration = start.elapsed();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    assert_eq!(body.sessions.len(), 55, "Should handle all 55 sessions");

    // Verify mix of sessions with and without explicit summaries
    let with_summaries = body
        .sessions
        .iter()
        .filter(|s| {
            s.summary
                .as_ref()
                .is_some_and(|sum| sum.starts_with("Summary"))
        })
        .count();
    let with_fallback = body
        .sessions
        .iter()
        .filter(|s| s.summary == Some("Hello Claude".to_string()))
        .count();

    assert!(with_summaries > 0, "Should have sessions with summaries");
    assert!(
        with_fallback > 0,
        "Should have sessions with fallback summaries"
    );

    // Performance check - should complete in reasonable time
    assert!(
        duration.as_secs() < 5,
        "Large scale list should complete within 5 seconds, took {duration:?}"
    );
}
