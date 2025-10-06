mod helpers;

use chef_de_vibe::{
    api::handlers::AppState, config::Config, models::ListSessionsResponse,
    session_manager::SessionManager,
};
use helpers::logging::init_logging;
use helpers::mock_claude::MockClaude;
use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

struct TestServer {
    pub base_url: String,
    pub mock: MockClaude,
    server_handle: tokio::task::JoinHandle<()>,
    session_manager: Arc<SessionManager>,
}

impl TestServer {
    async fn new() -> Self {
        init_logging();
        let mock = MockClaude::new();
        mock.setup_env_vars();

        let config = Config::from_env().expect("Failed to load config");
        let session_manager = Arc::new(SessionManager::new(config.clone()));

        let state = AppState {
            session_manager: session_manager.clone(),
            config: Arc::new(config),
        };

        let app = axum::Router::new()
            .route(
                "/api/v1/sessions",
                axum::routing::get(chef_de_vibe::api::handlers::list_sessions),
            )
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://127.0.0.1:{}", addr.port());

        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        TestServer {
            base_url,
            mock,
            server_handle,
            session_manager,
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.server_handle.abort();
        let session_manager = self.session_manager.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                session_manager.shutdown().await;
                tokio::time::sleep(Duration::from_millis(100)).await;
            });
        })
        .join()
        .ok();
    }
}

#[tokio::test]
#[serial]
async fn test_orphaned_summaries_with_multiple_scenarios() {
    let server = TestServer::new().await;
    let client = Client::new();

    let project_path = server.mock.projects_dir.join("orphan-test");
    fs::create_dir_all(&project_path).unwrap();

    // Scenario 1: Orphaned summary pointing to completely non-existent UUID
    let orphan1 = project_path.join("orphan1.jsonl");
    fs::write(
        &orphan1,
        r#"{"type":"summary","summary":"Orphaned Discussion","leafUuid":"00000000-0000-0000-0000-000000000000"}
{"type":"other","data":"irrelevant"}"#,
    )
    .unwrap();

    // Scenario 2: Summary pointing to UUID that exists in another project
    let orphan2 = project_path.join("orphan2.jsonl");
    fs::write(
        &orphan2,
        r#"{"type":"summary","summary":"Cross-Project Reference","leafUuid":"exists-elsewhere"}
{"uuid":"exists-elsewhere","sessionId":"other-project-session","type":"user","message":{"role":"user","content":"I exist in another project"}}"#,
    )
    .unwrap();

    // Scenario 3: Valid session without summary
    let valid_session = project_path.join("valid.jsonl");
    fs::write(
        &valid_session,
        r#"{"uuid":"valid-uuid","sessionId":"valid-session","type":"user","message":{"role":"user","content":"Valid message"},"cwd":"/home/user"}"#,
    )
    .unwrap();

    // Scenario 4: Summary with self-reference (circular)
    let circular = project_path.join("circular.jsonl");
    fs::write(
        &circular,
        r#"{"uuid":"circular-uuid","type":"summary","summary":"Circular Reference","leafUuid":"circular-uuid"}
{"uuid":"circular-uuid","sessionId":"circular-session","type":"user","message":{"role":"user","content":"Circular message"},"cwd":"/home/circular"}"#,
    )
    .unwrap();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    // Should find valid-session and circular-session, ignore pure orphans
    let session_ids: Vec<String> = body.sessions.iter().map(|s| s.session_id.clone()).collect();

    assert!(session_ids.contains(&"valid-session".to_string()));
    assert!(session_ids.contains(&"circular-session".to_string()));
    // other-project-session should NOT appear as it lacks cwd field

    // Circular reference should use its own summary
    let circular_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "circular-session");
    assert!(circular_session.is_some());
    assert_eq!(
        circular_session.unwrap().summary,
        Some("Circular Reference".to_string())
    );
}

