// This file serves as a common helper module with shared TestServer implementation
// and utility functions for other test files. Individual test functions have been
// moved to specific test files based on their functionality.

mod helpers;

use chef_de_vibe::{
    api::handlers::AppState,
    config::Config,
    models::{
        CreateSessionRequest, CreateSessionResponse,
    },
    session_manager::SessionManager,
};
use helpers::mock_claude::MockClaude;
use reqwest::Client;
use serial_test::serial;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

fn generate_unique_session_id(test_name: &str) -> String {
    format!("{}-{}-{}", 
        test_name, 
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    )
}

struct TestServer {
    pub base_url: String,
    pub ws_url: String,
    pub mock: MockClaude,
    server_handle: tokio::task::JoinHandle<()>,
    session_manager: Arc<SessionManager>,
}

impl TestServer {
    async fn new() -> Self {
        let mock = MockClaude::new();
        mock.setup_env_vars();
        Self::new_internal(mock).await
    }

    async fn new_with_approval_binary() -> Self {
        let mock = MockClaude::new();
        mock.setup_env_vars(); // Setup all environment variables first
        let approval_binary = mock.create_approval_binary();
        std::env::set_var("CLAUDE_BINARY_PATH", &approval_binary); // Override with approval binary
        Self::new_internal(mock).await
    }

    async fn new_with_config() -> Self {
        // Create a mock but don't call setup_env_vars - assume env vars are already set
        let mock = MockClaude::new();
        Self::new_internal(mock).await
    }

    async fn new_internal(mock: MockClaude) -> Self {

        let config = Config::from_env().expect("Failed to load config");
        let session_manager = Arc::new(SessionManager::new(config.clone()));

        let state = AppState {
            session_manager: session_manager.clone(),
            config: Arc::new(config),
        };

        // Build router
        let app = axum::Router::new()
            .route(
                "/api/v1/sessions",
                axum::routing::get(chef_de_vibe::api::handlers::list_sessions),
            )
            .route(
                "/api/v1/sessions",
                axum::routing::post(chef_de_vibe::api::handlers::create_session),
            )
            .route(
                "/api/v1/sessions/:id",
                axum::routing::get(chef_de_vibe::api::handlers::get_session),
            )
            .route(
                "/api/v1/sessions/:id/claude_ws",
                axum::routing::get(chef_de_vibe::api::websocket::websocket_handler),
            )
            .route(
                "/api/v1/sessions/:id/claude_approvals_ws",
                axum::routing::get(chef_de_vibe::api::websocket::approval_websocket_handler),
            )
            .with_state(state);

        // Find a free port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();

        let base_url = format!("http://127.0.0.1:{}", port);
        let ws_url = format!("ws://127.0.0.1:{}", port);

        // Spawn server
        let server_handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give server time to start - increase for better test isolation
        tokio::time::sleep(Duration::from_millis(500)).await;

        TestServer {
            base_url,
            ws_url,
            mock,
            server_handle,
            session_manager,
        }
    }
}


impl Drop for TestServer {
    fn drop(&mut self) {
        // First abort the server to stop accepting new connections
        self.server_handle.abort();
        
        // Use thread-based cleanup to avoid runtime nesting issues
        let session_manager = self.session_manager.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                // Give more time for ongoing operations to complete
                tokio::time::sleep(Duration::from_millis(200)).await;
                
                // Shutdown all sessions
                session_manager.shutdown().await;
                
                // Additional time for WebSocket connections and processes to clean up
                tokio::time::sleep(Duration::from_millis(300)).await;
            });
        }).join().ok();
    }
}
























