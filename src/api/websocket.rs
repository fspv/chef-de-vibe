use crate::api::handlers::AppState;
use crate::models::{
    ApprovalMessage, ApprovalWebSocketClient, BroadcastMessage, WebSocketClient, WriteMessage,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

#[instrument(skip(ws, state), fields(session_id = %session_id))]
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    info!(session_id = %session_id, "WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_websocket(socket, session_id, state))
}

#[instrument(skip(_session), fields(session_id = %session_id, client_id))]
fn setup_client_connection(
    session_id: &str,
    _session: &Arc<crate::models::Session>,
) -> (String, WebSocketClient) {
    // Generate unique client ID
    let client_id = Uuid::new_v4().to_string();
    tracing::Span::current().record("client_id", &client_id);

    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "Generating new WebSocket client"
    );

    // Create client
    let client = WebSocketClient::new(
        client_id.clone(),
        "127.0.0.1".to_string(), // In real implementation, get from socket
        Some("WebSocket Client".to_string()),
    );

    info!(
        session_id = %session_id,
        client_id = %client_id,
        ip_address = "127.0.0.1",
        "WebSocket client connected to session"
    );

    (client_id, client)
}

#[instrument(skip(sender, rx), fields(client_id = %client_id))]
fn spawn_outgoing_message_handler(
    mut sender: futures::stream::SplitSink<WebSocket, Message>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Message>,
    client_id: String,
) -> tokio::task::JoinHandle<()> {
    debug!(client_id = %client_id, "Spawning outgoing message handler");

    tokio::spawn(async move {
        let mut messages_sent = 0;
        while let Some(msg) = rx.recv().await {
            match sender.send(msg).await {
                Ok(()) => {
                    messages_sent += 1;
                    debug!(
                        client_id = %client_id,
                        messages_sent = messages_sent,
                        "Sent message to WebSocket client"
                    );
                }
                Err(e) => {
                    error!(
                        client_id = %client_id,
                        error = %e,
                        messages_sent = messages_sent,
                        "Failed to send message to WebSocket client, stopping handler"
                    );
                    break;
                }
            }
        }
        info!(
            client_id = %client_id,
            total_messages_sent = messages_sent,
            "Outgoing message handler stopped"
        );
    })
}

fn spawn_broadcast_handler(
    session: Arc<crate::models::Session>,
    tx: tokio::sync::mpsc::UnboundedSender<Message>,
    client_id: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut broadcast_rx = session.subscribe_to_broadcasts();
        debug!(
            client_id = %client_id,
            "Started broadcast receiver for WebSocket client"
        );

        while let Ok(broadcast_msg) = broadcast_rx.recv().await {
            let should_send_message = match &broadcast_msg {
                BroadcastMessage::ClaudeOutput(content) => {
                    debug!(
                        client_id = %client_id,
                        content_length = content.len(),
                        "Received Claude output to broadcast"
                    );
                    Some(content.clone())
                }
                BroadcastMessage::ClientInput {
                    content,
                    sender_client_id,
                } => {
                    // Only send to clients that are NOT the sender
                    if sender_client_id != &client_id {
                        debug!(
                            client_id = %client_id,
                            sender_client_id = %sender_client_id,
                            content_length = content.len(),
                            "Received client input to broadcast (not from this client)"
                        );
                        Some(content.clone())
                    } else {
                        debug!(
                            client_id = %client_id,
                            "Skipping client input broadcast (from this client)"
                        );
                        None
                    }
                }
                BroadcastMessage::Disconnect => {
                    info!(
                        client_id = %client_id,
                        "Received disconnect signal, closing WebSocket"
                    );
                    // Send close message and break
                    let _ = tx.send(Message::Close(None));
                    break;
                }
            };

            if let Some(message_content) = should_send_message {
                // Check if this client is still connected to the session
                let clients = session.get_clients().await;
                if clients.iter().any(|c| c.id == client_id) {
                    if let Err(e) = tx.send(Message::Text(message_content)) {
                        warn!(
                            client_id = %client_id,
                            error = %e,
                            "Failed to send message to WebSocket client, stopping broadcast handler"
                        );
                        break;
                    }
                } else {
                    info!(
                        client_id = %client_id,
                        "Client no longer connected to session, stopping broadcast handler"
                    );
                    break;
                }
            }
        }

        debug!(
            client_id = %client_id,
            "Broadcast handler finished"
        );
    })
}

