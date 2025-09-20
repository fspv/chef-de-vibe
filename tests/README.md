# Chef de Vibe Test Suite

This directory contains integration and end-to-end tests for the Chef de Vibe orchestration server.

## Using the Mock Claude Binary

The test suite includes a mock Claude binary (`tests/helpers/mock_claude.py`) that simulates Claude's behavior for testing purposes. This mock binary supports special control commands that allow tests to create session journal files and simulate various Claude behaviors.

### Control Commands

The mock Claude binary accepts JSON messages on stdin and responds on stdout. It supports several control commands:

- **`write_file`**: Writes content to a file (useful for creating session journal files)
- **`sleep`**: Pauses execution for a specified duration
- **`exit`**: Terminates with a specific exit code

### Creating Session Journal Files

When testing session management features, you often need to simulate existing Claude session journal files. The mock Claude binary's `write_file` control command allows you to create these files during test execution.

#### Basic Pattern

1. **Session ID Response First**: Always send the session ID response first to complete the handshake
2. **Write Journal File**: Use the `write_file` control command to create the journal file
3. **Journal File Location**: Files should be written to `{projects_dir}/{project_folder}/{session_id}.jsonl`
   - `project_folder` is derived from the working directory path with `/` replaced by `-` and prefixed with `-`

#### Minimal Example: Creating a Simple Session

```rust
#[tokio::test]
async fn test_session_with_journal_file() {
    let server = TestServer::new().await;
    let client = Client::new();
    
    // Create working directory
    let work_dir = server.mock.temp_dir.path().join("test_session");
    fs::create_dir_all(&work_dir).unwrap();
    
    let session_id = "test-session-123";
    
    // Calculate where the journal file should go
    let project_folder = format!("-{}", work_dir.display().to_string().replace('/', "-"));
    let project_path = server.mock.projects_dir.join(&project_folder);
    fs::create_dir_all(&project_path).unwrap();
    let session_file_path = project_path.join(format!("{}.jsonl", session_id));
    
    // Prepare journal file content
    let journal_content = format!(
        r#"{{"sessionId":"{}","type":"user","message":{{"role":"user","content":"Hello"}},"timestamp":"2025-09-20T10:00:00Z"}}"#,
        session_id
    );
    
    // Bootstrap messages for session creation
    let bootstrap_messages = vec![
        // 1. First: respond with session ID (required for handshake)
        format!(r#"{{"sessionId": "{}"}}"#, session_id),
        
        // 2. Then: write the journal file
        serde_json::json!({
            "control": "write_file",
            "path": session_file_path.to_string_lossy(),
            "content": journal_content
        }).to_string(),
    ];
    
    // Create the session
    let create_request = CreateSessionRequest {
        session_id: session_id.to_string(),
        working_dir: work_dir.clone(),
        resume: false,
        bootstrap_messages,
    };
    
    let response = client
        .post(format!("{}/api/v1/sessions", server.base_url))
        .json(&create_request)
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 200);
}
```

#### Example: Simulating a Conversation

```rust
// Create a more complex journal with user and assistant messages
let journal_content = format!(
    r#"{{"sessionId":"{}","type":"user","message":{{"role":"user","content":"What is 2+2?"}},"timestamp":"2025-09-20T10:00:00Z"}}
{{"sessionId":"{}","type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"4"}}]}},"timestamp":"2025-09-20T10:00:01Z"}}"#,
    session_id,
    session_id
);

let bootstrap_messages = vec![
    format!(r#"{{"sessionId": "{}"}}"#, session_id),
    serde_json::json!({
        "control": "write_file",
        "path": session_file_path.to_string_lossy(),
        "content": journal_content
    }).to_string(),
];
```

### Important Notes

1. **Order Matters**: The session ID response must come before any control commands for the handshake to complete successfully.

2. **File Paths**: The mock Claude binary will create parent directories automatically when using `write_file`.

3. **JSON Format**: All messages must be valid JSON. The mock binary will echo back non-control messages and execute control commands silently.

4. **Active Sessions**: When testing active sessions, the journal files created this way will be discovered by the session discovery system and can be used to verify features like first-user-message fallback summaries.

5. **Cleanup**: The test framework automatically cleans up temporary directories after tests complete.

## General tips

- Use `RUST_LOG=debug` when running tests to see detailed logs of what the mock Claude binary is doing
- Never run the entire test suite. It takes too long to run and you will time out. Instead run specifict test suites, for example `RUST_LOG=debug cargo test --test approval_system` or even individual tests.