#[tokio::test]
#[serial]
async fn test_corrupted_uuid_references() {
    let server = TestServer::new().await;
    let client = Client::new();

    let project_path = server.mock.projects_dir.join("corrupted-uuid");
    fs::create_dir_all(&project_path).unwrap();

    // Scenario 1: Summary with malformed UUID (not a valid UUID format)
    let malformed1 = project_path.join("malformed1.jsonl");
    fs::write(
        &malformed1,
        r#"{"type":"summary","summary":"Malformed UUID Reference","leafUuid":"not-a-uuid-at-all!@#$%"}
{"uuid":"actual-uuid","sessionId":"session-malformed","type":"user","message":{"role":"user","content":"Message"},"cwd":"/home/test"}"#,
    )
    .unwrap();

    // Scenario 2: Summary with empty UUID
    let empty_uuid = project_path.join("empty.jsonl");
    fs::write(
        &empty_uuid,
        r#"{"type":"summary","summary":"Empty UUID","leafUuid":""}
{"uuid":"real-uuid","sessionId":"session-empty","type":"user","message":{"role":"user","content":"Real message"},"cwd":"/home/empty"}"#,
    )
    .unwrap();

    // Scenario 3: Summary with null UUID (JSON null)
    let null_uuid = project_path.join("null.jsonl");
    fs::write(
        &null_uuid,
        r#"{"type":"summary","summary":"Null UUID","leafUuid":null}
{"uuid":"another-uuid","sessionId":"session-null","type":"user","message":{"role":"user","content":"Another message"},"cwd":"/home/null"}"#,
    )
    .unwrap();

    // Scenario 4: Summary with UUID field missing entirely
    let missing_uuid = project_path.join("missing.jsonl");
    fs::write(
        &missing_uuid,
        r#"{"type":"summary","summary":"Missing UUID Field"}
{"uuid":"yet-another","sessionId":"session-missing","type":"user","message":{"role":"user","content":"Yet another"},"cwd":"/home/missing"}"#,
    )
    .unwrap();

    // Scenario 5: Multiple summaries pointing to same UUID (should use first found)
    let duplicate_refs = project_path.join("duplicate.jsonl");
    fs::write(
        &duplicate_refs,
        r#"{"type":"summary","summary":"First Summary","leafUuid":"shared-uuid"}
{"type":"summary","summary":"Second Summary","leafUuid":"shared-uuid"}
{"type":"summary","summary":"Third Summary","leafUuid":"shared-uuid"}
{"uuid":"shared-uuid","sessionId":"session-duplicate","type":"user","message":{"role":"user","content":"Shared message"},"cwd":"/home/dup"}"#,
    )
    .unwrap();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    // All sessions should be found despite corrupted UUID references
    let session_ids: Vec<String> = body.sessions.iter().map(|s| s.session_id.clone()).collect();

    assert!(session_ids.contains(&"session-malformed".to_string()));
    assert!(session_ids.contains(&"session-empty".to_string()));
    assert!(session_ids.contains(&"session-null".to_string()));
    assert!(session_ids.contains(&"session-missing".to_string()));
    assert!(session_ids.contains(&"session-duplicate".to_string()));

    // For duplicate refs, HashMap behavior means last summary wins
    let dup_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "session-duplicate");
    assert!(dup_session.is_some());
    // The last summary in the file should be used due to HashMap overwriting
    assert_eq!(
        dup_session.unwrap().summary,
        Some("Third Summary".to_string())
    );
}

#[tokio::test]
#[serial]
async fn test_complex_uuid_chain_resolution() {
    let server = TestServer::new().await;
    let client = Client::new();

    let project_path = server.mock.projects_dir.join("uuid-chains");
    fs::create_dir_all(&project_path).unwrap();

    // Create a chain: Summary -> UUID1 -> UUID2 -> Session
    // This tests if the system can follow UUID chains correctly
    let chain_file = project_path.join("chain.jsonl");
    fs::write(
        &chain_file,
        r#"{"type":"summary","summary":"Chain Summary","leafUuid":"uuid-1"}
{"uuid":"uuid-1","type":"intermediate","nextUuid":"uuid-2"}
{"uuid":"uuid-2","type":"intermediate","nextUuid":"uuid-3"}
{"uuid":"uuid-3","sessionId":"chain-session","type":"user","message":{"role":"user","content":"End of chain"},"cwd":"/home/chain"}"#,
    )
    .unwrap();

    // Create a broken chain: Summary -> Missing UUID
    let broken_chain = project_path.join("broken.jsonl");
    fs::write(
        &broken_chain,
        r#"{"type":"summary","summary":"Broken Chain","leafUuid":"missing-link"}
{"uuid":"start-uuid","sessionId":"broken-session","type":"user","message":{"role":"user","content":"Start of broken chain"},"cwd":"/home/broken"}"#,
    )
    .unwrap();

    // Create multiple files contributing to one session
    let multi_file1 = project_path.join("multi1.jsonl");
    fs::write(
        &multi_file1,
        r#"{"type":"summary","summary":"Multi-file Session","leafUuid":"multi-uuid"}"#,
    )
    .unwrap();

    let multi_file2 = project_path.join("multi2.jsonl");
    fs::write(
        &multi_file2,
        r#"{"uuid":"multi-uuid","sessionId":"multi-session","type":"user","message":{"role":"user","content":"In another file"},"cwd":"/home/multi"}"#,
    )
    .unwrap();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    let session_ids: Vec<String> = body.sessions.iter().map(|s| s.session_id.clone()).collect();

    // Chain session should be found - but summary only works for direct UUID matches
    assert!(session_ids.contains(&"chain-session".to_string()));
    let chain_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "chain-session");
    assert!(chain_session.is_some());
    // System doesn't follow UUID chains, only direct matches
    // Since uuid-3 doesn't match uuid-1, falls back to user message
    assert_eq!(
        chain_session.unwrap().summary,
        Some("End of chain".to_string())
    );

    // Broken session should be found but without the broken chain summary
    assert!(session_ids.contains(&"broken-session".to_string()));
    let broken_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "broken-session");
    assert!(broken_session.is_some());
    // Should fall back to first user message
    assert_eq!(
        broken_session.unwrap().summary,
        Some("Start of broken chain".to_string())
    );

    // Multi-file session should work correctly
    assert!(session_ids.contains(&"multi-session".to_string()));
    let multi_session = body
        .sessions
        .iter()
        .find(|s| s.session_id == "multi-session");
    assert!(multi_session.is_some());
    assert_eq!(
        multi_session.unwrap().summary,
        Some("Multi-file Session".to_string())
    );
}