#[instrument(skip(session, state), fields(client_id = %client_id, session_id = %session_id, message_len = text.len()))]
async fn handle_text_message(
    text: String,
    client_id: &str,
    session_id: &str,
    session: Arc<crate::models::Session>,
    state: AppState,
) {
    debug!(
        client_id = %client_id,
        session_id = %session_id,
        message_length = text.len(),
        "Processing WebSocket text message"
    );

    // Validate JSON
    if let Err(e) = serde_json::from_str::<serde_json::Value>(&text) {
        error!(
            client_id = %client_id,
            session_id = %session_id,
            message_content = %text,
            error = %e,
            "Received invalid JSON from WebSocket client"
        );
        return;
    }

    debug!(
        client_id = %client_id,
        session_id = %session_id,
        "JSON validation passed"
    );

    // Create write message
    let write_msg = WriteMessage {
        content: text.clone(),
        sender_client_id: client_id.to_string(),
        timestamp: std::time::SystemTime::now(),
    };

    debug!(
        client_id = %client_id,
        session_id = %session_id,
        "Created WriteMessage for Claude"
    );

    // Enqueue message for Claude
    if let Err(e) = state
        .session_manager
        .enqueue_message(session_id, write_msg)
        .await
    {
        error!(
            client_id = %client_id,
            session_id = %session_id,
            error = %e,
            "Failed to enqueue message for Claude processing"
        );
        return;
    }

    info!(
        client_id = %client_id,
        session_id = %session_id,
        "Message successfully enqueued for Claude"
    );

    // Broadcast to all OTHER clients (not the sender) using session broadcast
    let clients = session.get_clients().await;
    let other_clients_count = clients.iter().filter(|c| c.id != client_id).count();

    debug!(
        client_id = %client_id,
        session_id = %session_id,
        total_clients = clients.len(),
        other_clients = other_clients_count,
        "Broadcasting client input to other clients"
    );

    if other_clients_count > 0 {
        let broadcast_msg = BroadcastMessage::ClientInput {
            content: text.clone(),
            sender_client_id: client_id.to_string(),
        };

        if let Err(e) = session.broadcast_message(broadcast_msg) {
            warn!(
                client_id = %client_id,
                session_id = %session_id,
                error = %e,
                "Failed to broadcast client input to other clients"
            );
        } else {
            debug!(
                client_id = %client_id,
                session_id = %session_id,
                "Successfully broadcast client input to other clients"
            );
        }
    } else {
        debug!(
            client_id = %client_id,
            session_id = %session_id,
            "No other clients to broadcast to"
        );
    }
}

#[allow(clippy::needless_pass_by_value)]
#[instrument(skip(session, send_task, broadcast_task), fields(client_id = %client_id, session_id = %session_id))]
fn cleanup_client_connection(
    session: Arc<crate::models::Session>,
    client_id: &str,
    session_id: &str,
    send_task: tokio::task::JoinHandle<()>,
    broadcast_task: tokio::task::JoinHandle<()>,
) {
    info!(
        client_id = %client_id,
        session_id = %session_id,
        "Starting WebSocket client cleanup"
    );

    // Client disconnected, clean up
    let session_cleanup = session.clone();
    let client_id_cleanup = client_id.to_string();
    let session_id_cleanup = session_id.to_string();
    tokio::spawn(async move {
        debug!(
            client_id = %client_id_cleanup,
            session_id = %session_id_cleanup,
            "Removing client from session"
        );
        session_cleanup.remove_client(&client_id_cleanup).await;
        debug!(
            client_id = %client_id_cleanup,
            session_id = %session_id_cleanup,
            "Client removed from session"
        );
    });

    debug!(
        client_id = %client_id,
        session_id = %session_id,
        "Aborting background tasks"
    );

    send_task.abort();
    broadcast_task.abort();

    info!(
        client_id = %client_id,
        session_id = %session_id,
        "WebSocket client disconnected and cleanup completed"
    );
}

