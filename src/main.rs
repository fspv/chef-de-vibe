mod api;
mod claude_process;
mod config;
mod discovery;
mod error;
mod models;
mod session_manager;

use crate::api::handlers::{create_session, get_session, list_sessions, AppState};
use crate::api::static_files::{serve_index, serve_static};
use crate::api::websocket::{approval_websocket_handler, websocket_handler};
use crate::config::Config;
use crate::session_manager::SessionManager;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "chef_de_vibe=debug,info".into()),
        )
        .init();

    // Load configuration
    let config = Config::from_env()?;
    info!("Starting Chef de Vibe Service");
    info!(claude_binary = %config.claude_binary_path.display(), "Claude binary path");
    info!(projects_dir = %config.claude_projects_dir.display(), "Projects directory");
    info!(listen_address = %config.http_listen_address, "Listen address");

    // Create session manager
    let session_manager = Arc::new(SessionManager::new(config.clone()));

    // Create application state
    let state = AppState {
        session_manager: session_manager.clone(),
        config: Arc::new(config.clone()),
    };

    // Build router
    let app = Router::new()
        // API routes
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions", post(create_session))
        .route("/api/v1/sessions/:id", get(get_session))
        .route("/api/v1/sessions/:id/claude_ws", get(websocket_handler))
        .route("/api/v1/sessions/:id/claude_approvals_ws", get(approval_websocket_handler))
        // Static file routes
        .route("/", get(serve_index))
        .route("/*path", get(serve_static))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.http_listen_address).await?;
    info!(address = %config.http_listen_address, "Server listening");
    info!("To change the listen address, set the HTTP_LISTEN_ADDRESS environment variable (e.g., HTTP_LISTEN_ADDRESS=0.0.0.0:8080)");

    // Setup graceful shutdown with double Ctrl+C handling
    let shutdown_signal = async {
        // First Ctrl+C - graceful shutdown
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for shutdown signal");
        info!("Received shutdown signal (Ctrl+C), initiating graceful shutdown...");
        info!("Press Ctrl+C again to force immediate shutdown");
        
        // Start a task to listen for second Ctrl+C
        tokio::spawn(async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to listen for second shutdown signal");
            error!("Received second shutdown signal, forcing immediate exit!");
            std::process::exit(1);
        });
    };

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    // Shutdown session manager
    info!("Shutting down session manager...");
    session_manager.shutdown().await;
    info!("Graceful shutdown completed successfully");

    Ok(())
}
