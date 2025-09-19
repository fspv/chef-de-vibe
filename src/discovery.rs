use crate::config::Config;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::models::{SessionFileLine, SessionInfo};
use crate::session_manager::SessionManager;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

        // Scan disk for sessions WITH SUMMARIES (now in parallel)
        let (disk_sessions_with_summaries, active_session_fallbacks) =
            self.scan_disk_for_sessions(&active_session_ids);

        // Add all sessions that have summaries (these are complete/inactive sessions)
        for mut session in disk_sessions_with_summaries {
            // Mark as active if it's in the active list
            session.active = active_session_ids.contains(&session.session_id);
            sessions.push(session);
        }

        // Add active sessions that weren't found with summaries
        for active_session in active_sessions {
            let session_id = active_session.get_id().await;
            if !sessions.iter().any(|s| s.session_id == session_id) {
                // Try to get the first user message as fallback from our scan
                let fallback_summary = active_session_fallbacks.get(&session_id).cloned();

                if fallback_summary.is_none() {
                    warn!(
                        session_id = %session_id,
                        working_dir = %active_session.working_dir.display(),
                        "Active session not found on disk - adding without summary"
                    );
                }

                sessions.push(SessionInfo {
                    session_id: session_id.clone(),
                    working_directory: active_session.working_dir.clone(),
                    active: true,
                    summary: fallback_summary,
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

    #[allow(clippy::too_many_lines)]
    fn scan_disk_for_sessions(
        &self,
        active_session_ids: &[String],
    ) -> (Vec<SessionInfo>, HashMap<String, String>) {
        struct FileData {
            summaries: Vec<(String, String)>, // leafUuid -> summary text
            lines: Vec<serde_json::Value>,
        }

        // Phase 1: Collect all .jsonl file paths
        let jsonl_files: Vec<PathBuf> = WalkDir::new(&self.config.claude_projects_dir)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
            .map(|entry| entry.path().to_path_buf())
            .collect();

        // Phase 2: Process files in parallel batches to extract data

        let file_data: Vec<FileData> = jsonl_files
            .par_iter()
            .filter_map(|path| {
                Self::read_jsonl_file(path).ok().map(|lines| {
                    let mut summaries = Vec::new();

                    // Extract summaries from this file
                    for line in &lines {
                        if let Some(entry_type) = line.get("type").and_then(|v| v.as_str()) {
                            if entry_type == "summary" {
                                if let (Some(summary_text), Some(leaf_uuid)) = (
                                    line.get("summary").and_then(|v| v.as_str()),
                                    line.get("leafUuid").and_then(|v| v.as_str()),
                                ) {
                                    summaries
                                        .push((leaf_uuid.to_string(), summary_text.to_string()));
                                }
                            }
                        }
                    }

                    FileData { summaries, lines }
                })
            })
            .collect();

        // Combine all summaries into a single map
        let mut all_summaries: HashMap<String, String> = HashMap::new();
        for data in &file_data {
            for (uuid, summary) in &data.summaries {
                all_summaries.insert(uuid.clone(), summary.clone());
            }
        }

        // Phase 3: Process all lines in parallel to build session information
        let summaries = Arc::new(all_summaries);
        let active_ids = Arc::new(active_session_ids.to_vec());

        // Process each file's data in parallel
        let processed_data: Vec<_> = file_data
            .par_iter()
            .map(|data| {
                let mut local_sessions: HashMap<String, SessionInfo> = HashMap::new();
                let mut local_first_messages: HashMap<String, (String, Option<String>)> =
                    HashMap::new();

                for line in &data.lines {
                    // Check if this entry has a UUID that matches a leafUuid
                    if let Some(uuid) = line.get("uuid").and_then(|v| v.as_str()) {
                        if let Some(summary_text) = summaries.get(uuid) {
                            // Found a match! Extract session information
                            if let Some(session_id) = line.get("sessionId").and_then(|v| v.as_str())
                            {
                                // Update or create session entry with summary
                                local_sessions
                                    .entry(session_id.to_string())
                                    .and_modify(|s| {
                                        if s.summary.is_none() {
                                            s.summary = Some(summary_text.clone());
                                        }
                                    })
                                    .or_insert_with(|| {
                                        let mut info = SessionInfo {
                                            session_id: session_id.to_string(),
                                            working_directory: PathBuf::new(),
                                            active: false,
                                            summary: Some(summary_text.clone()),
                                            earliest_message_date: None,
                                            latest_message_date: None,
                                        };

                                        // Try to get working directory from cwd field
                                        if let Some(cwd) = line.get("cwd").and_then(|v| v.as_str())
                                        {
                                            info.working_directory = PathBuf::from(cwd);
                                        }

                                        info
                                    });
                            }
                        }
                    }

                    // Collect first user message for active sessions (as fallback for active sessions)
                    if let Some(entry_type) = line.get("type").and_then(|v| v.as_str()) {
                        if entry_type == "user" {
                            if let Some(session_id) = line.get("sessionId").and_then(|v| v.as_str())
                            {
                                // Only collect for active sessions
                                if active_ids.contains(&session_id.to_string()) {
                                    let timestamp = line
                                        .get("timestamp")
                                        .and_then(|v| v.as_str())
                                        .map(String::from);

                                    // Extract user message content
                                    let message_content = line
                                        .get("message")
                                        .and_then(|m| m.get("content"))
                                        .and_then(|c| c.as_str())
                                        .map_or_else(|| "No content".to_string(), String::from);

                                    local_first_messages
                                        .entry(session_id.to_string())
                                        .and_modify(|(existing_msg, existing_ts)| {
                                            if let (Some(new_ts), Some(old_ts)) =
                                                (&timestamp, existing_ts.as_ref())
                                            {
                                                if new_ts < old_ts {
                                                    existing_msg.clone_from(&message_content);
                                                    existing_ts.clone_from(&timestamp);
                                                }
                                            } else if existing_ts.is_none() {
                                                existing_msg.clone_from(&message_content);
                                                existing_ts.clone_from(&timestamp);
                                            }
                                        })
                                        .or_insert((message_content, timestamp));
                                }
                            }
                        }
                    }

                    // Also collect sessions from sessionId and cwd fields
                    if let (Some(session_id), Some(cwd)) = (
                        line.get("sessionId").and_then(|v| v.as_str()),
                        line.get("cwd").and_then(|v| v.as_str()),
                    ) {
                        local_sessions
                            .entry(session_id.to_string())
                            .and_modify(|s| {
                                if s.working_directory == PathBuf::new() {
                                    s.working_directory = PathBuf::from(cwd);
                                }
                            })
                            .or_insert_with(|| SessionInfo {
                                session_id: session_id.to_string(),
                                working_directory: PathBuf::from(cwd),
                                active: false,
                                summary: None,
                                earliest_message_date: None,
                                latest_message_date: None,
                            });
                    }

                    // Collect timestamps for sessions
                    if let Some(session_id) = line.get("sessionId").and_then(|v| v.as_str()) {
                        if let Some(timestamp) = line.get("timestamp").and_then(|v| v.as_str()) {
                            if let Some(session) = local_sessions.get_mut(session_id) {
                                let timestamp_str = timestamp.to_string();

                                if session.earliest_message_date.is_none()
                                    || Some(&timestamp_str) < session.earliest_message_date.as_ref()
                                {
                                    session.earliest_message_date = Some(timestamp_str.clone());
                                }

                                if session.latest_message_date.is_none()
                                    || Some(&timestamp_str) > session.latest_message_date.as_ref()
                                {
                                    session.latest_message_date = Some(timestamp_str);
                                }
                            }
                        }
                    }
                }

                (local_sessions, local_first_messages)
            })
            .collect();

        // Merge all results
        let mut sessions: HashMap<String, SessionInfo> = HashMap::new();
        let mut first_user_messages: HashMap<String, (String, Option<String>)> = HashMap::new();

        for (local_sessions, local_messages) in processed_data {
            // Merge sessions
            for (session_id, session_info) in local_sessions {
                sessions
                    .entry(session_id)
                    .and_modify(|existing| {
                        // Merge data, preferring non-empty values
                        if existing.summary.is_none() && session_info.summary.is_some() {
                            existing.summary.clone_from(&session_info.summary);
                        }
                        if existing.working_directory == PathBuf::new()
                            && session_info.working_directory != PathBuf::new()
                        {
                            existing
                                .working_directory
                                .clone_from(&session_info.working_directory);
                        }
                        // Update timestamps to get earliest/latest
                        if let Some(ref new_earliest) = session_info.earliest_message_date {
                            if existing.earliest_message_date.is_none()
                                || existing.earliest_message_date.as_ref() > Some(new_earliest)
                            {
                                existing.earliest_message_date = Some(new_earliest.clone());
                            }
                        }
                        if let Some(ref new_latest) = session_info.latest_message_date {
                            if existing.latest_message_date.is_none()
                                || existing.latest_message_date.as_ref() < Some(new_latest)
                            {
                                existing.latest_message_date = Some(new_latest.clone());
                            }
                        }
                    })
                    .or_insert(session_info);
            }

            // Merge first user messages for active sessions
            for (session_id, (msg, timestamp)) in local_messages {
                first_user_messages
                    .entry(session_id)
                    .and_modify(|(existing_msg, existing_ts)| {
                        // Keep the earliest message
                        if let (Some(new_ts), Some(old_ts)) = (&timestamp, existing_ts.as_ref()) {
                            if new_ts < old_ts {
                                existing_msg.clone_from(&msg);
                                existing_ts.clone_from(&timestamp);
                            }
                        } else if existing_ts.is_none() {
                            existing_msg.clone_from(&msg);
                            existing_ts.clone_from(&timestamp);
                        }
                    })
                    .or_insert((msg, timestamp));
            }
        }

        // Prepare fallback summaries for active sessions only
        let mut active_session_fallbacks: HashMap<String, String> = HashMap::new();
        for (session_id, (first_msg, _)) in first_user_messages {
            active_session_fallbacks.insert(session_id, first_msg);
        }

        // Return sessions that have summaries OR are active sessions
        let mut sessions_to_return: Vec<SessionInfo> = Vec::new();

        // First, add all sessions with summaries
        for (session_id, session_info) in sessions {
            if session_info.summary.is_some() && session_info.working_directory != PathBuf::new() {
                sessions_to_return.push(session_info);
            } else if active_session_ids.contains(&session_id)
                && session_info.working_directory != PathBuf::new()
            {
                // Also include active sessions even without summaries
                sessions_to_return.push(session_info);
            }
        }

        (sessions_to_return, active_session_fallbacks)
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

    fn read_jsonl_file(path: &Path) -> OrchestratorResult<Vec<serde_json::Value>> {
        let file = File::open(path).map_err(|e| {
            error!(
                file_path = %path.display(),
                error = %e,
                "Failed to open file for reading"
            );
            OrchestratorError::FileParseError(format!(
                "Failed to open file {}: {e}",
                path.display()
            ))
        })?;

        let reader = BufReader::new(file);
        let mut lines = Vec::new();

        for (line_number, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| {
                error!(
                    file_path = %path.display(),
                    line_number = line_number + 1,
                    error = %e,
                    "Failed to read line from file"
                );
                OrchestratorError::FileParseError(format!(
                    "Failed to read line {} from {}: {e}",
                    line_number + 1,
                    path.display()
                ))
            })?;

            // Try to parse as JSON, skip invalid lines
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                lines.push(json);
            }
        }

        Ok(lines)
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
        projects_dir: &std::path::Path,
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
    async fn test_scan_disk_for_sessions_without_summaries() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        // Create test session files WITHOUT summaries
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

        // Sessions without summaries should NOT be returned
        let sessions = discovery.list_all_sessions().await.unwrap();
        assert_eq!(
            sessions.len(),
            0,
            "Sessions without summaries should not be listed"
        );
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
    async fn test_session_with_summary_and_filename_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        let project_path = projects_dir.join("project1");
        fs::create_dir_all(&project_path).unwrap();

        // Create summary file
        let summary_file = project_path.join("summary-file.jsonl");
        let summary_content =
            r#"{"type":"summary","summary":"Test Session Summary","leafUuid":"uuid-123"}"#;
        fs::write(summary_file, summary_content).unwrap();

        // Create session file where filename doesn't match session ID
        let session_file = project_path.join("some-uuid.jsonl");
        let content = r#"{"sessionId": "different-id", "cwd": "/home/user", "type": "start"}
{"sessionId": "different-id", "uuid": "uuid-123", "type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Response"}]}}"#;
        fs::write(session_file, content).unwrap();

        let config = Config {
            claude_binary_path: PathBuf::from("/usr/bin/claude"),
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let manager = SessionManager::new(config.clone());
        let discovery = SessionDiscovery::new(&config, &manager);

        // Should find the session because it has a summary
        let result = discovery.list_all_sessions().await;
        assert!(result.is_ok());
        let sessions = result.unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "different-id");
        assert_eq!(sessions[0].working_directory, PathBuf::from("/home/user"));
        assert_eq!(
            sessions[0].summary,
            Some("Test Session Summary".to_string())
        );
    }

    #[tokio::test]
    async fn test_active_sessions_included_without_summaries() {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).unwrap();

        let project_path = projects_dir.join("active-project");
        fs::create_dir_all(&project_path).unwrap();

        // Create a session file without a summary
        let session_file = project_path.join("active-session.jsonl");
        let content = r#"{"sessionId": "active-session", "cwd": "/home/user/active", "type": "start"}
{"sessionId": "active-session", "type": "user", "message": {"role": "user", "content": "Active message"}}"#;
        fs::write(session_file, content).unwrap();

        let config = Config {
            claude_binary_path: PathBuf::from("/usr/bin/claude"),
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let manager = SessionManager::new(config.clone());
        let discovery = SessionDiscovery::new(&config, &manager);

        // Without marking as active, session should NOT be listed (no summary)
        let sessions = discovery.list_all_sessions().await.unwrap();
        assert_eq!(
            sessions.len(),
            0,
            "Sessions without summaries should not be listed by default"
        );

        // Now simulate this being an active session
        let active_ids = vec!["active-session".to_string()];
        let (sessions_with_summaries, fallbacks) = discovery.scan_disk_for_sessions(&active_ids);

        // Should find the session now because it's in the active list
        assert_eq!(
            sessions_with_summaries.len(),
            1,
            "Active sessions should be included even without summaries"
        );
        assert_eq!(sessions_with_summaries[0].session_id, "active-session");

        // Should also have a fallback summary
        assert_eq!(
            fallbacks.get("active-session"),
            Some(&"Active message".to_string())
        );
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
