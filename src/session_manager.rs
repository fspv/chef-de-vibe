use crate::claude_process::ClaudeProcess;
use crate::config::Config;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::models::{ApprovalMessage, ApprovalRequest, BroadcastMessage, Session, SessionStatus, WriteMessage};
use dashmap::DashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use tracing::{info, warn, error, debug, instrument};
use uuid::Uuid;

/// Compacts JSON message to a single line for Claude stdin.
/// Claude expects each JSON message to be on a single line.
fn compact_json_message(message: &str, context: &str) -> OrchestratorResult<String> {
    match serde_json::from_str::<serde_json::Value>(message) {
        Ok(parsed) => {
            match serde_json::to_string(&parsed) {
                Ok(compacted) => {
                    debug!(
                        context = %context,
                        original_len = message.len(),
                        compacted_len = compacted.len(),
                        "Successfully compacted JSON message"
                    );
                    Ok(compacted)
                }
                Err(e) => {
                    error!(
                        context = %context,
                        error = %e,
                        "Failed to re-serialize JSON message"
                    );
                    Err(OrchestratorError::ProcessCommunicationError(
                        format!("Failed to compact JSON message for {}: {}", context, e)
                    ))
                }
            }
        }
        Err(e) => {
            error!(
                context = %context,
                error = %e,
                message = %message,
                "Invalid JSON in message"
            );
            Err(OrchestratorError::ProcessCommunicationError(
                format!("Invalid JSON in message for {}: {}", context, e)
            ))
        }
    }
}

pub struct SessionManager {
    sessions: Arc<DashMap<String, Arc<Session>>>,
    config: Arc<Config>,
    worker_handles: Arc<DashMap<String, JoinHandle<()>>>,
}

