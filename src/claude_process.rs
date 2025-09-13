use crate::config::Config;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::models::Session;
use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::{info, warn, error, debug, instrument};

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

pub struct ClaudeProcess {
    pub child: Child,
    pub stdin_tx: mpsc::Sender<String>,
    pub stdout_rx: mpsc::Receiver<String>,
}

impl ClaudeProcess {
    #[instrument(skip(config), fields(
        session_id = %session_id,
        working_dir = %working_dir.display(),
        resume = resume,
        claude_binary = %config.claude_binary_path.display(),
        first_message_len = first_message.len()
    ))]
    pub async fn spawn(
        config: &Config,
        session_id: &str,
        working_dir: &Path,
        resume: bool,
        first_message: &str,
    ) -> OrchestratorResult<(Self, String)> {
        info!(
            session_id = %session_id,
            working_dir = %working_dir.display(),
            resume = resume,
            claude_binary = %config.claude_binary_path.display(),
            "Spawning Claude process"
        );

        // Build the command
        let mut cmd = Command::new(&config.claude_binary_path);
        cmd.current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        cmd.arg("--output-format");
        cmd.arg("stream-json");
        cmd.arg("--input-format");
        cmd.arg("stream-json");
        cmd.arg("--verbose");
        cmd.arg("--print");
        cmd.arg("--permission-prompt-tool");
        cmd.arg("stdio");

        if resume {
            cmd.arg("--resume");
            cmd.arg(session_id);
            debug!(session_id = %session_id, "Using resume mode");
        } else {
            cmd.arg("--session-id");
            cmd.arg(session_id);
            debug!(session_id = %session_id, "Using new session mode");
        }

        debug!(
            command = ?cmd.as_std(),
            working_dir = %working_dir.display(),
            "Built Claude command"
        );

        // Check if Claude binary exists and is executable
        if !config.claude_binary_path.exists() {
            error!(
                session_id = %session_id,
                claude_binary = %config.claude_binary_path.display(),
                "Claude binary does not exist"
            );
            return Err(OrchestratorError::ClaudeSpawnFailed(
                format!("Claude binary does not exist: {}", config.claude_binary_path.display())
            ));
        }

        debug!(
            session_id = %session_id,
            claude_binary = %config.claude_binary_path.display(),
            "Claude binary exists, attempting to spawn process"
        );

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            error!(
                session_id = %session_id,
                working_dir = %working_dir.display(),
                claude_binary = %config.claude_binary_path.display(),
                error = %e,
                "Failed to spawn Claude process"
            );
            OrchestratorError::ClaudeSpawnFailed(format!("Failed to spawn Claude process: {e}"))
        })?;

        info!(
            session_id = %session_id,
            process_id = child.id(),
            "Claude process spawned successfully"
        );

        // Get stdin and stdout handles
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| {
                error!(session_id = %session_id, "Failed to get stdin handle from Claude process");
                OrchestratorError::ClaudeSpawnFailed("Failed to get stdin".into())
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| {
                error!(session_id = %session_id, "Failed to get stdout handle from Claude process");
                OrchestratorError::ClaudeSpawnFailed("Failed to get stdout".into())
            })?;

        debug!(session_id = %session_id, "Successfully obtained stdin and stdout handles");

        // Create channels for stdin writing
        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(100);
        debug!(session_id = %session_id, "Created stdin communication channel");

        // Send the first message immediately to trigger Claude's response
        debug!(
            session_id = %session_id,
            first_message_len = first_message.len(),
            "Sending first message to trigger Claude response"
        );
        
        // Compact the JSON to ensure it's a single line (Claude expects single-line JSON)
        let compacted_message = compact_json_message(first_message, "first_message")?;
        
        if let Err(e) = stdin_tx.send(compacted_message).await {
            error!(
                session_id = %session_id,
                error = %e,
                "Failed to send first message to Claude stdin"
            );
            return Err(OrchestratorError::ProcessCommunicationError(
                format!("Failed to send first message to Claude: {e}")
            ));
        }
        
        info!(
            session_id = %session_id,
            first_message_len = first_message.len(),
            "Successfully sent first message to Claude stdin"
        );

        // Spawn task to handle stdin writing
        let mut stdin_writer = stdin;
        let stdin_session_id = session_id.to_string();
        tokio::spawn(async move {
            info!(session_id = %stdin_session_id, "Starting stdin writer task");
            
            let mut messages_written = 0;
            while let Some(msg) = stdin_rx.recv().await {
                messages_written += 1;
                debug!(
                    session_id = %stdin_session_id,
                    message_number = messages_written,
                    message_length = msg.len(),
                    "Writing message to Claude stdin"
                );

                if let Err(e) = stdin_writer.write_all(msg.as_bytes()).await {
                    error!(
                        session_id = %stdin_session_id,
                        message_number = messages_written,
                        error = %e,
                        "Failed to write to Claude stdin"
                    );
                    break;
                }
                if let Err(e) = stdin_writer.write_all(b"\n").await {
                    error!(
                        session_id = %stdin_session_id,
                        message_number = messages_written,
                        error = %e,
                        "Failed to write newline to Claude stdin"
                    );
                    break;
                }
                if let Err(e) = stdin_writer.flush().await {
                    error!(
                        session_id = %stdin_session_id,
                        message_number = messages_written,
                        error = %e,
                        "Failed to flush Claude stdin"
                    );
                    break;
                }

                debug!(
                    session_id = %stdin_session_id,
                    message_number = messages_written,
                    "Successfully wrote message to Claude stdin"
                );
            }
            
            info!(
                session_id = %stdin_session_id,
                total_messages_written = messages_written,
                "Stdin writer task stopped"
            );
        });

        // Create channel for stdout reading
        let (stdout_tx, stdout_rx) = mpsc::channel::<String>(100);
        debug!(session_id = %session_id, "Created stdout communication channel");

        // Check if the process is still running before trying to read from it
        match child.try_wait() {
            Ok(None) => {
                debug!(
                    session_id = %session_id,
                    process_id = child.id(),
                    "Claude process is running, proceeding with stdout setup"
                );
            }
            Ok(Some(exit_status)) => {
                error!(
                    session_id = %session_id,
                    process_id = child.id(),
                    exit_status = ?exit_status,
                    "Claude process exited immediately after spawn"
                );
                return Err(OrchestratorError::ProcessCommunicationError(
                    format!("Claude process exited immediately with status: {:?}", exit_status)
                ));
            }
            Err(e) => {
                error!(
                    session_id = %session_id,
                    process_id = child.id(),
                    error = %e,
                    "Error checking Claude process status after spawn"
                );
            }
        }

        // Read the first line to get the actual session ID if resuming
        let mut reader = BufReader::new(stdout);
        let actual_session_id = if resume {
            debug!(session_id = %session_id, "Resume mode: reading first line to get actual session ID");
            
            // Add timeout to prevent hanging indefinitely
            let timeout_duration = std::time::Duration::from_secs(30);
            debug!(
                session_id = %session_id,
                timeout_seconds = timeout_duration.as_secs(),
                "Starting timed read of first line from Claude process"
            );
            
            let mut first_line = String::new();
            let read_result = tokio::time::timeout(timeout_duration, reader.read_line(&mut first_line)).await;
            
            match read_result {
                Ok(Ok(bytes_read)) => {
                    debug!(
                        session_id = %session_id,
                        bytes_read = bytes_read,
                        first_line = %first_line.trim(),
                        "Successfully read first line from Claude process"
                    );
                    
                    if bytes_read == 0 {
                        error!(
                            session_id = %session_id,
                            "Claude process closed stdout without sending initial response"
                        );
                        return Err(OrchestratorError::ProcessCommunicationError(
                            "Claude process closed stdout without sending initial response".into()
                        ));
                    }
                }
                Ok(Err(e)) => {
                    error!(
                        session_id = %session_id,
                        error = %e,
                        "IO error while reading first line from Claude process"
                    );
                    return Err(OrchestratorError::ProcessCommunicationError(e.to_string()));
                }
                Err(_) => {
                    error!(
                        session_id = %session_id,
                        timeout_seconds = timeout_duration.as_secs(),
                        "Timeout while waiting for first line from Claude process"
                    );
                    
                    // Check if the process is still running
                    match child.try_wait() {
                        Ok(None) => {
                            warn!(
                                session_id = %session_id,
                                process_id = child.id(),
                                "Claude process is still running but not responding, killing it"
                            );
                            if let Err(e) = child.kill().await {
                                error!(
                                    session_id = %session_id,
                                    error = %e,
                                    "Failed to kill unresponsive Claude process"
                                );
                            }
                        }
                        Ok(Some(exit_status)) => {
                            error!(
                                session_id = %session_id,
                                exit_status = ?exit_status,
                                "Claude process exited before sending initial response"
                            );
                        }
                        Err(e) => {
                            error!(
                                session_id = %session_id,
                                error = %e,
                                "Error checking Claude process status during timeout"
                            );
                        }
                    }
                    
                    return Err(OrchestratorError::ProcessCommunicationError(
                        format!("Timeout after {} seconds waiting for Claude process response", timeout_duration.as_secs())
                    ));
                }
            }

            // Parse the first line to extract the new session ID
            // Expected format: {"sessionId": "new-session-id", ...}
            let parsed: serde_json::Value = match serde_json::from_str(&first_line) {
                Ok(value) => {
                    debug!(
                        session_id = %session_id,
                        "Successfully parsed first line JSON from Claude"
                    );
                    value
                }
                Err(e) => {
                    error!(
                        session_id = %session_id,
                        first_line = %first_line.trim(),
                        error = %e,
                        "Failed to parse first line JSON from Claude"
                    );
                    return Err(OrchestratorError::ProcessCommunicationError(format!("Failed to parse first line from Claude: {e}")));
                }
            };

            let new_session_id = parsed["session_id"]
                .as_str()
                .ok_or_else(|| {
                    error!(
                        session_id = %session_id,
                        first_line = %first_line.trim(),
                        "No session_id field found in first line from Claude"
                    );
                    OrchestratorError::ProcessCommunicationError(
                        "No session_id in first line from Claude".into(),
                    )
                })?
                .to_string();

            info!(
                requested_session_id = %session_id,
                actual_session_id = %new_session_id,
                "Resume mode: received actual session ID from Claude"
            );

            new_session_id
        } else {
            debug!(session_id = %session_id, "New session mode: using provided session ID");
            session_id.to_string()
        };

        // Spawn task to handle stdout reading
        let stdout_session_id = actual_session_id.clone();
        tokio::spawn(async move {
            info!(session_id = %stdout_session_id, "Starting stdout reader task");
            
            let mut lines = reader.lines();
            let mut lines_read = 0;
            
            loop {
                debug!(
                    session_id = %stdout_session_id,
                    lines_read_so_far = lines_read,
                    "Waiting for next line from Claude stdout"
                );
                
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        lines_read += 1;
                        debug!(
                            session_id = %stdout_session_id,
                            line_number = lines_read,
                            line_length = line.len(),
                            line_preview = %line.chars().take(100).collect::<String>(),
                            "Read line from Claude stdout"
                        );

                        // Validate JSON before sending
                        if let Err(e) = serde_json::from_str::<serde_json::Value>(&line) {
                            error!(
                                session_id = %stdout_session_id,
                                line_number = lines_read,
                                line_content = %line,
                                error = %e,
                                "Invalid JSON received from Claude stdout"
                            );
                        } else {
                            debug!(
                                session_id = %stdout_session_id,
                                line_number = lines_read,
                                "Valid JSON received from Claude stdout"
                            );
                        }

                        debug!(
                            session_id = %stdout_session_id,
                            line_number = lines_read,
                            "Attempting to send line to stdout channel"
                        );

                        if let Err(e) = stdout_tx.send(line).await {
                            warn!(
                                session_id = %stdout_session_id,
                                line_number = lines_read,
                                error = %e,
                                "Failed to send stdout line to channel, receiver dropped"
                            );
                            break;
                        }

                        debug!(
                            session_id = %stdout_session_id,
                            line_number = lines_read,
                            "Successfully sent line to stdout channel"
                        );
                    }
                    Ok(None) => {
                        info!(
                            session_id = %stdout_session_id,
                            total_lines_read = lines_read,
                            "Claude process closed stdout (EOF reached)"
                        );
                        break;
                    }
                    Err(e) => {
                        error!(
                            session_id = %stdout_session_id,
                            total_lines_read = lines_read,
                            error = %e,
                            "Error reading from Claude stdout"
                        );
                        break;
                    }
                }
            }
            
            info!(
                session_id = %stdout_session_id,
                total_lines_read = lines_read,
                "Stdout reader task finished"
            );
        });

        info!(
            session_id = %actual_session_id,
            "Claude process initialization completed successfully"
        );

        Ok((
            Self {
                child,
                stdin_tx,
                stdout_rx,
            },
            actual_session_id,
        ))
    }

    /// Writes a message to the Claude process stdin.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be sent to the stdin writer.
    #[instrument(skip(self), fields(message_len = message.len()))]
    #[allow(dead_code)] // Public API for Claude process management
    pub async fn write(&self, message: &str) -> Result<()> {
        debug!(
            message_length = message.len(),
            "Sending message to Claude process"
        );
        
        // Compact JSON to ensure single-line format
        let compacted_message = compact_json_message(message, "write_method")
            .map_err(|e| anyhow::anyhow!("Failed to compact message: {}", e))?;
            
        match self.stdin_tx.send(compacted_message).await {
            Ok(()) => {
                debug!(
                    message_length = message.len(),
                    "Successfully sent message to Claude process"
                );
            }
            Err(e) => {
                error!(
                    message_length = message.len(),
                    error = %e,
                    "Failed to send message to Claude process stdin writer"
                );
                return Err(anyhow::anyhow!("Failed to send message to stdin writer: {}", e));
            }
        }
        Ok(())
    }

    #[instrument(skip(self))]
    #[allow(dead_code)] // Public API for Claude process management
    pub async fn read(&mut self) -> Option<String> {
        match self.stdout_rx.recv().await {
            Some(line) => {
                debug!(
                    line_length = line.len(),
                    "Read line from Claude process stdout"
                );
                Some(line)
            }
            None => {
                debug!("Claude process stdout channel closed");
                None
            }
        }
    }

    /// Kills the Claude process.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be killed.
    #[instrument(skip(self))]
    #[allow(dead_code)] // Public API for Claude process management
    pub async fn kill(mut self) -> Result<()> {
        info!(process_id = self.child.id(), "Killing Claude process");
        match self.child.kill().await {
            Ok(()) => {
                info!(process_id = self.child.id(), "Claude process killed successfully");
                Ok(())
            }
            Err(e) => {
                error!(
                    process_id = self.child.id(),
                    error = %e,
                    "Failed to kill Claude process"
                );
                Err(anyhow::anyhow!("Failed to kill process: {}", e))
            }
        }
    }

    #[instrument(skip(self))]
    #[allow(dead_code)] // Public API for Claude process management
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => {
                debug!(process_id = self.child.id(), "Claude process is still running");
                true
            }
            Ok(Some(exit_status)) => {
                info!(
                    process_id = self.child.id(),
                    exit_status = ?exit_status,
                    "Claude process has exited"
                );
                false
            }
            Err(e) => {
                error!(
                    process_id = self.child.id(),
                    error = %e,
                    "Error checking Claude process status"
                );
                false
            }
        }
    }
}

