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
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{debug, info};

// Helper function to create test session files on disk
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

/// Test that active sessions return the correct working_directory
#[tokio::test]
#[serial]
async fn test_active_session_working_directory() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create multiple working directories
    let work_dir1 = server.mock.temp_dir.path().join("project1");
    let work_dir2 = server.mock.temp_dir.path().join("project2");
    fs::create_dir_all(&work_dir1).unwrap();
    fs::create_dir_all(&work_dir2).unwrap();

    info!("Created test directories: {:?}, {:?}", work_dir1, work_dir2);

    // Create first session with control command to create session file
    let session_file_path1 = server.mock.projects_dir.join("active-session-1.jsonl");
    let session_content1 = format!(
        r#"{{"sessionId": "active-session-1", "cwd": "{}", "type": "start"}}"#,
        work_dir1.display()
    );
    let create_file_command1 = serde_json::json!({
        "control": "write_file",
        "path": session_file_path1.to_string_lossy(),
        "content": session_content1
    })
    .to_string();

    let request1 = CreateSessionRequest {
        session_id: "active-session-1".to_string(),
        working_dir: work_dir1.clone(),
        resume: false,
        first_message: vec![
            create_file_command1,
            r#"{"role": "user", "content": "Test message 1"}"#.to_string(),
        ],
    };

    let response1 = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request1)
        .send()
        .await
        .unwrap();

    assert_eq!(response1.status(), 200);
    debug!("Created first active session");

    // Create second session with control command to create session file
    let session_file_path2 = server.mock.projects_dir.join("active-session-2.jsonl");
    let session_content2 = format!(
        r#"{{"sessionId": "active-session-2", "cwd": "{}", "type": "start"}}"#,
        work_dir2.display()
    );
    let create_file_command2 = serde_json::json!({
        "control": "write_file",
        "path": session_file_path2.to_string_lossy(),
        "content": session_content2
    })
    .to_string();

    let request2 = CreateSessionRequest {
        session_id: "active-session-2".to_string(),
        working_dir: work_dir2.clone(),
        resume: false,
        first_message: vec![
            create_file_command2,
            r#"{"role": "user", "content": "Test message 2"}"#.to_string(),
        ],
    };

    let response2 = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&request2)
        .send()
        .await
        .unwrap();

    assert_eq!(response2.status(), 200);
    debug!("Created second active session");

    // Test GET session for first session
    let get_response1 = client
        .get(format!(
            "{}/api/v1/sessions/active-session-1",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response1.status(), 200);

    let session1: GetSessionResponse = get_response1.json().await.unwrap();
    assert_eq!(session1.session_id, "active-session-1");
    assert_eq!(session1.working_directory, work_dir1);
    assert!(session1.websocket_url.is_some());
    assert!(session1.approval_websocket_url.is_some());
    debug!(
        "Verified first session working directory: {:?}",
        session1.working_directory
    );

    // Test GET session for second session
    let get_response2 = client
        .get(format!(
            "{}/api/v1/sessions/active-session-2",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response2.status(), 200);

    let session2: GetSessionResponse = get_response2.json().await.unwrap();
    assert_eq!(session2.session_id, "active-session-2");
    assert_eq!(session2.working_directory, work_dir2);
    assert!(session2.websocket_url.is_some());
    assert!(session2.approval_websocket_url.is_some());
    debug!(
        "Verified second session working directory: {:?}",
        session2.working_directory
    );

    // Verify they have different working directories
    assert_ne!(session1.working_directory, session2.working_directory);
    info!("Successfully verified different working directories for active sessions");
}

/// Test that inactive sessions (read from disk) return the correct working_directory
#[tokio::test]
#[serial]
async fn test_inactive_session_working_directory_from_disk() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create test working directories
    let work_dir1 = "/home/user/project1";
    let work_dir2 = "/home/user/project2";
    let work_dir3 = "/tmp/workspace";

    // Create session files on disk (these will be inactive)
    create_test_session_file(
        &server.mock.projects_dir,
        "project1",
        "disk-session-1",
        work_dir1,
    );
    create_test_session_file(
        &server.mock.projects_dir,
        "project2",
        "disk-session-2",
        work_dir2,
    );
    create_test_session_file(
        &server.mock.projects_dir,
        "workspace",
        "disk-session-3",
        work_dir3,
    );

    debug!("Created test session files on disk");

    // Test GET session for first disk session
    let get_response1 = client
        .get(format!(
            "{}/api/v1/sessions/disk-session-1",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response1.status(), 200);

    let session1: GetSessionResponse = get_response1.json().await.unwrap();
    assert_eq!(session1.session_id, "disk-session-1");
    assert_eq!(session1.working_directory, PathBuf::from(work_dir1));
    assert!(session1.websocket_url.is_none()); // Should be None for inactive sessions
    assert!(session1.approval_websocket_url.is_none()); // Should be None for inactive sessions
    assert!(!session1.content.is_empty()); // Should have content from file
    debug!(
        "Verified first disk session working directory: {:?}",
        session1.working_directory
    );

    // Test GET session for second disk session
    let get_response2 = client
        .get(format!(
            "{}/api/v1/sessions/disk-session-2",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response2.status(), 200);

    let session2: GetSessionResponse = get_response2.json().await.unwrap();
    assert_eq!(session2.session_id, "disk-session-2");
    assert_eq!(session2.working_directory, PathBuf::from(work_dir2));
    assert!(session2.websocket_url.is_none());
    assert!(session2.approval_websocket_url.is_none());
    assert!(!session2.content.is_empty());
    debug!(
        "Verified second disk session working directory: {:?}",
        session2.working_directory
    );

    // Test GET session for third disk session
    let get_response3 = client
        .get(format!(
            "{}/api/v1/sessions/disk-session-3",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response3.status(), 200);

    let session3: GetSessionResponse = get_response3.json().await.unwrap();
    assert_eq!(session3.session_id, "disk-session-3");
    assert_eq!(session3.working_directory, PathBuf::from(work_dir3));
    assert!(session3.websocket_url.is_none());
    assert!(session3.approval_websocket_url.is_none());
    assert!(!session3.content.is_empty());
    debug!(
        "Verified third disk session working directory: {:?}",
        session3.working_directory
    );

    // Verify all sessions have different working directories
    assert_ne!(session1.working_directory, session2.working_directory);
    assert_ne!(session1.working_directory, session3.working_directory);
    assert_ne!(session2.working_directory, session3.working_directory);

    info!("Successfully verified different working directories for inactive sessions from disk");
}

/// Test that session resume uses the correct working directory
#[tokio::test]
#[serial]
async fn test_session_resume_working_directory() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create working directory for initial session
    let original_work_dir = server.mock.temp_dir.path().join("resume_test");
    fs::create_dir_all(&original_work_dir).unwrap();

    // Create an existing session file on disk with specific working directory
    let original_cwd = original_work_dir.to_str().unwrap();
    create_test_session_file(
        &server.mock.projects_dir,
        "resume_test",
        "original-session",
        original_cwd,
    );

    debug!(
        "Created original session file for resume test with working directory: {}",
        original_cwd
    );

    // Resume the session - create control commands for resume mode
    let new_session_id = "resumed-session-123";
    let session_file_path = server
        .mock
        .projects_dir
        .join(format!("{}.jsonl", new_session_id));
    let session_content = format!(
        r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}"#,
        new_session_id,
        original_work_dir.display()
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

    let resume_request = CreateSessionRequest {
        session_id: "original-session".to_string(),
        working_dir: original_work_dir.clone(),
        resume: true,
        first_message: vec![
            create_file_command,
            session_init_response,
            r#"{"role": "user", "content": "Resume this session"}"#.to_string(),
        ],
    };

    let resume_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&resume_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resume_response.status(), 200);

    let resume_body: CreateSessionResponse = resume_response.json().await.unwrap();
    assert!(resume_body.session_id.starts_with("resumed-")); // Should get a new session ID
    debug!("Session resumed with new ID: {}", resume_body.session_id);

    // Get the resumed session and verify working directory
    let get_response = client
        .get(format!(
            "{}/api/v1/sessions/{}",
            server.base_url, &resume_body.session_id
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), 200);

    let session: GetSessionResponse = get_response.json().await.unwrap();
    assert_eq!(session.session_id, resume_body.session_id);
    assert_eq!(session.working_directory, original_work_dir); // Should use original working directory
    assert!(session.websocket_url.is_some()); // Should be active
    assert!(session.approval_websocket_url.is_some()); // Should be active

    info!(
        "Successfully verified resumed session uses correct working directory: {:?}",
        session.working_directory
    );
}

/// Test that LIST sessions returns correct working directories for all sessions
#[tokio::test]
#[serial]
async fn test_list_sessions_working_directories() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create active session with control command to create session file
    let active_work_dir = server.mock.temp_dir.path().join("active_list_test");
    fs::create_dir_all(&active_work_dir).unwrap();

    let session_file_path = server.mock.projects_dir.join("active-list-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "active-list-session", "cwd": "{}", "type": "start"}}"#,
        active_work_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let active_request = CreateSessionRequest {
        session_id: "active-list-session".to_string(),
        working_dir: active_work_dir.clone(),
        resume: false,
        first_message: vec![
            create_file_command,
            r#"{"role": "user", "content": "Active session"}"#.to_string(),
        ],
    };

    client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&active_request)
        .send()
        .await
        .unwrap();

    // Create inactive sessions on disk
    create_test_session_file(
        &server.mock.projects_dir,
        "list_project1",
        "inactive-list-1",
        "/home/user/list1",
    );
    create_test_session_file(
        &server.mock.projects_dir,
        "list_project2",
        "inactive-list-2",
        "/home/user/list2",
    );

    debug!("Created active and inactive sessions for list test");

    // List all sessions
    let list_response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(list_response.status(), 200);

    let list_body: ListSessionsResponse = list_response.json().await.unwrap();
    assert_eq!(list_body.sessions.len(), 3);

    // Verify each session has the correct working directory
    for session_info in &list_body.sessions {
        match session_info.session_id.as_str() {
            "active-list-session" => {
                assert_eq!(session_info.working_directory, active_work_dir);
                assert!(session_info.active);
                debug!(
                    "Verified active session in list: {:?}",
                    session_info.working_directory
                );
            }
            "inactive-list-1" => {
                assert_eq!(
                    session_info.working_directory,
                    PathBuf::from("/home/user/list1")
                );
                assert!(!session_info.active);
                debug!(
                    "Verified inactive session 1 in list: {:?}",
                    session_info.working_directory
                );
            }
            "inactive-list-2" => {
                assert_eq!(
                    session_info.working_directory,
                    PathBuf::from("/home/user/list2")
                );
                assert!(!session_info.active);
                debug!(
                    "Verified inactive session 2 in list: {:?}",
                    session_info.working_directory
                );
            }
            _ => panic!("Unexpected session ID: {}", session_info.session_id),
        }
    }

    info!("Successfully verified working directories in session list");
}