impl SessionManager {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            config: Arc::new(config),
            worker_handles: Arc::new(DashMap::new()),
        }
    }

    /// Waits for a session file to be created and contain non-empty content.
    /// 
    /// # Arguments
    /// * `session_id` - The session ID to wait for
    /// * `timeout_duration` - Maximum time to wait for the file
    /// 
    /// # Returns
    /// Ok(()) if file exists and is non-empty, Err if timeout or other error
    #[instrument(skip(self), fields(session_id = %session_id, timeout_seconds = timeout_duration.as_secs()))]
    async fn wait_for_session_file(&self, session_id: &str, timeout_duration: Duration) -> OrchestratorResult<()> {
        let filename = format!("{}.jsonl", session_id);
        
        debug!(
            session_id = %session_id,
            filename = %filename,
            projects_dir = %self.config.claude_projects_dir.display(),
            timeout_seconds = timeout_duration.as_secs(),
            "Starting to wait for session file creation"
        );

        let check_file_ready = || async {
            // Search for the session file in all subdirectories
            let session_file_path = match self.find_session_file(&filename) {
                Some(path) => path,
                None => {
                    debug!(
                        session_id = %session_id,
                        filename = %filename,
                        "Session file not found in any project directory"
                    );
                    return false;
                }
            };

            // Check if file has content (not empty)
            match std::fs::metadata(&session_file_path) {
                Ok(metadata) => {
                    if metadata.len() == 0 {
                        debug!(
                            session_id = %session_id,
                            file_path = %session_file_path.display(),
                            "Session file exists but is empty"
                        );
                        return false;
                    }
                    
                    info!(
                        session_id = %session_id,
                        file_path = %session_file_path.display(),
                        file_size = metadata.len(),
                        "Session file is ready with content"
                    );
                    true
                }
                Err(e) => {
                    debug!(
                        session_id = %session_id,
                        file_path = %session_file_path.display(),
                        error = %e,
                        "Error reading session file metadata"
                    );
                    false
                }
            }
        };

        // Use timeout to wait for file to be ready
        match timeout(timeout_duration, async {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                if check_file_ready().await {
                    return Ok(());
                }
                interval.tick().await;
            }
        }).await {
            Ok(result) => {
                info!(
                    session_id = %session_id,
                    filename = %filename,
                    "Successfully waited for session file to be ready"
                );
                result
            }
            Err(_) => {
                error!(
                    session_id = %session_id,
                    filename = %filename,
                    timeout_seconds = timeout_duration.as_secs(),
                    "Timeout waiting for session file to be created and populated"
                );
                Err(OrchestratorError::InternalError(
                    format!("Timeout waiting for session file {} to be created", session_id)
                ))
            }
        }
    }

    /// Finds a session file by scanning all subdirectories in the projects directory
    fn find_session_file(&self, filename: &str) -> Option<PathBuf> {
        use walkdir::WalkDir;
        
        for entry in WalkDir::new(&self.config.claude_projects_dir)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) == Some(filename) {
                return Some(path.to_path_buf());
            }
        }
        None
    }

    /// Creates or resumes a session with the given parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the working directory is invalid, if the Claude process
    /// fails to spawn, or if the session creation fails.
    #[instrument(skip(self), fields(session_id = %session_id, working_dir = %working_dir.display(), resume = resume, first_message_len = first_message.len()))]
    pub async fn create_session(
        &self,
        session_id: String,
        working_dir: &Path,
        resume: bool,
        first_message: String,
    ) -> OrchestratorResult<String> {
        info!(
            session_id = %session_id,
            working_dir = %working_dir.display(),
            resume = resume,
            "Creating session"
        );

        // Check if session already exists and is running
        if let Some(session) = self.sessions.get(&session_id) {
            if session.is_active().await {
                info!(
                    session_id = %session_id,
                    "Session already exists and is active, returning existing session"
                );
                return Ok(session_id);
            }
            // Session exists but not running, remove it
            warn!(
                session_id = %session_id,
                "Session exists but is not active, removing it before creating new one"
            );
            self.sessions.remove(&session_id);
        }

        debug!(
            session_id = %session_id,
            working_dir = %working_dir.display(),
            "Validating working directory"
        );

        // Validate working directory
        if !working_dir.exists() {
            error!(
                session_id = %session_id,
                working_dir = %working_dir.display(),
                "Working directory does not exist"
            );
            return Err(OrchestratorError::WorkingDirInvalid(format!("Working directory does not exist: {}", working_dir.display())));
        }

        if !working_dir.is_dir() {
            error!(
                session_id = %session_id,
                working_dir = %working_dir.display(),
                "Path is not a directory"
            );
            return Err(OrchestratorError::WorkingDirInvalid(format!("Path is not a directory: {}", working_dir.display())));
        }

        debug!(
            session_id = %session_id,
            working_dir = %working_dir.display(),
            "Working directory validation passed"
        );

        // Create new session
        let session = Arc::new(Session::new(session_id.clone(), working_dir.to_path_buf()));
        debug!(
            session_id = %session_id,
            "Created new session instance"
        );

        // Store session immediately with pending status
        self.sessions.insert(session_id.clone(), session.clone());
        info!(
            session_id = %session_id,
            "Session stored in session manager with pending status"
        );

        // Spawn background worker
        let config = self.config.clone();
        let session_clone = session.clone();
        let sessions = self.sessions.clone();
        let worker_session_id = session_id.clone();
        let working_dir = working_dir.to_path_buf();

        debug!(
            session_id = %session_id,
            "Spawning background worker for Claude process"
        );

        let handle = tokio::spawn(async move {
            debug!(
                session_id = %worker_session_id,
                working_dir = %working_dir.display(),
                resume = resume,
                "Starting Claude process spawn in background worker"
            );

            match Self::spawn_claude_process(
                &config,
                &worker_session_id,
                &working_dir,
                resume,
                first_message.clone(),
                session_clone.clone(),
            )
            .await
            {
                Ok(actual_session_id) => {
                    info!(
                        requested_session_id = %worker_session_id,
                        actual_session_id = %actual_session_id,
                        "Claude process spawned successfully"
                    );

                    // If session ID changed (resume case), update the mapping
                    if actual_session_id != worker_session_id {
                        info!(
                            old_session_id = %worker_session_id,
                            new_session_id = %actual_session_id,
                            "Session ID changed during resume, updating session mapping"
                        );
                        sessions.remove(&worker_session_id);
                        session_clone.set_id(actual_session_id.clone()).await;
                        sessions.insert(actual_session_id.clone(), session_clone.clone());
                    }
                    session_clone.set_status(SessionStatus::Ready).await;
                    info!(
                        session_id = %actual_session_id,
                        "Session status set to Ready"
                    );
                }
                Err(e) => {
                    error!(
                        session_id = %worker_session_id,
                        working_dir = %working_dir.display(),
                        error = %e,
                        "Failed to spawn Claude process"
                    );
                    session_clone.set_status(SessionStatus::Failed).await;
                    error!(
                        session_id = %worker_session_id,
                        "Session status set to Failed, removing from sessions"
                    );
                    sessions.remove(&worker_session_id);
                }
            }
        });

        self.worker_handles.insert(session_id.clone(), handle);
        debug!(
            session_id = %session_id,
            "Worker handle stored, waiting for completion"
        );

        // Wait for the worker to complete
        if let Some((_, handle)) = self.worker_handles.remove(&session_id) {
            debug!(
                session_id = %session_id,
                "Awaiting worker task completion"
            );
            handle.await.map_err(|e| {
                error!(
                    session_id = %session_id,
                    error = %e,
                    "Worker task failed"
                );
                OrchestratorError::InternalError(format!("Worker task failed: {e}"))
            })?;
        }

        // Check final status
        let final_status = session.get_status().await;
        debug!(
            session_id = %session_id,
            status = ?final_status,
            "Checking final session status"
        );

        match final_status {
            SessionStatus::Ready => {
                // Get the actual session ID (may be different for resume case)
                let actual_session_id = if resume {
                    session.get_id().await
                } else {
                    session_id.clone()
                };

                // Wait for session file to be created and populated
                debug!(
                    session_id = %actual_session_id,
                    "Claude process is ready, now waiting for session file to be created"
                );

                // Use a 20 second timeout for file creation
                let file_timeout = Duration::from_secs(20);
                if let Err(e) = self.wait_for_session_file(&actual_session_id, file_timeout).await {
                    error!(
                        session_id = %actual_session_id,
                        error = %e,
                        "Failed to wait for session file creation"
                    );
                    return Err(e);
                }

                if resume {
                    info!(
                        requested_session_id = %session_id,
                        actual_session_id = %actual_session_id,
                        "Session created successfully (resumed with different ID) and file is ready"
                    );
                    Ok(actual_session_id)
                } else {
                    info!(
                        session_id = %session_id,
                        "Session created successfully and file is ready"
                    );
                    Ok(session_id)
                }
            }
            SessionStatus::Failed => {
                error!(
                    session_id = %session_id,
                    "Session creation failed - Claude process spawn failed"
                );
                Err(OrchestratorError::ClaudeSpawnFailed(
                    "Failed to spawn Claude process".into(),
                ))
            }
            SessionStatus::Pending => {
                error!(
                    session_id = %session_id,
                    "Session creation failed - unexpected pending status"
                );
                Err(OrchestratorError::InternalError(
                    "Session creation did not complete".into(),
                ))
            }
        }
    }

    #[instrument(skip(config, session), fields(session_id = %session_id, working_dir = %working_dir.display(), resume = resume, first_message_len = first_message.len()))]
    async fn spawn_claude_process(
        config: &Config,
        session_id: &str,
        working_dir: &Path,
        resume: bool,
        first_message: String,
        session: Arc<Session>,
    ) -> OrchestratorResult<String> {
        info!(
            session_id = %session_id,
            working_dir = %working_dir.display(),
            resume = resume,
            claude_binary = %config.claude_binary_path.display(),
            "Spawning Claude process"
        );

        // Spawn Claude process
        let (process, actual_session_id) = match ClaudeProcess::spawn(config, session_id, working_dir, resume, &first_message).await {
            Ok((proc, id)) => {
                info!(
                    requested_session_id = %session_id,
                    actual_session_id = %id,
                    "Claude process spawned successfully"
                );
                (proc, id)
            }
            Err(e) => {
                error!(
                    session_id = %session_id,
                    working_dir = %working_dir.display(),
                    claude_binary = %config.claude_binary_path.display(),
                    error = %e,
                    "Failed to spawn Claude process"
                );
                return Err(e);
            }
        };

        // Extract components from process before moving
        let mut child = process.child;
        let stdin_tx = process.stdin_tx;
        let mut stdout_rx = process.stdout_rx;

        debug!(
            session_id = %actual_session_id,
            "Extracted Claude process components"
        );

        // Store process ID in session
        session.set_process_id(child.id()).await;
        debug!(
            session_id = %actual_session_id,
            process_id = ?child.id(),
            "Stored Claude process ID in session"
        );

        // Spawn dedicated task to wait for process exit and trigger immediate cleanup
        let process_waiter_session = session.clone();
        let process_waiter_session_id = actual_session_id.clone();
        tokio::spawn(async move {
            let process_id = child.id();
            debug!(
                session_id = %process_waiter_session_id,
                process_id = ?process_id,
                "Starting dedicated process waiter task"
            );
            
            // Wait for the process to exit - this is the proper way to wait and avoid zombies
            let exit_status = child.wait().await;

            match exit_status {
                Ok(status) => {
                    warn!(
                        session_id = %process_waiter_session_id,
                        process_id = ?process_id,
                        exit_code = status.code(),
                        exit_success = status.success(),
                        "Claude process has exited"
                    );
                }
                Err(e) => {
                    error!(
                        session_id = %process_waiter_session_id,
                        process_id = ?process_id,
                        error = %e,
                        "Error waiting for Claude process to exit"
                    );
                }
            }

            // Clear the process ID from the session
            process_waiter_session.set_process_id(None).await;
            
            // Immediately broadcast disconnect to all WebSocket clients
            if let Err(e) = process_waiter_session.broadcast_message(BroadcastMessage::Disconnect) {
                error!(
                    session_id = %process_waiter_session_id,
                    error = %e,
                    "Failed to broadcast disconnect after process exit"
                );
            } else {
                info!(
                    session_id = %process_waiter_session_id,
                    "Successfully broadcast disconnect after process exit"
                );
            }
            
            debug!(
                session_id = %process_waiter_session_id,
                "Process waiter task finished"
            );
        });

        // Spawn task to handle Claude output and broadcast to WebSocket clients
        let output_session = session.clone();
        let output_session_id = actual_session_id.clone();
        tokio::spawn(async move {
            info!(
                session_id = %output_session_id,
                "Starting Claude output handler task"
            );
            
            let mut lines_processed = 0;
            while let Some(line) = stdout_rx.recv().await {
                lines_processed += 1;
                debug!(
                    session_id = %output_session_id,
                    line_number = lines_processed,
                    line_length = line.len(),
                    "Received line from Claude stdout"
                );

                // Parse and validate JSON
                let parsed_line: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(value) => value,
                    Err(e) => {
                        error!(
                            session_id = %output_session_id,
                            line_number = lines_processed,
                            line_content = %line,
                            error = %e,
                            "Received invalid JSON from Claude process"
                        );
                        
                        // Kill the process and notify clients
                        if let Err(e) = output_session.broadcast_message(BroadcastMessage::Disconnect) {
                            error!(
                                session_id = %output_session_id,
                                error = %e,
                                "Failed to send disconnect notification"
                            );
                        }
                        break;
                    }
                };

                debug!(
                    session_id = %output_session_id,
                    line_number = lines_processed,
                    json_content = %line,
                    "Valid JSON received from Claude, checking message type"
                );

                // Check if this is a control_request for tool approval
                let message_type = parsed_line.get("type");
                let message_subtype = parsed_line.get("subtype");
                debug!(
                    session_id = %output_session_id,
                    message_type = ?message_type,
                    message_subtype = ?message_subtype,
                    "Checking if this is a control_request"
                );
                
                if parsed_line.get("type") == Some(&serde_json::Value::String("control_request".to_string()))
                    && parsed_line.get("request")
                        .and_then(|r| r.get("subtype"))
                        .map(|st| st == &serde_json::Value::String("can_use_tool".to_string()))
                        .unwrap_or(false) {
                    
                    debug!(
                        session_id = %output_session_id,
                        line_number = lines_processed,
                        "Detected control_request for tool approval"
                    );

                    // Generate unique ID for our wrapper (for frontend/backend matching)
                    let approval_id = Uuid::new_v4().to_string();

                    // Extract Claude's original request_id for internal error handling
                    let claude_request_id = parsed_line.get("request_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&Uuid::new_v4().to_string())
                        .to_string();

                    // Pass through the entire nested 'request' object from Claude as-is
                    let claude_request = parsed_line.get("request")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));

                    debug!(
                        session_id = %output_session_id,
                        approval_id = %approval_id,
                        claude_request_id = %claude_request_id,
                        "Creating wrapped approval request (pass-through approach)"
                    );

                    // Create approval request with raw Claude data - no parsing
                    let approval_request = ApprovalRequest {
                        id: approval_id.clone(),
                        session_id: output_session_id.clone(),
                        claude_request_id: claude_request_id.clone(),
                        request: claude_request,  // Raw Claude request - pass through
                        created_at: std::time::SystemTime::now(),
                    };

                    // Store the approval request in the session
                    output_session.add_pending_approval(approval_request.clone()).await;

                    info!(
                        session_id = %output_session_id,
                        approval_id = %approval_id,
                        claude_request_id = %claude_request_id,
                        "Stored approval request and broadcasting to approval clients"
                    );

                    // Broadcast to approval WebSocket clients
                    if let Err(e) = output_session.broadcast_approval_message(
                        ApprovalMessage::ApprovalRequest(approval_request)
                    ) {
                        error!(
                            session_id = %output_session_id,
                            approval_id = %approval_id,
                            error = %e,
                            "Failed to broadcast approval request to approval clients"
                        );
                    }
                    
                    // Do NOT broadcast control_requests to regular Claude WebSocket clients
                    // Claude will wait for our response via stdin
                } else {
                    // This is a regular Claude message, broadcast to regular clients
                    debug!(
                        session_id = %output_session_id,
                        line_number = lines_processed,
                        message_type = ?message_type,
                        "Regular Claude message (not control_request), broadcasting to clients"
                    );

                    // Broadcast Claude output to all clients
                    match output_session.broadcast_message(BroadcastMessage::ClaudeOutput(line)) {
                        Ok(receiver_count) => {
                            debug!(
                                session_id = %output_session_id,
                                line_number = lines_processed,
                                receiver_count = receiver_count,
                                "Successfully broadcast Claude output to clients"
                            );
                        }
                        Err(broadcast::error::SendError(_)) => {
                            // No receivers connected - this is expected when no regular WebSocket clients
                            // are connected. According to README, we should discard the output and continue.
                            debug!(
                                session_id = %output_session_id,
                                line_number = lines_processed,
                                "No regular WebSocket clients connected, discarding Claude output"
                            );
                            // Continue processing - approval system may still need us
                        }
                    }
                }
            }

            // Process has ended
            info!(
                session_id = %output_session_id,
                total_lines_processed = lines_processed,
                "Claude output handler finished - process waiter will handle disconnect"
            );
            // Note: Don't broadcast disconnect here since the dedicated process waiter will handle it
        });

        // Spawn task to process write queue
        let write_session = session.clone();
        let write_stdin_tx = stdin_tx.clone();
        tokio::spawn(async move {
            loop {
                // Check if process is still alive
                if write_session.get_process_id().await.is_none() {
                    break;
                }

                // Process write queue
                if let Some(msg) = write_session.dequeue_message().await {
                    // Compact JSON to ensure single-line format
                    let compacted_message = match compact_json_message(&msg.content, "write_queue") {
                        Ok(compacted) => compacted,
                        Err(e) => {
                            error!("Failed to compact message from write queue: {}", e);
                            break;
                        }
                    };
                    
                    if write_stdin_tx.send(compacted_message).await.is_err() {
                        eprintln!("Failed to send message to Claude stdin");
                        break;
                    }
                }

                // Small delay to prevent busy loop
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });

        // Spawn task to handle approval responses
        let approval_session = session.clone();
        let approval_stdin_tx = stdin_tx.clone();
        let approval_session_id = actual_session_id.clone();
        tokio::spawn(async move {
            let mut approval_rx = approval_session.subscribe_to_approval_broadcasts();
            info!(session_id = %approval_session_id, "Starting approval response handler");
            
            while let Ok(approval_message) = approval_rx.recv().await {
                if let ApprovalMessage::ApprovalResponse(response_data) = approval_message {
                    // Extract our wrapper id from the client response
                    let wrapper_id = response_data.get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    
                    debug!(
                        session_id = %approval_session_id,
                        wrapper_id = %wrapper_id,
                        "Processing approval response with new format"
                    );

                    // Check if process is still alive
                    if approval_session.get_process_id().await.is_none() {
                        debug!(session_id = %approval_session_id, "Claude process not active, stopping approval response handler");
                        break;
                    }

                    // Remove the approval request from pending using our wrapper id
                    if let Some(removed_request) = approval_session.remove_pending_approval(&wrapper_id).await {
                        info!(
                            session_id = %approval_session_id,
                            wrapper_id = %wrapper_id,
                            "Removed approval request from pending state"
                        );

                        // Use the stored Claude request_id from the approval request
                        let claude_request_id = removed_request.claude_request_id;

                        // Pass through client's raw response to Claude without parsing
                        let default_response = serde_json::json!({
                            "behavior": "deny", 
                            "message": "Invalid response format"
                        });
                        let client_response = response_data.get("response")
                            .unwrap_or(&default_response);

                        let control_response = serde_json::json!({
                            "type": "control_response",
                            "response": {
                                "subtype": "success",
                                "request_id": claude_request_id,  // Use Claude's original request_id
                                "response": client_response  // Pass through client's raw response
                            }
                        });

                        debug!(
                            session_id = %approval_session_id,
                            wrapper_id = %wrapper_id,
                            claude_request_id = %claude_request_id,
                            "Extracted Claude request_id from stored request and prepared control_response"
                        );

                        let response_json = match serde_json::to_string(&control_response) {
                            Ok(json) => json,
                            Err(e) => {
                                error!(
                                    session_id = %approval_session_id,
                                    wrapper_id = %wrapper_id,
                                    error = %e,
                                    "Failed to serialize control_response"
                                );
                                continue;
                            }
                        };

                        debug!(
                            session_id = %approval_session_id,
                            wrapper_id = %wrapper_id,
                            claude_request_id = %claude_request_id,
                            response_json = %response_json,
                            "Sending control_response to Claude stdin"
                        );

                        if let Err(e) = approval_stdin_tx.send(response_json).await {
                            error!(
                                session_id = %approval_session_id,
                                wrapper_id = %wrapper_id,
                                error = %e,
                                "Failed to send control_response to Claude stdin"
                            );
                            break;
                        }

                        info!(
                            session_id = %approval_session_id,
                            wrapper_id = %wrapper_id,
                            claude_request_id = %claude_request_id,
                            "Successfully sent approval response to Claude"
                        );
                    } else {
                        warn!(
                            session_id = %approval_session_id,
                            wrapper_id = %wrapper_id,
                            "Approval request not found in pending state"
                        );
                        continue;  // Skip this response if we can't find the original request
                    }
                }
            }

            info!(session_id = %approval_session_id, "Approval response handler stopped");
        });

        Ok(actual_session_id)
    }

    #[must_use]
    pub fn get_session(&self, session_id: &str) -> Option<Arc<Session>> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    pub async fn get_active_sessions(&self) -> Vec<Arc<Session>> {
        let mut active_sessions = Vec::new();
        for entry in self.sessions.iter() {
            let session = entry.value();
            if session.is_active().await {
                active_sessions.push(session.clone());
            }
        }
        active_sessions
    }

    pub async fn shutdown(&self) {
        // Send SIGTERM to all Claude processes using process IDs
        for entry in self.sessions.iter() {
            let session = entry.value();
            if let Some(pid) = session.get_process_id().await {
                // Try to kill the process using system kill
                #[cfg(unix)]
                {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                        warn!(
                            session_id = %session.get_id().await,
                            process_id = pid,
                            error = %e,
                            "Failed to send SIGTERM to Claude process"
                        );
                    }
                }
                #[cfg(not(unix))]
                {
                    warn!(
                        session_id = %session.get_id().await,
                        process_id = pid,
                        "Process killing not implemented for non-Unix systems"
                    );
                }
            }
        }

        // Wait for graceful shutdown
        tokio::time::sleep(self.config.shutdown_timeout).await;

        // Force kill any remaining processes
        for entry in self.sessions.iter() {
            let session = entry.value();
            if let Some(pid) = session.get_process_id().await {
                #[cfg(unix)]
                {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGKILL) {
                        warn!(
                            session_id = %session.get_id().await,
                            process_id = pid,
                            error = %e,
                            "Failed to send SIGKILL to Claude process"
                        );
                    }
                }
                session.set_process_id(None).await;
            }
        }
    }

    /// Enqueues a message for a specific session.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is not found or not active.
    pub async fn enqueue_message(
        &self,
        session_id: &str,
        message: WriteMessage,
    ) -> OrchestratorResult<()> {
        let session = self
            .get_session(session_id)
            .ok_or_else(|| OrchestratorError::SessionNotFound(session_id.to_string()))?;

        if !session.is_active().await {
            return Err(OrchestratorError::ProcessCommunicationError(
                "Session is not active".into(),
            ));
        }

        session.enqueue_message(message).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config(temp_dir: &TempDir) -> Config {
        // Create a mock Claude binary
        let claude_path = temp_dir.path().join("mock_claude");
        let script = r#"#!/bin/bash
echo '{"sessionId": "'$2'", "type": "start"}'
while read line; do
    echo '{"type": "echo", "content": "'$line'"}'
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

        Config {
            claude_binary_path: claude_path,
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(1),
        }
    }

    #[tokio::test]
    async fn test_create_session() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        let manager = SessionManager::new(config);

        let session_id = manager
            .create_session("test-session".to_string(), &working_dir, false, r#"{"role": "user", "content": "Hello"}"#.to_string())
            .await
            .unwrap();

        assert_eq!(session_id, "test-session");

        let session = manager.get_session("test-session").unwrap();
        assert!(session.is_active().await);
        assert_eq!(session.get_status().await, SessionStatus::Ready);
    }

    #[tokio::test]
    async fn test_invalid_working_directory() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let manager = SessionManager::new(config);

        let non_existent = temp_dir.path().join("non_existent");
        let result = manager
            .create_session("test-session".to_string(), &non_existent, false, r#"{"role": "user", "content": "Hello"}"#.to_string())
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OrchestratorError::WorkingDirInvalid(_)
        ));
    }

    #[tokio::test]
    async fn test_session_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        let manager = SessionManager::new(config);

        // Create first session
        let session_id1 = manager
            .create_session("test-session".to_string(), &working_dir, false, r#"{"role": "user", "content": "Hello"}"#.to_string())
            .await
            .unwrap();

        // Try to create same session again
        let session_id2 = manager
            .create_session("test-session".to_string(), &working_dir, false, r#"{"role": "user", "content": "Hello"}"#.to_string())
            .await
            .unwrap();

        assert_eq!(session_id1, session_id2);
    }

    #[tokio::test]
    async fn test_enqueue_message() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        let manager = SessionManager::new(config);

        manager
            .create_session("test-session".to_string(), &working_dir, false, r#"{"role": "user", "content": "Hello"}"#.to_string())
            .await
            .unwrap();

        let message = WriteMessage {
            content: "Hello Claude".to_string(),
            sender_client_id: "client1".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        manager
            .enqueue_message("test-session", message)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_shutdown() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        let manager = SessionManager::new(config);

        manager
            .create_session("test-session".to_string(), &working_dir, false, r#"{"role": "user", "content": "Hello"}"#.to_string())
            .await
            .unwrap();

        manager.shutdown().await;

        let session = manager.get_session("test-session").unwrap();
        assert!(!session.is_active().await);
    }
}