#[instrument(skip(_session, process, broadcast_tx))]
#[allow(dead_code)] // Planned functionality for handling Claude output
pub async fn handle_claude_output(
    _session: &Session,
    mut process: ClaudeProcess,
    broadcast_tx: mpsc::Sender<(String, Option<String>)>,
) {
    info!("Starting Claude output handler");
    
    let mut lines_processed = 0;
    while let Some(line) = process.read().await {
        lines_processed += 1;
        debug!(
            line_number = lines_processed,
            line_length = line.len(),
            "Processing line from Claude output"
        );

        // Validate JSON
        if let Err(e) = serde_json::from_str::<serde_json::Value>(&line) {
            error!(
                line_number = lines_processed,
                line_content = %line,
                error = %e,
                "Received invalid JSON from Claude, killing process"
            );
            
            // Kill the process and notify clients
            if let Err(e) = process.kill().await {
                error!(error = %e, "Failed to kill Claude process after invalid JSON");
            }
            if let Err(e) = broadcast_tx.send(("DISCONNECT".to_string(), None)).await {
                error!(error = %e, "Failed to send disconnect notification after invalid JSON");
            }
            break;
        }

        debug!(
            line_number = lines_processed,
            "Valid JSON received, broadcasting to clients"
        );

        // Broadcast to all clients
        if let Err(e) = broadcast_tx.send((line, None)).await {
            warn!(
                line_number = lines_processed,
                error = %e,
                "Broadcast channel closed, stopping output processing"
            );
            break;
        }
    }

    info!(
        total_lines_processed = lines_processed,
        "Claude output handler stopped, sending final disconnect notification"
    );

    // Process has ended
    if let Err(e) = broadcast_tx.send(("DISCONNECT".to_string(), None)).await {
        error!(error = %e, "Failed to send final disconnect notification");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    async fn create_mock_claude_script(temp_dir: &TempDir) -> PathBuf {
        let script_path = temp_dir.path().join("mock_claude");
        let script_content = r#"#!/bin/bash
echo '{"session_id": "test-session", "type": "start"}'
while read line; do
    echo '{"type": "echo", "content": "'$line'"}'
done
"#;
        fs::write(&script_path, script_content).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        script_path
    }

    #[tokio::test]
    async fn test_spawn_claude_process() {
        let temp_dir = TempDir::new().unwrap();
        let mock_claude = create_mock_claude_script(&temp_dir).await;
        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        let config = Config {
            claude_binary_path: mock_claude,
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: temp_dir.path().to_path_buf(),
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let (mut process, session_id) =
            ClaudeProcess::spawn(&config, "test-session", &working_dir, false, r#"{"role": "user", "content": "Hello Claude"}"#)
                .await
                .unwrap();

        assert_eq!(session_id, "test-session");

        // Test writing and reading
        process.write(r#"{"role": "user", "content": "Hello Claude"}"#).await.unwrap();

        // Give the mock process time to respond
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if let Some(response) = process.read().await {
            // Just verify we got some response, the exact format may vary
            assert!(!response.is_empty());
        }

        process.kill().await.unwrap();
    }

    #[tokio::test]
    async fn test_resume_session() {
        let temp_dir = TempDir::new().unwrap();

        // Create a mock claude that returns a new session ID on resume
        let script_path = temp_dir.path().join("mock_claude_resume");
        let script_content = r#"#!/bin/bash
# Check if --resume is in the arguments
if [[ "$*" == *"--resume"* ]]; then
    echo '{"session_id": "new-session-789", "type": "resume"}'
else
    # Find the session ID after --session-id
    session_id=""
    for i in "${@}"; do
        if [[ "$prev_arg" == "--session-id" ]]; then
            session_id="$i"
            break
        fi
        prev_arg="$i"
    done
    echo '{"session_id": "'$session_id'", "type": "start"}'
fi
while read line; do
    echo '{"type": "echo", "content": "'$line'"}'
done
"#;
        fs::write(&script_path, script_content).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).unwrap();
        }

        let working_dir = temp_dir.path().join("work");
        fs::create_dir_all(&working_dir).unwrap();

        let config = Config {
            claude_binary_path: script_path,
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: temp_dir.path().to_path_buf(),
            shutdown_timeout: std::time::Duration::from_secs(30),
        };

        let (process, actual_session_id) =
            ClaudeProcess::spawn(&config, "old-session-456", &working_dir, true, r#"{"role": "user", "content": "Resume session"}"#)
                .await
                .unwrap();

        assert_eq!(actual_session_id, "new-session-789");

        process.kill().await.unwrap();
    }
}
