use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct MockClaude {
    pub temp_dir: TempDir,
    pub binary_path: PathBuf,
    pub projects_dir: PathBuf,
}

impl MockClaude {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let binary_path = temp_dir.path().join("mock_claude");
        let projects_dir = temp_dir.path().join("projects");

        // Create the mock Claude binary script
        let script_content = r#"#!/bin/bash

# Parse command line arguments
RESUME=false
SESSION_ID=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --resume)
            RESUME=true
            shift
            ;;
        --session | --session-id)
            SESSION_ID="$2"
            shift 2
            ;;
        --output-format | --input-format | --permission-prompt-tool)
            # Skip these flags and their values
            shift 2
            ;;
        --verbose | --print)
            # Skip these boolean flags
            shift
            ;;
        stdio)
            # Skip the stdio argument for --permission-prompt-tool
            shift
            ;;
        *)
            # If it's not a flag, assume it's a session ID for resume
            if [[ "$RESUME" == "true" ]]; then
                SESSION_ID="$1"
            fi
            shift
            ;;
    esac
done

# Generate a new session ID if resuming
if [[ "$RESUME" == "true" ]]; then
    NEW_SESSION_ID="resumed-$(date +%s)-$(shuf -i 100-999 -n 1)"
    ACTUAL_SESSION_ID="$NEW_SESSION_ID"
    echo "{\"session_id\": \"$NEW_SESSION_ID\", \"type\": \"resume\", \"originalSessionId\": \"$SESSION_ID\"}"
else
    ACTUAL_SESSION_ID="$SESSION_ID"
    echo "{\"session_id\": \"$SESSION_ID\", \"type\": \"start\", \"cwd\": \"$(pwd)\"}"
fi

# Create session file for session file waiting logic
WORKING_DIR="$(pwd)"
PROJECTS_DIR="${CLAUDE_PROJECTS_DIR}"

if [ -n "$PROJECTS_DIR" ]; then
    # Create project subdirectory based on working directory path
    ENCODED_DIR=$(echo "$WORKING_DIR" | tr '/\\:' '___')
    PROJECT_SUBDIR="$PROJECTS_DIR/$ENCODED_DIR"
    
    # Create directory if it doesn't exist
    mkdir -p "$PROJECT_SUBDIR"
    
    # Create session file with initial content using the actual session ID
    SESSION_FILE="$PROJECT_SUBDIR/$ACTUAL_SESSION_ID.jsonl"
    echo "{\"sessionId\": \"$ACTUAL_SESSION_ID\", \"cwd\": \"$WORKING_DIR\", \"type\": \"start\"}" > "$SESSION_FILE"
fi