#[allow(clippy::too_many_lines)]
#[instrument(skip(socket, state), fields(session_id = %session_id, client_id))]
async fn handle_websocket(socket: WebSocket, session_id: String, state: AppState) {
    info!(session_id = %session_id, "Starting WebSocket connection handling");

    // Get session
    let Some(session) = state.session_manager.get_session(&session_id) else {
        error!(session_id = %session_id, "WebSocket connection rejected: session not found");
        // Close the WebSocket connection immediately
        let _ = socket.close().await;
        return;
    };

    debug!(session_id = %session_id, "Session found, checking if active");

    // Check if session is active
    if !session.is_active().await {
        error!(session_id = %session_id, "WebSocket connection rejected: session not active");
        // Close the WebSocket connection immediately
        let _ = socket.close().await;
        return;
    };

    debug!(session_id = %session_id, "Session is active, proceeding with connection");

    // Setup client connection
    let (client_id, client) = setup_client_connection(&session_id, &session);
    tracing::Span::current().record("client_id", &client_id);

    session.add_client(client).await;
    info!(
        session_id = %session_id,
        client_id = %client_id,
        "Client added to session"
    );

    // Split socket into sender and receiver
    let (sender, mut receiver) = socket.split();
    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "WebSocket split into sender and receiver"
    );

    // Create channel for outgoing messages
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "Communication channels created"
    );

    // Spawn background tasks
    let send_task = spawn_outgoing_message_handler(sender, rx, client_id.clone());
    let broadcast_task = spawn_broadcast_handler(session.clone(), tx.clone(), client_id.clone());

    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "Background tasks spawned"
    );

    // Handle incoming messages from this WebSocket client
    let client_id_recv = client_id.clone();
    info!(
        session_id = %session_id,
        client_id = %client_id_recv,
        "Starting message processing loop"
    );

    let mut messages_processed = 0;
    while let Some(msg) = receiver.next().await {
        messages_processed += 1;
        match msg {
            Ok(Message::Text(text)) => {
                debug!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    message_number = messages_processed,
                    "Received text message"
                );
                handle_text_message(
                    text,
                    &client_id_recv,
                    &session_id,
                    session.clone(),
                    state.clone(),
                )
                .await;
            }
            Ok(Message::Close(close_frame)) => {
                info!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    close_code = close_frame.as_ref().map(|f| f.code.into()).unwrap_or(0),
                    "WebSocket client sent close message"
                );
                break;
            }
            Ok(Message::Ping(data)) => {
                debug!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    ping_data_len = data.len(),
                    "Received ping, sending pong"
                );
                if tx.send(Message::Pong(data)).is_err() {
                    warn!(
                        session_id = %session_id,
                        client_id = %client_id_recv,
                        "Failed to send pong response, breaking connection"
                    );
                    break;
                }
            }
            Ok(Message::Pong(data)) => {
                debug!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    pong_data_len = data.len(),
                    "Received pong message"
                );
            }
            Ok(Message::Binary(data)) => {
                warn!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    binary_data_len = data.len(),
                    "Received unexpected binary message from WebSocket client"
                );
            }
            Err(e) => {
                error!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    error = %e,
                    messages_processed = messages_processed,
                    "WebSocket error, breaking connection"
                );
                break;
            }
        }
    }

    info!(
        session_id = %session_id,
        client_id = %client_id_recv,
        total_messages_processed = messages_processed,
        "Message processing loop ended"
    );

    cleanup_client_connection(session, &client_id, &session_id, send_task, broadcast_task);
}

/// Approval WebSocket handler
#[instrument(skip(ws, state), fields(session_id = %session_id))]
pub async fn approval_websocket_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AppState>,
) -> Response {
    info!(session_id = %session_id, "Approval WebSocket upgrade request");
    ws.on_upgrade(move |socket| handle_approval_websocket(socket, session_id, state))
}

#[instrument(skip(_session), fields(session_id = %session_id, client_id))]
fn setup_approval_client_connection(
    session_id: &str,
    _session: &Arc<crate::models::Session>,
) -> (String, ApprovalWebSocketClient) {
    // Generate unique client ID
    let client_id = Uuid::new_v4().to_string();
    tracing::Span::current().record("client_id", &client_id);

    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "Generating new approval WebSocket client"
    );

    // Create approval client
    let client = ApprovalWebSocketClient::new(
        client_id.clone(),
        "127.0.0.1".to_string(), // In real implementation, get from socket
        Some("Approval WebSocket Client".to_string()),
    );

    info!(
        session_id = %session_id,
        client_id = %client_id,
        ip_address = "127.0.0.1",
        "Approval WebSocket client connected to session"
    );

    (client_id, client)
}

