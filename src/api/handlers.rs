use crate::discovery::SessionDiscovery;
use crate::error::OrchestratorResult;
use crate::models::{
    CreateSessionRequest, CreateSessionResponse, GetSessionResponse, ListSessionsResponse,
};
use crate::session_manager::SessionManager;
use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

#[derive(Clone)]
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub config: Arc<crate::config::Config>,
}

/// Lists all available sessions.
///
/// # Errors
///
/// Returns an error if session discovery fails or if there's an I/O error accessing session files.
#[instrument(skip(state), fields(sessions_count))]
pub async fn list_sessions(
    State(state): State<AppState>,
) -> OrchestratorResult<Json<ListSessionsResponse>> {
    info!("Listing all sessions");

    let discovery = SessionDiscovery::new(&state.config, &state.session_manager);
    let sessions = match discovery.list_all_sessions().await {
        Ok(sessions) => {
            info!(count = sessions.len(), "Successfully retrieved sessions");
            tracing::Span::current().record("sessions_count", sessions.len());
            sessions
        }
        Err(e) => {
            error!(error = %e, "Failed to list sessions");
            return Err(e);
        }
    };

    debug!("Returning sessions response");
    Ok(Json(ListSessionsResponse { sessions }))
}

/// Creates a new session or resumes an existing one.
///
/// # Errors
///
/// Returns an error if the session ID is empty, if the session manager fails to create
/// the session, or if there's an I/O error.
#[instrument(skip(state), fields(session_id = %request.session_id, working_dir = %request.working_dir.display(), resume = request.resume))]
pub async fn create_session(
    State(state): State<AppState>,
    Json(request): Json<CreateSessionRequest>,
) -> OrchestratorResult<Json<CreateSessionResponse>> {
    info!(
        session_id = %request.session_id,
        working_dir = %request.working_dir.display(),
        resume = request.resume,
        "Creating session"
    );

    // Validate request
    if request.session_id.is_empty() {
        warn!("Rejecting session creation request: empty session_id");
        return Err(crate::error::OrchestratorError::InvalidRequest(
            "session_id cannot be empty".to_string(),
        ));
    }

    if request.first_message.is_empty() {
        warn!("Rejecting session creation request: empty first_message");
        return Err(crate::error::OrchestratorError::InvalidRequest(
            "first_message cannot be empty".to_string(),
        ));
    }

    // Create or resume session
    let actual_session_id = match state
        .session_manager
        .create_session(
            request.session_id.clone(),
            &request.working_dir,
            request.resume,
            request.first_message.clone(),
        )
        .await
    {
        Ok(id) => {
            info!(
                requested_id = %request.session_id,
                actual_id = %id,
                "Session created successfully"
            );
            id
        }
        Err(e) => {
            error!(
                session_id = %request.session_id,
                working_dir = %request.working_dir.display(),
                error = %e,
                "Failed to create session"
            );
            return Err(e);
        }
    };

    // Generate WebSocket URLs
    let websocket_url = format!("/api/v1/sessions/{actual_session_id}/claude_ws");
    let approval_websocket_url =
        format!("/api/v1/sessions/{actual_session_id}/claude_approvals_ws");
    debug!(
        session_id = %actual_session_id,
        websocket_url = %websocket_url,
        approval_websocket_url = %approval_websocket_url,
        "Generated WebSocket URLs"
    );

    Ok(Json(CreateSessionResponse {
        session_id: actual_session_id,
        websocket_url,
        approval_websocket_url,
    }))
}