# Simulate Claude's interactive behavior
FIRST_MESSAGE=true
while IFS= read -r line; do
    # Parse JSON input (basic simulation)
    if [[ "$line" == *"\"content\":"* ]]; then
        # Extract content from JSON (simple grep-based approach)
        content=$(echo "$line" | sed -n 's/.*"content"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
        if [[ -n "$content" ]]; then
            # Simulate Claude's response
            echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"Mock Claude received: $content\"}]}}"
            
            # After the first message, send a ready signal for WebSocket clients
            if [[ "$FIRST_MESSAGE" == "true" ]]; then
                FIRST_MESSAGE=false
                sleep 0.1
                echo "{\"type\": \"session_ready\", \"message\": \"Session is ready for WebSocket connections\"}"
            fi
        else
            # If simple extraction fails, just respond that we got a message
            echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"Mock Claude received multiline WebSocket message\"}]}}"
        fi
    else
        # Echo back any other JSON
        echo "{\"type\": \"echo\", \"data\": $line}"
    fi
    
    # Small delay to simulate processing
    sleep 0.1
done
"#;

        fs::write(&binary_path, script_content).expect("Failed to write mock Claude script");

        // Make the script executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms).unwrap();
        }

        // Ensure file is fully written and synced to disk
        std::thread::sleep(std::time::Duration::from_millis(5));

        // Create projects directory
        fs::create_dir_all(&projects_dir).expect("Failed to create projects directory");

        MockClaude {
            temp_dir,
            binary_path,
            projects_dir,
        }
    }

    pub fn create_test_session_file(&self, project_name: &str, session_id: &str, cwd: &str) {
        let project_dir = self.projects_dir.join(project_name);
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

    pub fn create_complete_session_file(&self, project_name: &str, session_id: &str, cwd: &str) {
        let project_dir = self.projects_dir.join(project_name);
        fs::create_dir_all(&project_dir).unwrap();

        let session_file = project_dir.join(format!("{}.jsonl", session_id));

        let content = format!(
            r#"{{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"{}","sessionId":"{}","version":"1.0.65","gitBranch":"master","type":"user","message":{{"role":"user","content":"fix all the clippy warnings"}},"uuid":"30644cc3-c5cd-4e9e-953a-bbe299394703","timestamp":"2025-09-09T21:51:18.091Z"}}
{{"parentUuid":"30644cc3-c5cd-4e9e-953a-bbe299394703","isSidechain":false,"userType":"external","cwd":"{}","sessionId":"{}","version":"1.0.65","gitBranch":"master","message":{{"id":"msg_01SZg91CWiohGz1EGpKNtiNc","type":"message","role":"assistant","model":"claude-sonnet-4-20250514","content":[{{"type":"text","text":"I'll use the rust-linter-fixer agent..."}}],"stop_reason":null,"stop_sequence":null,"usage":{{"input_tokens":4,"cache_creation_input_tokens":17902,"cache_read_input_tokens":0,"cache_creation":{{"ephemeral_5m_input_tokens":17902,"ephemeral_1h_input_tokens":0}},"output_tokens":3,"service_tier":"standard"}}}},"requestId":"req_011CSyZqUfBhmGuMy8ymqeNp","type":"assistant","uuid":"44f23e72-dd74-467a-b7b2-bd1bb905abe4","timestamp":"2025-09-09T21:51:21.682Z"}}
{{"type":"system","message":"Session started","timestamp":"2025-09-09T21:51:22.000Z","sessionId":"{}"}}"#,
            cwd, session_id, cwd, session_id, session_id
        );

        fs::write(session_file, content).unwrap();
    }

    pub fn setup_env_vars(&self) {
        env::set_var("CLAUDE_BINARY_PATH", &self.binary_path);
        env::set_var("CLAUDE_PROJECTS_DIR", &self.projects_dir);
        env::set_var("HTTP_LISTEN_ADDRESS", "127.0.0.1:0"); // Use port 0 for random free port
        env::set_var("SHUTDOWN_TIMEOUT", "1");
    }

    #[allow(dead_code)] // Test helper method
    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    #[allow(dead_code)] // Test helper method
    pub fn projects_dir(&self) -> &Path {
        &self.projects_dir
    }

    #[allow(dead_code)] // Test helper method
    pub fn create_approval_binary(&self) -> PathBuf {
        let approval_binary_path = self.temp_dir.path().join("approval_mock_claude");
        
        // Create a mock Claude binary that can simulate tool approval requests
        let script_content = r#"#!/bin/bash -x

# Parse command line arguments
RESUME=false
SESSION_ID=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --resume)
            RESUME=true
            shift
            ;;
        --session | --session-id)
            SESSION_ID="$2"
            shift 2
            ;;
        --output-format | --input-format | --permission-prompt-tool)
            # Skip these flags and their values
            shift 2
            ;;
        --verbose | --print)
            # Skip these boolean flags
            shift
            ;;
        stdio)
            # Skip the stdio argument for --permission-prompt-tool
            shift
            ;;
        *)
            if [[ "$RESUME" == "true" ]]; then
                SESSION_ID="$1"
            fi
            shift
            ;;
    esac
done