#[instrument(skip(sender, rx), fields(client_id = %client_id))]
fn spawn_approval_outgoing_message_handler(
    mut sender: futures::stream::SplitSink<WebSocket, Message>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Message>,
    client_id: String,
) -> tokio::task::JoinHandle<()> {
    debug!(client_id = %client_id, "Spawning approval outgoing message handler");

    tokio::spawn(async move {
        let mut messages_sent = 0;
        while let Some(msg) = rx.recv().await {
            match sender.send(msg).await {
                Ok(()) => {
                    messages_sent += 1;
                    debug!(
                        client_id = %client_id,
                        messages_sent = messages_sent,
                        "Sent approval message to WebSocket client"
                    );
                }
                Err(e) => {
                    error!(
                        client_id = %client_id,
                        error = %e,
                        messages_sent = messages_sent,
                        "Failed to send approval message to WebSocket client, stopping handler"
                    );
                    break;
                }
            }
        }
        info!(
            client_id = %client_id,
            total_messages_sent = messages_sent,
            "Approval outgoing message handler stopped"
        );
    })
}

fn spawn_approval_broadcast_handler(
    session: Arc<crate::models::Session>,
    tx: tokio::sync::mpsc::UnboundedSender<Message>,
    client_id: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut broadcast_rx = session.subscribe_to_approval_broadcasts();
        debug!(
            client_id = %client_id,
            "Started approval broadcast receiver for WebSocket client"
        );

        while let Ok(broadcast_msg) = broadcast_rx.recv().await {
            let message_json = match &broadcast_msg {
                ApprovalMessage::ApprovalRequest(request) => {
                    debug!(
                        client_id = %client_id,
                        approval_id = %request.id,
                        "Received approval request to broadcast with new simplified format"
                    );
                    match serde_json::to_string(&serde_json::json!({
                        "id": request.id,
                        "request": request.request,  // Pass through raw Claude request
                        "created_at": request.created_at.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default().as_secs()
                    })) {
                        Ok(json) => json,
                        Err(e) => {
                            error!(
                                client_id = %client_id,
                                error = %e,
                                "Failed to serialize approval request"
                            );
                            continue;
                        }
                    }
                }
                ApprovalMessage::ApprovalResponse { .. } => {
                    // Approval responses are not broadcast to clients, only processed internally
                    continue;
                }
            };

            // Check if this client is still connected to the session
            let clients = session.get_approval_clients().await;
            if clients.iter().any(|c| c.id == client_id) {
                if let Err(e) = tx.send(Message::Text(message_json)) {
                    warn!(
                        client_id = %client_id,
                        error = %e,
                        "Failed to send approval message to WebSocket client, stopping broadcast handler"
                    );
                    break;
                }
            } else {
                info!(
                    client_id = %client_id,
                    "Approval client no longer connected to session, stopping broadcast handler"
                );
                break;
            }
        }

        debug!(
            client_id = %client_id,
            "Approval broadcast handler finished"
        );
    })
}

#[instrument(skip(session, _state), fields(client_id = %client_id, session_id = %session_id))]
async fn handle_approval_text_message(
    text: String,
    client_id: &str,
    session_id: &str,
    session: Arc<crate::models::Session>,
    _state: AppState,
) {
    debug!(
        client_id = %client_id,
        session_id = %session_id,
        message_length = text.len(),
        "Processing approval WebSocket text message"
    );

    // Parse the approval response message
    let parsed: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(e) => {
            error!(
                client_id = %client_id,
                session_id = %session_id,
                message_content = %text,
                error = %e,
                "Received invalid JSON from approval WebSocket client"
            );
            return;
        }
    };

    // Check if this has the expected new format: {id: "...", response: {...}}
    if parsed.get("id").is_some() && parsed.get("response").is_some() {
        let approval_response = ApprovalMessage::ApprovalResponse(parsed.clone());

        debug!(
            client_id = %client_id,
            session_id = %session_id,
            wrapper_id = ?parsed.get("id"),
            "Received approval response from client with new format"
        );

        // Broadcast the approval response internally (this will be handled by the session manager)
        if let Err(e) = session.broadcast_approval_message(approval_response) {
            error!(
                client_id = %client_id,
                session_id = %session_id,
                error = %e,
                "Failed to broadcast approval response"
            );
        } else {
            info!(
                client_id = %client_id,
                session_id = %session_id,
                wrapper_id = ?parsed.get("id"),
                "Successfully broadcast approval response"
            );
        }
    } else {
        warn!(
            client_id = %client_id,
            session_id = %session_id,
            has_id = parsed.get("id").is_some(),
            has_response = parsed.get("response").is_some(),
            "Received invalid message format from approval WebSocket client (expected: {{id: '...', response: {{...}}}})"
        );
    }
}

