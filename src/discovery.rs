use crate::config::Config;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::models::{SessionFileLine, SessionInfo};
use crate::session_manager::SessionManager;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tracing::{error, instrument, warn};
use walkdir::WalkDir;

pub struct SessionDiscovery<'a> {
    config: &'a Config,
    session_manager: &'a SessionManager,
}

impl<'a> SessionDiscovery<'a> {
    #[must_use]
    pub const fn new(config: &'a Config, session_manager: &'a SessionManager) -> Self {
        Self {
            config,
            session_manager,
        }
    }

    /// Lists all available sessions, both active and inactive.
    ///
    /// # Errors
    ///
    /// Returns an error if there's an I/O error scanning the disk for session files
    /// or if session files cannot be parsed.
    #[instrument(skip(self))]
    pub async fn list_all_sessions(&self) -> OrchestratorResult<Vec<SessionInfo>> {
        let mut sessions = Vec::new();

        // Get active sessions from session manager
        let active_sessions = self.session_manager.get_active_sessions().await;

        // Collect active session IDs (need to await each one)
        let mut active_session_ids = Vec::new();
        for session in &active_sessions {
            active_session_ids.push(session.get_id().await);
        }

        // Scan disk for all sessions
        let disk_sessions = self.scan_disk_for_sessions();

        // Merge disk sessions with active status
        for mut session in disk_sessions {
            session.active = active_session_ids.contains(&session.session_id);
            sessions.push(session);
        }

        // Add any active sessions not found on disk (shouldn't normally happen)
        for active_session in active_sessions {
            let session_id = active_session.get_id().await;
            if !sessions.iter().any(|s| s.session_id == session_id) {
                warn!(
                    session_id = %session_id,
                    working_dir = %active_session.working_dir.display(),
                    "Active session not found on disk - adding virtual entry"
                );
                sessions.push(SessionInfo {
                    session_id,
                    working_directory: active_session.working_dir.clone(),
                    active: true,
                    summary: None,
                    earliest_message_date: None,
                    latest_message_date: None,
                });
            }
        }

        Ok(sessions)
    }

    /// Gets detailed information and content for a specific session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not found or if there's an I/O error
    /// reading the session file.
    pub async fn get_session_content(
        &self,
        session_id: &str,
    ) -> OrchestratorResult<(SessionInfo, Vec<serde_json::Value>)> {
        // First check if session is active
        if let Some(session) = self.session_manager.get_session(session_id) {
            // Try to get enhanced session info from disk parsing
            let disk_session_info = self
                .find_session_on_disk(session_id)
                .ok()
                .map(|(info, _)| info);

            let session_info = SessionInfo {
                session_id: session.get_id().await,
                working_directory: session.working_dir.clone(),
                active: session.is_active().await,
                summary: disk_session_info
                    .as_ref()
                    .and_then(|info| info.summary.clone()),
                earliest_message_date: disk_session_info
                    .as_ref()
                    .and_then(|info| info.earliest_message_date.clone()),
                latest_message_date: disk_session_info
                    .as_ref()
                    .and_then(|info| info.latest_message_date.clone()),
            };

            // Try to read content from disk
            let content = self.read_session_content_from_disk(session_id)?;

            return Ok((session_info, content));
        }

        // Not active, search on disk
        self.find_session_on_disk(session_id)
    }