# Generate a new session ID if resuming
if [[ "$RESUME" == "true" ]]; then
    NEW_SESSION_ID="resumed-$(date +%s)-$(shuf -i 100-999 -n 1)"
    ACTUAL_SESSION_ID="$NEW_SESSION_ID"
    echo "{\"session_id\": \"$NEW_SESSION_ID\", \"type\": \"resume\", \"originalSessionId\": \"$SESSION_ID\"}"
else
    ACTUAL_SESSION_ID="$SESSION_ID"
    echo "{\"session_id\": \"$SESSION_ID\", \"type\": \"start\", \"cwd\": \"$(pwd)\"}"
fi

# Create session file for session file waiting logic
WORKING_DIR="$(pwd)"
PROJECTS_DIR="${CLAUDE_PROJECTS_DIR}"

if [ -n "$PROJECTS_DIR" ]; then
    # Create project subdirectory based on working directory path
    ENCODED_DIR=$(echo "$WORKING_DIR" | tr '/\\:' '___')
    PROJECT_SUBDIR="$PROJECTS_DIR/$ENCODED_DIR"
    
    # Create directory if it doesn't exist
    mkdir -p "$PROJECT_SUBDIR"
    
    # Create session file with initial content
    SESSION_FILE="$PROJECT_SUBDIR/$ACTUAL_SESSION_ID.jsonl"
    echo "{\"sessionId\": \"$ACTUAL_SESSION_ID\", \"cwd\": \"$WORKING_DIR\", \"type\": \"start\"}" > "$SESSION_FILE"
fi

# Validation function for control_response format
validate_control_response() {
    local response="$1"
    local request_id="$2"
    
    # Check if response is valid JSON
    if ! echo "$response" | jq . >/dev/null 2>&1; then
        echo "VALIDATION_ERROR: Invalid JSON format"
        return 1
    fi
    
    # Extract and validate required fields
    local type=$(echo "$response" | jq -r '.type // empty')
    local subtype=$(echo "$response" | jq -r '.response.subtype // empty')
    local resp_request_id=$(echo "$response" | jq -r '.response.request_id // empty')
    local behavior=$(echo "$response" | jq -r '.response.response.behavior // empty')
    
    # Validate required fields
    if [[ "$type" != "control_response" ]]; then
        echo "VALIDATION_ERROR: Missing or invalid 'type' field, expected 'control_response', got '$type'"
        return 1
    fi
    
    if [[ "$subtype" != "success" && "$subtype" != "error" ]]; then
        echo "VALIDATION_ERROR: Missing or invalid 'response.subtype' field, expected 'success' or 'error', got '$subtype'"
        return 1
    fi
    
    if [[ -n "$request_id" && "$resp_request_id" != "$request_id" ]]; then
        echo "VALIDATION_ERROR: Request ID mismatch, expected '$request_id', got '$resp_request_id'"
        return 1
    fi
    
    if [[ "$subtype" == "success" ]]; then
        if [[ "$behavior" != "allow" && "$behavior" != "deny" ]]; then
            echo "VALIDATION_ERROR: Invalid 'response.response.behavior' field, expected 'allow' or 'deny', got '$behavior'"
            return 1
        fi
        
        # For deny responses, check if message is present
        if [[ "$behavior" == "deny" ]]; then
            local message=$(echo "$response" | jq -r '.response.response.message // empty')
            if [[ -z "$message" ]]; then
                echo "VALIDATION_ERROR: Missing 'response.response.message' field for deny response"
                return 1
            fi
        fi
    fi
    
    echo "VALIDATION_SUCCESS: control_response format is valid"
    return 0
}