#[instrument(skip(socket, state), fields(session_id = %session_id, client_id))]
async fn handle_approval_websocket(socket: WebSocket, session_id: String, state: AppState) {
    info!(session_id = %session_id, "Starting approval WebSocket connection handling");

    // Get session
    let Some(session) = state.session_manager.get_session(&session_id) else {
        error!(session_id = %session_id, "Approval WebSocket connection rejected: session not found");
        let _ = socket.close().await;
        return;
    };

    debug!(session_id = %session_id, "Session found, checking if active");

    // Check if session is active
    if !session.is_active().await {
        error!(session_id = %session_id, "Approval WebSocket connection rejected: session not active");
        let _ = socket.close().await;
        return;
    };

    debug!(session_id = %session_id, "Session is active, proceeding with approval connection");

    // Setup approval client connection
    let (client_id, client) = setup_approval_client_connection(&session_id, &session);
    tracing::Span::current().record("client_id", &client_id);

    session.add_approval_client(client).await;
    info!(
        session_id = %session_id,
        client_id = %client_id,
        "Approval client added to session"
    );

    // Split socket into sender and receiver
    let (sender, mut receiver) = socket.split();
    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "Approval WebSocket split into sender and receiver"
    );

    // Create channel for outgoing messages
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "Approval communication channels created"
    );

    // Spawn background tasks first
    let send_task = spawn_approval_outgoing_message_handler(sender, rx, client_id.clone());
    let broadcast_task =
        spawn_approval_broadcast_handler(session.clone(), tx.clone(), client_id.clone());

    // Send pending approvals immediately upon connection as individual messages
    let pending_approvals = session.get_pending_approvals().await;
    if !pending_approvals.is_empty() {
        info!(
            session_id = %session_id,
            client_id = %client_id,
            pending_count = pending_approvals.len(),
            "Sending pending approvals to newly connected client as individual messages"
        );

        for approval_request in pending_approvals {
            let approval_message = match serde_json::to_string(&serde_json::json!({
                "id": approval_request.id,
                "request": approval_request.request,
                "created_at": approval_request.created_at.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default().as_secs()
            })) {
                Ok(json) => json,
                Err(e) => {
                    error!(
                        session_id = %session_id,
                        client_id = %client_id,
                        approval_id = %approval_request.id,
                        error = %e,
                        "Failed to serialize pending approval request"
                    );
                    continue;
                }
            };

            if let Err(e) = tx.send(Message::Text(approval_message)) {
                warn!(
                    session_id = %session_id,
                    client_id = %client_id,
                    approval_id = %approval_request.id,
                    error = %e,
                    "Failed to send pending approval request to client"
                );
                break; // Stop sending if there's a connection issue
            }
        }
    }

    debug!(
        session_id = %session_id,
        client_id = %client_id,
        "Approval background tasks spawned"
    );

    // Handle incoming messages from this approval WebSocket client
    let client_id_recv = client_id.clone();
    info!(
        session_id = %session_id,
        client_id = %client_id_recv,
        "Starting approval message processing loop"
    );

    let mut messages_processed = 0;
    while let Some(msg) = receiver.next().await {
        messages_processed += 1;
        match msg {
            Ok(Message::Text(text)) => {
                debug!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    message_number = messages_processed,
                    "Received approval text message"
                );
                handle_approval_text_message(
                    text,
                    &client_id_recv,
                    &session_id,
                    session.clone(),
                    state.clone(),
                )
                .await;
            }
            Ok(Message::Close(close_frame)) => {
                info!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    close_code = close_frame.as_ref().map(|f| f.code.into()).unwrap_or(0),
                    "Approval WebSocket client sent close message"
                );
                break;
            }
            Ok(Message::Ping(data)) => {
                debug!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    ping_data_len = data.len(),
                    "Received ping from approval client, sending pong"
                );
                if tx.send(Message::Pong(data)).is_err() {
                    warn!(
                        session_id = %session_id,
                        client_id = %client_id_recv,
                        "Failed to send pong response, breaking approval connection"
                    );
                    break;
                }
            }
            Ok(Message::Pong(data)) => {
                debug!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    pong_data_len = data.len(),
                    "Received pong message from approval client"
                );
            }
            Ok(Message::Binary(data)) => {
                warn!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    binary_data_len = data.len(),
                    "Received unexpected binary message from approval WebSocket client"
                );
            }
            Err(e) => {
                error!(
                    session_id = %session_id,
                    client_id = %client_id_recv,
                    error = %e,
                    messages_processed = messages_processed,
                    "Approval WebSocket error, breaking connection"
                );
                break;
            }
        }
    }

    info!(
        session_id = %session_id,
        client_id = %client_id_recv,
        total_messages_processed = messages_processed,
        "Approval message processing loop ended"
    );

    // Client disconnected, clean up
    let session_cleanup = session.clone();
    let client_id_cleanup = client_id.to_string();
    let session_id_cleanup = session_id.to_string();
    tokio::spawn(async move {
        debug!(
            client_id = %client_id_cleanup,
            session_id = %session_id_cleanup,
            "Removing approval client from session"
        );
        session_cleanup
            .remove_approval_client(&client_id_cleanup)
            .await;
        debug!(
            client_id = %client_id_cleanup,
            session_id = %session_id_cleanup,
            "Approval client removed from session"
        );
    });

    debug!(
        client_id = %client_id,
        session_id = %session_id,
        "Aborting approval background tasks"
    );

    send_task.abort();
    broadcast_task.abort();

    info!(
        client_id = %client_id,
        session_id = %session_id,
        "Approval WebSocket client disconnected and cleanup completed"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::session_manager::SessionManager;
    use std::fs;
    use tempfile::TempDir;

    #[allow(dead_code)] // Test helper function
    fn create_test_state(temp_dir: &TempDir) -> AppState {
        // Create mock Claude binary
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

        let config = Config {
            claude_binary_path: claude_path,
            http_listen_address: "127.0.0.1:8080".to_string(),
            claude_projects_dir: projects_dir,
            shutdown_timeout: std::time::Duration::from_secs(1),
        };

        AppState {
            session_manager: Arc::new(SessionManager::new(config.clone())),
            config: Arc::new(config),
        }
    }

    #[tokio::test]
    async fn test_broadcast_message_creation() {
        let claude_msg = BroadcastMessage::ClaudeOutput("test output".to_string());
        let client_msg = BroadcastMessage::ClientInput {
            content: "test input".to_string(),
            sender_client_id: "client123".to_string(),
        };
        let disconnect_msg = BroadcastMessage::Disconnect;

        // Verify enum variants can be created
        match claude_msg {
            BroadcastMessage::ClaudeOutput(content) => assert_eq!(content, "test output"),
            _ => panic!("Expected ClaudeOutput"),
        }

        match client_msg {
            BroadcastMessage::ClientInput {
                content,
                sender_client_id,
            } => {
                assert_eq!(content, "test input");
                assert_eq!(sender_client_id, "client123");
            }
            _ => panic!("Expected ClientInput"),
        }

        matches!(disconnect_msg, BroadcastMessage::Disconnect);
    }

    #[tokio::test]
    async fn test_write_message_creation() {
        let message = WriteMessage {
            content: "test content".to_string(),
            sender_client_id: "client123".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        assert_eq!(message.content, "test content");
        assert_eq!(message.sender_client_id, "client123");
    }

    #[tokio::test]
    async fn test_websocket_client_creation() {
        let client = WebSocketClient::new(
            "test-client".to_string(),
            "192.168.1.1".to_string(),
            Some("Test Agent".to_string()),
        );

        assert_eq!(client.id, "test-client");
        assert_eq!(client.ip_address, "192.168.1.1");
        assert_eq!(client.user_agent, Some("Test Agent".to_string()));
    }
}
