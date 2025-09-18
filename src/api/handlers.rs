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

    if request.bootstrap_messages.is_empty() {
        warn!("Rejecting session creation request: empty bootstrap_messages");
        return Err(crate::error::OrchestratorError::InvalidRequest(
            "bootstrap_messages cannot be empty".to_string(),
        ));
    }

    // Create or resume session
    let actual_session_id = match state
        .session_manager
        .create_session(
            request.session_id.clone(),
            &request.working_dir,
            request.resume,
            request.bootstrap_messages.clone(),
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