# Simulate Claude's interactive behavior with tool approval requests
while IFS= read -r line; do
    # Check for special trigger messages that should generate tool approval requests
    if [[ "$line" == *"trigger_tool_request"* ]]; then
        # Send a control_request for tool approval
        request_id="test-request-$(date +%s)-$RANDOM"
        echo "{\"type\": \"control_request\", \"request_id\": \"$request_id\", \"request\": {\"subtype\": \"can_use_tool\", \"tool_name\": \"Read\", \"input\": {\"file_path\": \"/etc/passwd\"}, \"permission_suggestions\": [{\"type\": \"addRules\", \"rules\": [{\"toolName\": \"Read\", \"ruleContent\": \"/etc/*\"}], \"behavior\": \"allow\", \"destination\": \"session\"}]}}"
        
        # Wait for control_response from server
        read -r response
        
        # Validate the control_response format
        if validate_result=$(validate_control_response "$response" "$request_id"); then
            if [[ "$response" == *"\"behavior\":\"allow\""* ]]; then
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"APPROVAL_TEST_SUCCESS_READ\"}]}}"
            else
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"APPROVAL_TEST_DENIED_READ\"}]}}"
            fi
        else
            echo "MOCK_CLAUDE_VALIDATION_FAILED: $validate_result" >&2
            exit 1
        fi
    elif [[ "$line" == *"trigger_bash_request"* ]]; then
        # Send a control_request for Bash tool
        request_id="bash-request-$(date +%s)-$RANDOM"
        echo "{\"type\": \"control_request\", \"request_id\": \"$request_id\", \"request\": {\"subtype\": \"can_use_tool\", \"tool_name\": \"Bash\", \"input\": {\"command\": \"ls -la\", \"description\": \"List directory contents\"}, \"permission_suggestions\": []}}"
        
        # Wait for control_response from server
        read -r response
        
        # Validate the control_response format
        if validate_result=$(validate_control_response "$response" "$request_id"); then
            if [[ "$response" == *"\"behavior\":\"allow\""* ]]; then
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"APPROVAL_TEST_SUCCESS_BASH\"}]}}"
            else
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"APPROVAL_TEST_DENIED_BASH\"}]}}"
            fi
        else
            echo "MOCK_CLAUDE_VALIDATION_FAILED: $validate_result" >&2
            exit 1
        fi
    elif [[ "$line" == *"trigger_write_request"* ]]; then
        # Send a control_request for Write tool
        request_id="write-request-$(date +%s)-$RANDOM"
        echo "{\"type\": \"control_request\", \"request_id\": \"$request_id\", \"request\": {\"subtype\": \"can_use_tool\", \"tool_name\": \"Write\", \"input\": {\"file_path\": \"/tmp/output.txt\", \"content\": \"test data\"}, \"permission_suggestions\": []}}"
        
        # Wait for control_response from server
        read -r response
        
        # Validate the control_response format
        if validate_result=$(validate_control_response "$response" "$request_id"); then
            if [[ "$response" == *"\"behavior\":\"allow\""* ]]; then
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"APPROVAL_TEST_SUCCESS_WRITE\"}]}}"
            else
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"APPROVAL_TEST_DENIED_WRITE\"}]}}"
            fi
        else
            echo "MOCK_CLAUDE_VALIDATION_FAILED: $validate_result" >&2
            exit 1
        fi
    else
        # Regular message processing
        if [[ "$line" == *"\"content\":"* ]]; then
            # Extract content from JSON (simple grep-based approach)
            content=$(echo "$line" | sed -n 's/.*"content"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
            if [[ -n "$content" ]]; then
                # Simulate Claude's response
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"Mock Claude received: $content\"}]}}"
            else
                # If simple extraction fails, just respond that we got a message
                echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"Mock Claude received multiline WebSocket message\"}]}}"
            fi
        else
            # Echo back any other JSON
            echo "{\"type\": \"echo\", \"data\": $line}"
        fi
    fi
    
    # Small delay to simulate processing
    sleep 0.1
done
"#;

        fs::write(&approval_binary_path, script_content).expect("Failed to write approval mock Claude script");

        // Make the script executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&approval_binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&approval_binary_path, perms).unwrap();
        }

        // Ensure file is fully written and synced to disk
        std::thread::sleep(std::time::Duration::from_millis(5));

        approval_binary_path
    }

    #[allow(dead_code)] // Test helper method
    pub fn create_failing_binary(&self) -> PathBuf {
        let failing_binary_path = self.temp_dir.path().join("failing_mock_claude");
        
        // Create a mock Claude binary that exits after a short time
        let script_content = r#"#!/bin/bash

# Parse command line arguments
RESUME=false
SESSION_ID=""
while [[ $# -gt 0 ]]; do
    case $1 in
        --resume)
            RESUME=true
            shift
            ;;
        --session | --session-id)
            SESSION_ID="$2"
            shift 2
            ;;
        --output-format | --input-format | --permission-prompt-tool)
            # Skip these flags and their values
            shift 2
            ;;
        --verbose | --print)
            # Skip these boolean flags
            shift
            ;;
        stdio)
            # Skip the stdio argument for --permission-prompt-tool
            shift
            ;;
        *)
            if [[ "$RESUME" == "true" ]]; then
                SESSION_ID="$1"
            fi
            shift
            ;;
    esac