/// Gets information about a specific session including its content.
///
/// # Errors
///
/// Returns an error if the session is not found or if there's an I/O error accessing
/// the session data.
#[instrument(skip(state), fields(session_id = %session_id))]
pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> OrchestratorResult<Json<GetSessionResponse>> {
    info!(session_id = %session_id, "Getting session details");

    let discovery = SessionDiscovery::new(&state.config, &state.session_manager);
    let (session_info, content) = match discovery.get_session_content(&session_id).await {
        Ok((info, content)) => {
            info!(
                session_id = %session_id,
                active = info.active,
                content_entries = content.len(),
                working_dir = %info.working_directory.display(),
                "Successfully retrieved session content"
            );
            (info, content)
        }
        Err(e) => {
            error!(
                session_id = %session_id,
                error = %e,
                "Failed to get session content"
            );
            return Err(e);
        }
    };

    let (websocket_url, approval_websocket_url) = if session_info.active {
        let ws_url = format!("/api/v1/sessions/{session_id}/claude_ws");
        let approval_url = format!("/api/v1/sessions/{session_id}/claude_approvals_ws");
        debug!(
            session_id = %session_id,
            websocket_url = %ws_url,
            approval_websocket_url = %approval_url,
            "Session is active, providing WebSocket URLs"
        );
        (Some(ws_url), Some(approval_url))
    } else {
        debug!(session_id = %session_id, "Session is inactive, no WebSocket URLs");
        (None, None)
    };

    Ok(Json(GetSessionResponse {
        session_id: session_info.session_id,
        working_directory: session_info.working_directory,
        content,
        websocket_url,
        approval_websocket_url,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use serial_test::serial;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_state(temp_dir: &TempDir) -> AppState {
        // Create mock Claude binary
        let claude_path = temp_dir.path().join("mock_claude");
        let script = r#"#!/bin/bash
# Parse arguments to find session ID
SESSION_ID=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --session-id)
            SESSION_ID="$2"
            shift 2
            ;;
        --resume)
            SESSION_ID="$2"
            shift 2
            ;;
        *)
            shift
            ;;
    esac
done

# Use current working directory and CLAUDE_PROJECTS_DIR environment variable
WORKING_DIR="$(pwd)"
PROJECTS_DIR="${CLAUDE_PROJECTS_DIR}"

if [ -z "$PROJECTS_DIR" ]; then
    echo "Error: CLAUDE_PROJECTS_DIR environment variable not set" >&2
    exit 1
fi

# Create project subdirectory based on working directory path
# Use tr to replace path separators for better portability
ENCODED_DIR=$(echo "$WORKING_DIR" | tr '/\\:' '___')
PROJECT_SUBDIR="$PROJECTS_DIR/$ENCODED_DIR"

# Create directory if it doesn't exist
mkdir -p "$PROJECT_SUBDIR"

# Create session file with initial content
SESSION_FILE="$PROJECT_SUBDIR/$SESSION_ID.jsonl"
echo "{\"sessionId\": \"$SESSION_ID\", \"cwd\": \"/tmp/test_work_dir\", \"type\": \"start\"}" > "$SESSION_FILE"

# Output initial response
echo "{\"sessionId\": \"$SESSION_ID\", \"type\": \"start\"}"

# Process input lines (if any) - output to stdout but don't modify session file
while read line; do
    echo "{\"type\": \"echo\", \"content\": \"$line\"}"
done
"#;
        fs::write(&claude_path, script).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&claude_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&claude_path, perms).unwrap();
        }

        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        let config = Config {
            claude_binary_path: claude_path,
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(1),
        };

        AppState {
            session_manager: Arc::new(SessionManager::new(config.clone())),
            config: Arc::new(config),
        }
    }

    #[tokio::test]
    async fn test_list_empty_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_state(&temp_dir);

        let result = list_sessions(State(state)).await.unwrap();
        assert_eq!(result.0.sessions.len(), 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_session_success() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_state(&temp_dir);

        // Set environment variable for the mock Claude binary
        std::env::set_var(
            "CLAUDE_PROJECTS_DIR",
            state.config.claude_projects_dir.to_str().unwrap(),
        );

        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        let request = CreateSessionRequest {
            session_id: "test-session".to_string(),
            working_dir: working_dir.clone(),
            resume: false,
            first_message: vec![r#"{"role": "user", "content": "Hello"}"#.to_string()],
        };

        let result = create_session(State(state.clone()), Json(request))
            .await
            .unwrap();

        assert_eq!(result.0.session_id, "test-session");
        assert_eq!(
            result.0.websocket_url,
            "/api/v1/sessions/test-session/claude_ws"
        );
        assert_eq!(
            result.0.approval_websocket_url,
            "/api/v1/sessions/test-session/claude_approvals_ws"
        );

        // Verify session is in the list
        let list_result = list_sessions(State(state)).await.unwrap();
        assert_eq!(list_result.0.sessions.len(), 1);
        assert_eq!(list_result.0.sessions[0].session_id, "test-session");
        assert!(list_result.0.sessions[0].active);

        // Clean up environment variable
        std::env::remove_var("CLAUDE_PROJECTS_DIR");
    }

    #[tokio::test]
    async fn test_create_session_empty_id() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_state(&temp_dir);

        let request = CreateSessionRequest {
            session_id: String::new(),
            working_dir: PathBuf::from("/tmp"),
            resume: false,
            first_message: vec![r#"{"role": "user", "content": "Hello"}"#.to_string()],
        };

        let result = create_session(State(state), Json(request)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_session_empty_first_message() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_state(&temp_dir);

        let request = CreateSessionRequest {
            session_id: "test-session".to_string(),
            working_dir: PathBuf::from("/tmp"),
            resume: false,
            first_message: vec![],
        };

        let result = create_session(State(state), Json(request)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_session_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_state(&temp_dir);

        let result = get_session(State(state), Path("non-existent".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_active_session() {
        let temp_dir = TempDir::new().unwrap();
        let state = create_test_state(&temp_dir);

        // Set environment variable for the mock Claude binary
        std::env::set_var(
            "CLAUDE_PROJECTS_DIR",
            state.config.claude_projects_dir.to_str().unwrap(),
        );

        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        // Create session first
        let request = CreateSessionRequest {
            session_id: "testsession".to_string(),
            working_dir: working_dir.clone(),
            resume: false,
            first_message: vec![r#"{"role": "user", "content": "Hello"}"#.to_string()],
        };

        let _ = create_session(State(state.clone()), Json(request))
            .await
            .unwrap();

        // Now get the session
        let result = get_session(State(state), Path("testsession".to_string()))
            .await
            .unwrap();

        assert_eq!(result.0.session_id, "testsession");
        assert_eq!(
            result.0.websocket_url,
            Some("/api/v1/sessions/testsession/claude_ws".to_string())
        );
        assert_eq!(
            result.0.approval_websocket_url,
            Some("/api/v1/sessions/testsession/claude_approvals_ws".to_string())
        );

        // Clean up environment variable
        std::env::remove_var("CLAUDE_PROJECTS_DIR");
    }
}