    fn scan_disk_for_sessions(&self) -> Vec<SessionInfo> {
        let mut sessions = Vec::new();

        for entry in WalkDir::new(&self.config.claude_projects_dir)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();

            // Look for .jsonl files
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                match Self::parse_session_file(path) {
                    Ok(Some(session_info)) => {
                        sessions.push(session_info);
                    }
                    Ok(None) => {
                        // File parsed successfully but no session info found, skip
                    }
                    Err(_e) => {
                        // Skip files that can't be parsed - continue processing other files
                    }
                }
            }
        }

        sessions
    }

    fn find_session_on_disk(
        &self,
        session_id: &str,
    ) -> OrchestratorResult<(SessionInfo, Vec<serde_json::Value>)> {
        let filename = format!("{session_id}.jsonl");

        for entry in WalkDir::new(&self.config.claude_projects_dir)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();

            if path.file_name().and_then(|n| n.to_str()) == Some(&filename) {
                match Self::parse_session_file(path) {
                    Ok(Some(session_info)) => {
                        if session_info.session_id == session_id {
                            let content = Self::read_session_content(path)?;
                            return Ok((session_info, content));
                        }
                    }
                    Ok(None) => {
                        // File parsed successfully but no session info found, continue looking
                    }
                    Err(_e) => {
                        // Continue looking for other files with the same name
                    }
                }
            }
        }

        Err(OrchestratorError::SessionNotFound(session_id.to_string()))
    }

    fn read_session_content_from_disk(
        &self,
        session_id: &str,
    ) -> OrchestratorResult<Vec<serde_json::Value>> {
        let filename = format!("{session_id}.jsonl");

        for entry in WalkDir::new(&self.config.claude_projects_dir)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();

            if path.file_name().and_then(|n| n.to_str()) == Some(&filename) {
                return Self::read_session_content(path);
            }
        }

        Ok(Vec::new())
    }

    #[allow(clippy::too_many_lines)]
    #[instrument(skip_all, fields(file_path = %path.display()))]
    fn parse_session_file(path: &Path) -> OrchestratorResult<Option<SessionInfo>> {
        let file = File::open(path).map_err(|e| {
            error!(
                file_path = %path.display(),
                error = %e,
                "Failed to open session file"
            );
            OrchestratorError::FileParseError(format!(
                "Failed to open file {}: {e}",
                path.display()
            ))
        })?;

        let reader = BufReader::new(file);
        let mut session_id: Option<String> = None;
        let mut working_dir: Option<PathBuf> = None;
        let mut summary: Option<String> = None;
        let mut earliest_timestamp: Option<String> = None;
        let mut latest_timestamp: Option<String> = None;
        // Extract session ID from filename
        let file_session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                error!(file_path = %path.display(), "Invalid filename - cannot extract session ID");
                OrchestratorError::FileParseError(format!("Invalid filename: {}", path.display()))
            })?
            .to_string();

        for (line_number, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| {
                error!(
                    file_path = %path.display(),
                    line_number = line_number + 1,
                    error = %e,
                    "Failed to read line from session file"
                );
                OrchestratorError::FileParseError(format!(
                    "Failed to read line {} from {}: {e}",
                    line_number + 1,
                    path.display()
                ))
            })?;

            // Try to parse as a general JSON value for summary and timestamp extraction
            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(json) => {
                    // Check for summary entry
                    if let Some(entry_type) = json.get("type").and_then(|v| v.as_str()) {
                        if entry_type == "summary" {
                            if let Some(summary_text) = json.get("summary").and_then(|v| v.as_str())
                            {
                                summary = Some(summary_text.to_string());
                            }
                        }
                    }

                    // Check for timestamp
                    if let Some(timestamp) = json.get("timestamp").and_then(|v| v.as_str()) {
                        let timestamp_str = timestamp.to_string();

                        if earliest_timestamp.is_none()
                            || Some(&timestamp_str) < earliest_timestamp.as_ref()
                        {
                            earliest_timestamp = Some(timestamp_str.clone());
                        }

                        if latest_timestamp.is_none()
                            || Some(&timestamp_str) > latest_timestamp.as_ref()
                        {
                            latest_timestamp = Some(timestamp_str.clone());
                        }
                    }
                }
                Err(_) => {
                    // Line is not valid JSON, skip it
                    continue;
                }
            }

            // Also try to parse as SessionFileLine for sessionId and cwd
            match serde_json::from_str::<SessionFileLine>(&line) {
                Ok(parsed) => {
                    if let Some(id) = parsed.session_id {
                        session_id = Some(id);
                    }
                    if let Some(cwd) = parsed.cwd {
                        working_dir = Some(cwd);
                    }
                }
                Err(_e) => {
                    // Failed to parse as SessionFileLine - this may be normal for non-metadata lines
                }
            }
        }

        // Validate session ID matches filename
        match (session_id, working_dir) {
            (Some(id), Some(dir)) => {
                if id != file_session_id {
                    error!(
                        file_path = %path.display(),
                        filename_session_id = %file_session_id,
                        content_session_id = %id,
                        "Session ID mismatch between filename and file content"
                    );
                    return Err(OrchestratorError::FileParseError(format!(
                        "Session ID mismatch in {}: filename '{}' contains session ID '{}'",
                        path.display(),
                        file_session_id,
                        id
                    )));
                }
                Ok(Some(SessionInfo {
                    session_id: id,
                    working_directory: dir,
                    active: false,
                    summary,
                    earliest_message_date: earliest_timestamp,
                    latest_message_date: latest_timestamp,
                }))
            }
            (None, _) => Err(OrchestratorError::FileParseError(format!(
                "Missing sessionId in file {}",
                path.display()
            ))),
            (_, None) => Err(OrchestratorError::FileParseError(format!(
                "Missing cwd in file {}",
                path.display()
            ))),
        }
    }

    #[instrument(skip_all, fields(file_path = %path.display()))]
    fn read_session_content(path: &Path) -> OrchestratorResult<Vec<serde_json::Value>> {
        let file = File::open(path).map_err(|e| {
            error!(
                file_path = %path.display(),
                error = %e,
                "Failed to open session content file"
            );
            OrchestratorError::FileParseError(format!(
                "Failed to open file {}: {e}",
                path.display()
            ))
        })?;

        let reader = BufReader::new(file);
        let mut content = Vec::new();
        for (line_number, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| {
                error!(
                    file_path = %path.display(),
                    line_number = line_number + 1,
                    error = %e,
                    "Failed to read line from session content file"
                );
                OrchestratorError::FileParseError(format!(
                    "Failed to read line {} from {}: {e}",
                    line_number + 1,
                    path.display()
                ))
            })?;

            // Parse line as JSON
            let value: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    error!(
                        file_path = %path.display(),
                        line_number = line_number + 1,
                        line_content = %line,
                        error = %e,
                        "Failed to parse line as JSON in session content file"
                    );
                    return Err(OrchestratorError::from(e));
                }
            };

            // Add the raw JSON value to content
            content.push(value);
        }

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_session_file(
        projects_dir: &Path,
        project_name: &str,
        session_id: &str,
        working_dir: &str,
    ) {
        let project_path = projects_dir.join(project_name);
        fs::create_dir_all(&project_path).unwrap();

        let session_file = project_path.join(format!("{session_id}.jsonl"));

        let content = format!(
            r#"{{"sessionId": "{session_id}", "cwd": "{working_dir}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Hello"}}}}
{{"type": "assistant", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "Hi there!"}}]}}}}
"#
        );

        fs::write(session_file, content).unwrap();
    }

    #[tokio::test]
    async fn test_scan_disk_for_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        // Create test session files
        create_test_session_file(
            &projects_dir,
            "project1",
            "session-123",
            "/home/user/project1",
        );
        create_test_session_file(
            &projects_dir,
            "project2",
            "session-456",
            "/home/user/project2",
        );

        let config = Config {
            claude_binary_path: PathBuf::from("/usr/bin/claude"),
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let manager = SessionManager::new(config.clone());
        let discovery = SessionDiscovery::new(&config, &manager);

        let sessions = discovery.list_all_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);

        let session_ids: Vec<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
        assert!(session_ids.contains(&"session-123".to_string()));
        assert!(session_ids.contains(&"session-456".to_string()));
    }

    #[tokio::test]
    async fn test_get_session_content() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        create_test_session_file(
            &projects_dir,
            "project1",
            "session-123",
            "/home/user/project1",
        );

        let config = Config {
            claude_binary_path: PathBuf::from("/usr/bin/claude"),
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let manager = SessionManager::new(config.clone());
        let discovery = SessionDiscovery::new(&config, &manager);

        let (info, content) = discovery.get_session_content("session-123").await.unwrap();
        assert_eq!(info.session_id, "session-123");
        assert_eq!(info.working_directory, PathBuf::from("/home/user/project1"));
        assert!(!info.active);

        assert_eq!(content.len(), 3); // 3 lines in the test file

        // Verify the raw JSON content is preserved
        assert!(content[0].get("sessionId").is_some());
        assert_eq!(
            content[0].get("sessionId").unwrap().as_str().unwrap(),
            "session-123"
        );

        assert_eq!(content[1].get("type").unwrap().as_str().unwrap(), "user");
        assert_eq!(
            content[1]
                .get("message")
                .unwrap()
                .get("content")
                .unwrap()
                .as_str()
                .unwrap(),
            "Hello"
        );

        assert_eq!(
            content[2].get("type").unwrap().as_str().unwrap(),
            "assistant"
        );
    }

    #[tokio::test]
    async fn test_session_id_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        let project_path = projects_dir.join("project1");
        fs::create_dir_all(&project_path).unwrap();

        // Create file with mismatched session ID
        let session_file = project_path.join("wrong-id.jsonl");
        let content = r#"{"sessionId": "different-id", "cwd": "/home/user", "type": "start"}"#;
        fs::write(session_file, content).unwrap();

        let config = Config {
            claude_binary_path: PathBuf::from("/usr/bin/claude"),
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let manager = SessionManager::new(config.clone());
        let discovery = SessionDiscovery::new(&config, &manager);

        // Files with session ID mismatch should now be ignored, not cause errors
        let result = discovery.list_all_sessions().await;
        assert!(result.is_ok());
        let sessions = result.unwrap();
        // The malformed file should be ignored, so no sessions should be found
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_missing_required_fields() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        let project_path = projects_dir.join("project1");
        fs::create_dir_all(&project_path).unwrap();

        // Create file missing cwd field
        let session_file = project_path.join("incomplete.jsonl");
        let content = r#"{"sessionId": "incomplete", "type": "start"}"#;
        fs::write(session_file, content).unwrap();

        let config = Config {
            claude_binary_path: PathBuf::from("/usr/bin/claude"),
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let manager = SessionManager::new(config.clone());
        let discovery = SessionDiscovery::new(&config, &manager);

        // Files with missing required fields should now be ignored, not cause errors
        let result = discovery.list_all_sessions().await;
        assert!(result.is_ok());
        let sessions = result.unwrap();
        // The malformed file should be ignored, so no sessions should be found
        assert!(sessions.is_empty());
    }
}