done

# Generate a new session ID if resuming
if [[ "$RESUME" == "true" ]]; then
    NEW_SESSION_ID="resumed-$(date +%s)-$(shuf -i 100-999 -n 1)"
    echo "{\"session_id\": \"$NEW_SESSION_ID\", \"type\": \"resume\", \"originalSessionId\": \"$SESSION_ID\"}"
else
    echo "{\"session_id\": \"$SESSION_ID\", \"type\": \"start\", \"cwd\": \"$(pwd)\"}"
fi

# Send one response then exit to simulate process death
read -r line
if [[ "$line" == *"\"content\":"* ]]; then
    content=$(echo "$line" | sed -n 's/.*"content"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
    if [[ -n "$content" ]]; then
        echo "{\"type\": \"assistant\", \"message\": {\"role\": \"assistant\", \"content\": [{\"type\": \"text\", \"text\": \"Mock Claude received: $content\"}]}}"
    fi
fi

# Exit unexpectedly to simulate process death  
exit 1
"#;

        fs::write(&failing_binary_path, script_content).expect("Failed to write failing mock Claude script");

        // Make the script executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&failing_binary_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&failing_binary_path, perms).unwrap();
        }

        // Ensure file is fully written and synced to disk
        std::thread::sleep(std::time::Duration::from_millis(5));

        failing_binary_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_mock_claude_creation() {
        let mock = MockClaude::new();
        assert!(mock.binary_path.exists());
        assert!(mock.projects_dir.exists());
    }

    #[test]
    fn test_mock_claude_execution() {
        let mock = MockClaude::new();
        let working_dir = mock.temp_dir.path().join("test_work");
        fs::create_dir_all(&working_dir).unwrap();

        // Add a small delay to prevent file system race conditions
        std::thread::sleep(std::time::Duration::from_millis(10));

        let output = Command::new(&mock.binary_path)
            .arg("--session")
            .arg("test-session")
            .current_dir(&working_dir)
            .output()
            .expect("Failed to execute mock Claude");

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("test-session"));
        assert!(stdout.contains("start"));
    }

    #[test]
    fn test_mock_claude_resume() {
        let mock = MockClaude::new();
        let working_dir = mock.temp_dir.path().join("test_work");
        fs::create_dir_all(&working_dir).unwrap();

        // Add a small delay to prevent file system race conditions
        std::thread::sleep(std::time::Duration::from_millis(10));

        let output = Command::new(&mock.binary_path)
            .arg("--resume")
            .arg("old-session")
            .current_dir(&working_dir)
            .output()
            .expect("Failed to execute mock Claude");

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("resumed-"));
        assert!(stdout.contains("resume"));
        assert!(stdout.contains("old-session"));
    }

    #[test]
    fn test_session_file_creation() {
        let mock = MockClaude::new();
        mock.create_test_session_file("test-project", "session-123", "/home/user/test");

        let session_file = mock
            .projects_dir
            .join("test-project")
            .join("session-123.jsonl");
        assert!(session_file.exists());

        let content = fs::read_to_string(session_file).unwrap();
        assert!(content.contains("session-123"));
        assert!(content.contains("/home/user/test"));
        assert!(content.contains("Hello Claude"));
    }
}