/// Test mixed scenario: active and inactive sessions with various working directories
#[tokio::test]
#[serial]
async fn test_mixed_active_inactive_working_directories() {
    let server = TestServer::new().await;
    let client = Client::new();

    // Create various working directories
    let web_project = server.mock.temp_dir.path().join("web-app");
    let api_project = server.mock.temp_dir.path().join("backend-api");
    fs::create_dir_all(&web_project).unwrap();
    fs::create_dir_all(&api_project).unwrap();

    // Create active sessions with control commands to create session files
    let web_session_file_path = server.mock.projects_dir.join("web-session.jsonl");
    let web_session_content = format!(
        r#"{{"sessionId": "web-session", "cwd": "{}", "type": "start"}}"#,
        web_project.display()
    );
    let web_create_file_command = serde_json::json!({
        "control": "write_file",
        "path": web_session_file_path.to_string_lossy(),
        "content": web_session_content
    })
    .to_string();

    let api_session_file_path = server.mock.projects_dir.join("api-session.jsonl");
    let api_session_content = format!(
        r#"{{"sessionId": "api-session", "cwd": "{}", "type": "start"}}"#,
        api_project.display()
    );
    let api_create_file_command = serde_json::json!({
        "control": "write_file",
        "path": api_session_file_path.to_string_lossy(),
        "content": api_session_content
    })
    .to_string();

    let web_request = CreateSessionRequest {
        session_id: "web-session".to_string(),
        working_dir: web_project.clone(),
        resume: false,
        first_message: vec![
            web_create_file_command,
            r#"{"role": "user", "content": "Working on web frontend"}"#.to_string(),
        ],
    };

    let api_request = CreateSessionRequest {
        session_id: "api-session".to_string(),
        working_dir: api_project.clone(),
        resume: false,
        first_message: vec![
            api_create_file_command,
            r#"{"role": "user", "content": "Working on backend API"}"#.to_string(),
        ],
    };

    // Create the active sessions
    client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&web_request)
        .send()
        .await
        .unwrap();

    client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&api_request)
        .send()
        .await
        .unwrap();

    // Create inactive sessions on disk
    create_test_session_file(
        &server.mock.projects_dir,
        "mobile",
        "mobile-session",
        "/home/user/mobile-app",
    );
    create_test_session_file(
        &server.mock.projects_dir,
        "scripts",
        "script-session",
        "/usr/local/scripts",
    );

    debug!("Created mixed active and inactive sessions");

    // Test each session individually
    let test_cases = vec![
        ("web-session", web_project.clone(), true),
        ("api-session", api_project.clone(), true),
        (
            "mobile-session",
            PathBuf::from("/home/user/mobile-app"),
            false,
        ),
        ("script-session", PathBuf::from("/usr/local/scripts"), false),
    ];

    for (session_id, expected_dir, should_be_active) in test_cases {
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
        assert_eq!(session.working_directory, expected_dir);

        if should_be_active {
            assert!(session.websocket_url.is_some());
            assert!(session.approval_websocket_url.is_some());
        } else {
            assert!(session.websocket_url.is_none());
            assert!(session.approval_websocket_url.is_none());
            assert!(!session.content.is_empty());
        }

        debug!(
            "Verified session {} working directory: {:?} (active: {})",
            session_id, session.working_directory, should_be_active
        );
    }

    info!(
        "Successfully verified mixed active/inactive sessions with different working directories"
    );
}