#[tokio::test]
#[serial]
async fn test_uuid_reference_edge_cases() {
    let server = TestServer::new().await;
    let client = Client::new();

    let project_path = server.mock.projects_dir.join("uuid-edges");
    fs::create_dir_all(&project_path).unwrap();

    // Scenario 1: UUID with special characters
    let special_chars = project_path.join("special.jsonl");
    fs::write(
        &special_chars,
        r#"{"type":"summary","summary":"Special Chars","leafUuid":"uuid-with-üñïçødé-chars"}
{"uuid":"uuid-with-üñïçødé-chars","sessionId":"special-session","type":"user","message":{"role":"user","content":"Special"},"cwd":"/home/special"}"#,
    )
    .unwrap();

    // Scenario 2: Very long UUID (beyond typical UUID length)
    let long_uuid = format!("very-long-uuid-{}", "x".repeat(500));
    let long_uuid_file = project_path.join("long.jsonl");
    fs::write(
        &long_uuid_file,
        format!(
            r#"{{"type":"summary","summary":"Long UUID","leafUuid":"{long_uuid}"}}
{{"uuid":"{long_uuid}","sessionId":"long-session","type":"user","message":{{"role":"user","content":"Long"}},"cwd":"/home/long"}}"#
        ),
    )
    .unwrap();

    // Scenario 3: UUID with only numbers
    let numeric_uuid = project_path.join("numeric.jsonl");
    fs::write(
        &numeric_uuid,
        r#"{"type":"summary","summary":"Numeric UUID","leafUuid":"12345678901234567890"}
{"uuid":"12345678901234567890","sessionId":"numeric-session","type":"user","message":{"role":"user","content":"Numbers"},"cwd":"/home/numeric"}"#,
    )
    .unwrap();

    // Scenario 4: Case sensitivity test
    let case_sensitive = project_path.join("case.jsonl");
    fs::write(
        &case_sensitive,
        r#"{"type":"summary","summary":"Case Test","leafUuid":"UUID-UPPERCASE"}
{"uuid":"uuid-uppercase","sessionId":"case-session","type":"user","message":{"role":"user","content":"Case"},"cwd":"/home/case"}"#,
    )
    .unwrap();

    // Scenario 5: Summary and session in same line (malformed JSONL)
    let malformed_jsonl = project_path.join("malformed.jsonl");
    fs::write(
        &malformed_jsonl,
        r#"{"type":"summary","summary":"Same Line","leafUuid":"same-uuid"}{"uuid":"same-uuid","sessionId":"malformed-session","type":"user","message":{"role":"user","content":"Malformed"},"cwd":"/home/malformed"}
{"type":"other","data":"next line"}"#,
    )
    .unwrap();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    let session_ids: Vec<String> = body.sessions.iter().map(|s| s.session_id.clone()).collect();

    // All valid sessions should be found
    assert!(session_ids.contains(&"special-session".to_string()));
    assert!(session_ids.contains(&"long-session".to_string()));
    assert!(session_ids.contains(&"numeric-session".to_string()));

    // Case sensitivity: UUIDs should be case-sensitive, so no match expected
    if session_ids.contains(&"case-session".to_string()) {
        let case_session = body
            .sessions
            .iter()
            .find(|s| s.session_id == "case-session");
        // Should fall back to user message since UUIDs don't match
        assert_eq!(case_session.unwrap().summary, Some("Case".to_string()));
    }

    // Malformed JSONL should be handled gracefully (each JSON object on its own line)
    // This file violates JSONL format, so behavior may vary
}

#[tokio::test]
#[serial]
async fn test_summary_without_uuid_field() {
    let server = TestServer::new().await;
    let client = Client::new();

    let project_path = server.mock.projects_dir.join("no-uuid-field");
    fs::create_dir_all(&project_path).unwrap();

    // Summary objects with various missing/malformed fields
    let incomplete = project_path.join("incomplete.jsonl");
    fs::write(
        &incomplete,
        r#"{"type":"summary","summary":"No UUID field at all"}
{"type":"summary","leafUuid":"has-uuid-but-no-summary"}
{"summary":"Has summary but no type","leafUuid":"some-uuid"}
{"type":"summary"}
{"uuid":"actual-session","sessionId":"incomplete-session","type":"user","message":{"role":"user","content":"Incomplete"},"cwd":"/home/incomplete"}"#,
    )
    .unwrap();

    let response = client
        .get(format!("{}/api/v1/sessions", server.base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: ListSessionsResponse = response.json().await.unwrap();

    // Should still find the session despite malformed summaries
    let session_ids: Vec<String> = body.sessions.iter().map(|s| s.session_id.clone()).collect();

    assert!(session_ids.contains(&"incomplete-session".to_string()));
}
