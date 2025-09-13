use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Working directory invalid: {0}")]
    WorkingDirInvalid(String),

    #[error("Failed to spawn Claude process: {0}")]
    ClaudeSpawnFailed(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Failed to read directory: {0}")]
    #[allow(dead_code)] // Used by public API error handling
    DirectoryReadError(String),

    #[error("Failed to parse file: {0}")]
    FileParseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("WebSocket error: {0}")]
    #[allow(dead_code)] // Used by WebSocket broadcasting API
    WebSocketError(String),

    #[error("Process communication error: {0}")]
    ProcessCommunicationError(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
}

impl OrchestratorError {
    const fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "INVALID_REQUEST",
            Self::WorkingDirInvalid(_) => "WORKING_DIR_INVALID",
            Self::ClaudeSpawnFailed(_) => "CLAUDE_SPAWN_FAILED",
            Self::SessionNotFound(_) => "SESSION_NOT_FOUND",
            Self::DirectoryReadError(_) => "DIRECTORY_READ_ERROR",
            Self::FileParseError(_) => "FILE_PARSE_ERROR",
            Self::InternalError(_) => "INTERNAL_ERROR",
            Self::WebSocketError(_) => "WEBSOCKET_ERROR",
            Self::ProcessCommunicationError(_) => "PROCESS_COMMUNICATION_ERROR",
        }
    }

    const fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidRequest(_) | Self::WorkingDirInvalid(_) | Self::FileParseError(_) => StatusCode::BAD_REQUEST,
            Self::SessionNotFound(_) => StatusCode::NOT_FOUND,
            Self::ClaudeSpawnFailed(_) | Self::DirectoryReadError(_) | Self::InternalError(_) | Self::WebSocketError(_) | Self::ProcessCommunicationError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for OrchestratorError {
    fn into_response(self) -> Response {
        let error_response = ErrorResponse {
            error: self.to_string(),
            code: self.error_code().to_string(),
        };

        (self.status_code(), Json(error_response)).into_response()
    }
}

// Helper type for Results
pub type OrchestratorResult<T> = Result<T, OrchestratorError>;

// Conversion from anyhow::Error to OrchestratorError
impl From<anyhow::Error> for OrchestratorError {
    fn from(err: anyhow::Error) -> Self {
        Self::InternalError(err.to_string())
    }
}

// Conversion from std::io::Error
impl From<std::io::Error> for OrchestratorError {
    fn from(err: std::io::Error) -> Self {
        Self::InternalError(err.to_string())
    }
}

// Conversion from serde_json::Error
impl From<serde_json::Error> for OrchestratorError {
    fn from(err: serde_json::Error) -> Self {
        Self::FileParseError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(
            OrchestratorError::InvalidRequest("test".to_string()).error_code(),
            "INVALID_REQUEST"
        );
        assert_eq!(
            OrchestratorError::SessionNotFound("test".to_string()).error_code(),
            "SESSION_NOT_FOUND"
        );
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            OrchestratorError::InvalidRequest("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            OrchestratorError::SessionNotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            OrchestratorError::InternalError("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
