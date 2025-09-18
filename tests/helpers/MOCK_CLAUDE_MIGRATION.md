# Mock Claude Migration Guide

## Overview
The mock Claude binary has been redesigned from a complex bash script to a minimal Python script that echoes JSON input. This makes tests more explicit and easier to understand.

## New Design

### mock_claude.py
- **Default behavior**: Echoes any JSON input it receives
- **Control commands**: Special JSON objects that trigger specific behaviors:
  - `{"control": "exit", "code": 1}`: Exit with specified code
  - `{"control": "sleep", "duration": 1.5}`: Sleep for specified duration  
  - `{"control": "write_file", "path": "/path/to/file", "content": "data"}`: Write content to file

### Changes to MockClaude struct
- Removed `create_approval_binary()` - no longer needed, same binary for all tests
- Removed `create_failing_binary()` - tests use exit control command instead
- Removed `create_test_session_file()` - moved to individual test files where needed

## Test Migration Examples

### 1. Tests that need session files on disk
For tests that check if the service can list historical sessions:

```rust
// Add helper function to test file
fn create_test_session_file(projects_dir: &std::path::Path, project_name: &str, session_id: &str, cwd: &str) {
    let project_dir = projects_dir.join(project_name);
    fs::create_dir_all(&project_dir).unwrap();
    
    let session_file = project_dir.join(format!("{}.jsonl", session_id));
    let content = format!(r#"{{"sessionId": "{}", "cwd": "{}", "type": "start"}}
{{"type": "user", "message": {{"role": "user", "content": "Hello Claude"}}}}
"#, session_id, cwd);
    
    fs::write(session_file, content).unwrap();
}

// Use in test
create_test_session_file(&server.mock.projects_dir, "project1", "session-123", "/home/user/project1");
```

### 2. Tests that simulate approval flows
Instead of special approval binary, tests send approval JSON directly:

```rust
// Send a control request
let control_request = r#"{"type": "control_request", "request_id": "test-123", "request": {"subtype": "can_use_tool", "tool_name": "Read"}}"#;
ws.send(Message::Text(control_request.to_string())).await.unwrap();

// Send a control response
let control_response = r#"{"type": "control_response", "response": {"request_id": "test-123", "subtype": "success", "response": {"behavior": "allow"}}}"#;
ws.send(Message::Text(control_response.to_string())).await.unwrap();
```

### 3. Tests that simulate process failure
Instead of `create_failing_binary()`, send exit command:

```rust
// Send exit command to simulate process death
let exit_command = r#"{"control": "exit", "code": 1}"#;
ws.send(Message::Text(exit_command.to_string())).await.unwrap();
```

### 4. Tests that need specific responses
With the echo design, tests control exactly what Claude returns:

```rust
// Send session start response
let session_start = r#"{"session_id": "test-session", "type": "start", "cwd": "/home/user"}"#;
ws.send(Message::Text(session_start.to_string())).await.unwrap();

// Mock will echo it back, service will process it
```

## Benefits of New Design
1. **Simplicity**: Single Python script instead of multiple bash variants
2. **Explicitness**: Tests clearly show what they're testing
3. **Flexibility**: Tests can send any JSON to simulate any scenario
4. **Maintainability**: No complex bash parsing logic to maintain
5. **Debugging**: Easy to see what's being sent and received in tests