/// Test error handling: session not found
#[tokio::test]
#[serial]
async fn test_working_directory_session_not_found() {
    let server = TestServer::new().await;
    let client = Client::new();

    let get_response = client
        .get(format!(
            "{}/api/v1/sessions/non-existent-session",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_response.status(), 404);
    debug!("Correctly returned 404 for non-existent session");
}

/// Test working directory consistency across session lifecycle
#[tokio::test]
#[serial]
async fn test_working_directory_consistency_across_lifecycle() {
    let server = TestServer::new().await;
    let client = Client::new();

    let work_dir = server.mock.temp_dir.path().join("consistency_test");
    fs::create_dir_all(&work_dir).unwrap();

    // 1. Create session with control command to create session file
    let session_file_path = server.mock.projects_dir.join("lifecycle-session.jsonl");
    let session_content = format!(
        r#"{{"sessionId": "lifecycle-session", "cwd": "{}", "type": "start"}}"#,
        work_dir.display()
    );
    let create_file_command = serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": session_content
    })
    .to_string();

    let create_request = CreateSessionRequest {
        session_id: "lifecycle-session".to_string(),
        working_dir: work_dir.clone(),
        resume: false,
        first_message: vec![
            create_file_command,
            r#"{"role": "user", "content": "Starting lifecycle test"}"#.to_string(),
        ],
    };

    let create_response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(create_response.status(), 200);

    // 2. Get session while active
    let get_active_response = client
        .get(format!(
            "{}/api/v1/sessions/lifecycle-session",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_active_response.status(), 200);
    let active_session: GetSessionResponse = get_active_response.json().await.unwrap();
    assert_eq!(active_session.working_directory, work_dir);
    assert!(active_session.websocket_url.is_some());

    debug!(
        "Verified working directory for active session: {:?}",
        active_session.working_directory
    );

    // 3. Shutdown session manager to make session inactive
    server.session_manager.shutdown().await;

    // Give time for shutdown
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 4. Get session after it becomes inactive (should read from disk)
    let get_inactive_response = client
        .get(format!(
            "{}/api/v1/sessions/lifecycle-session",
            server.base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(get_inactive_response.status(), 200);
    let inactive_session: GetSessionResponse = get_inactive_response.json().await.unwrap();
    assert_eq!(inactive_session.working_directory, work_dir); // Should be the same
    assert!(inactive_session.websocket_url.is_none()); // Should be None now

    debug!(
        "Verified working directory for inactive session: {:?}",
        inactive_session.working_directory
    );

    // Verify working directory is consistent
    assert_eq!(
        active_session.working_directory,
        inactive_session.working_directory
    );

    info!("Successfully verified working directory consistency across session lifecycle");
}
