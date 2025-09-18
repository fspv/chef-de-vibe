use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub working_directory: PathBuf,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earliest_message_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_message_date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Pending,
    Ready,
    Failed,
}

#[derive(Debug)]
pub struct Session {
    pub id: Arc<RwLock<String>>,
    pub working_dir: PathBuf,
    pub process_id: Arc<RwLock<Option<u32>>>,
    pub clients: Arc<RwLock<Vec<WebSocketClient>>>,
    pub write_queue: Arc<Mutex<VecDeque<WriteMessage>>>,
    pub status: Arc<RwLock<SessionStatus>>,
    pub broadcast_tx: broadcast::Sender<BroadcastMessage>,
    // Approval system fields
    pub approval_clients: Arc<RwLock<Vec<ApprovalWebSocketClient>>>,
    pub pending_approvals: Arc<Mutex<HashMap<String, ApprovalRequest>>>,
    pub approval_broadcast_tx: broadcast::Sender<ApprovalMessage>,
}

#[derive(Debug, Clone)]
pub enum BroadcastMessage {
    /// Message from Claude to be sent to all clients
    ClaudeOutput(String),
    /// Message from a client to be sent to all other clients (excludes sender)
    ClientInput {
        content: String,
        sender_client_id: String,
    },
    /// Disconnect signal when Claude process dies
    Disconnect,
}

/// Approval-related data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String, // Our wrapper ID for frontend/backend communication
    pub session_id: String,
    pub claude_request_id: String, // Claude's original request_id for internal use
    pub request: serde_json::Value, // Raw Claude request - pass-through
    pub created_at: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub enum ApprovalMessage {
    /// Approval request from Claude (both new and when sending pending on connection)
    ApprovalRequest(ApprovalRequest),
    /// Approval response from client (raw JSON)
    ApprovalResponse(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalWebSocketClient {
    pub id: String,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub connected_at: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct WebSocketClient {
    pub id: String,
    #[allow(dead_code)] // Used in logging and may be needed for client tracking
    pub ip_address: String,
    #[allow(dead_code)] // Used in logging and may be needed for client tracking
    pub user_agent: Option<String>,
    #[allow(dead_code)] // Used in logging and may be needed for client tracking
    pub connected_at: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct WriteMessage {
    pub content: String,
    #[allow(dead_code)] // Used for message attribution and logging
    pub sender_client_id: String,
    #[allow(dead_code)] // Used for message timing and logging
    pub timestamp: std::time::SystemTime,
}

// API Request/Response types
#[derive(Debug, Serialize, Deserialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub session_id: String,
    pub working_dir: PathBuf,
    pub resume: bool,
    pub bootstrap_messages: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub websocket_url: String,
    pub approval_websocket_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionContentEntry {
    User { message: UserMessage },
    Assistant { message: AssistantMessage },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: String,
    pub content: Vec<AssistantContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantContent {
    Text { text: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetSessionResponse {
    pub session_id: String,
    pub working_directory: PathBuf,
    pub content: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub websocket_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_websocket_url: Option<String>,
}

// Session file format types
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionFileLine {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub cwd: Option<PathBuf>,
    #[serde(flatten)]
    pub other: serde_json::Value,
}

impl Session {
    #[must_use]
    pub fn new(id: String, working_dir: PathBuf) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        let (approval_broadcast_tx, _) = broadcast::channel(1000);
        Self {
            id: Arc::new(RwLock::new(id)),
            working_dir,
            process_id: Arc::new(RwLock::new(None)),
            clients: Arc::new(RwLock::new(Vec::new())),
            write_queue: Arc::new(Mutex::new(VecDeque::new())),
            status: Arc::new(RwLock::new(SessionStatus::Pending)),
            broadcast_tx,
            // Initialize approval system fields
            approval_clients: Arc::new(RwLock::new(Vec::new())),
            pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            approval_broadcast_tx,
        }
    }

    pub async fn add_client(&self, client: WebSocketClient) {
        let mut clients = self.clients.write().await;
        clients.push(client);
    }

    pub async fn remove_client(&self, client_id: &str) {
        let mut clients = self.clients.write().await;
        clients.retain(|c| c.id != client_id);
    }

    #[must_use]
    pub async fn get_clients(&self) -> Vec<WebSocketClient> {
        let clients = self.clients.read().await;
        clients.clone()
    }

    pub async fn enqueue_message(&self, message: WriteMessage) {
        let mut queue = self.write_queue.lock().await;
        queue.push_back(message);
    }

    pub async fn dequeue_message(&self) -> Option<WriteMessage> {
        let mut queue = self.write_queue.lock().await;
        queue.pop_front()
    }

    pub async fn set_status(&self, status: SessionStatus) {
        let mut current_status = self.status.write().await;
        *current_status = status;
    }

    #[must_use]
    pub async fn get_status(&self) -> SessionStatus {
        let status = self.status.read().await;
        *status
    }

    #[must_use]
    pub async fn is_active(&self) -> bool {
        let process_id = self.process_id.read().await;
        process_id.is_some()
    }

    pub async fn set_process_id(&self, pid: Option<u32>) {
        let mut process_id = self.process_id.write().await;
        *process_id = pid;
    }

    #[must_use]
    pub async fn get_process_id(&self) -> Option<u32> {
        let process_id = self.process_id.read().await;
        *process_id
    }

    #[must_use]
    pub async fn get_id(&self) -> String {
        let id = self.id.read().await;
        id.clone()
    }

    pub async fn set_id(&self, new_id: String) {
        let mut id = self.id.write().await;
        *id = new_id;
    }

    /// Broadcasts a message to all or filtered clients
    pub fn broadcast_message(
        &self,
        message: BroadcastMessage,
    ) -> Result<usize, broadcast::error::SendError<BroadcastMessage>> {
        self.broadcast_tx.send(message)
    }

    /// Get a receiver for broadcast messages
    #[must_use]
    pub fn subscribe_to_broadcasts(&self) -> broadcast::Receiver<BroadcastMessage> {
        self.broadcast_tx.subscribe()
    }

    // Approval system methods
    pub async fn add_approval_client(&self, client: ApprovalWebSocketClient) {
        let mut clients = self.approval_clients.write().await;
        clients.push(client);
    }

    pub async fn remove_approval_client(&self, client_id: &str) {
        let mut clients = self.approval_clients.write().await;
        clients.retain(|c| c.id != client_id);
    }

    #[must_use]
    pub async fn get_approval_clients(&self) -> Vec<ApprovalWebSocketClient> {
        let clients = self.approval_clients.read().await;
        clients.clone()
    }

    pub async fn add_pending_approval(&self, request: ApprovalRequest) {
        let mut pending = self.pending_approvals.lock().await;
        pending.insert(request.id.clone(), request); // Updated to use id instead of request_id
    }

    pub async fn remove_pending_approval(&self, request_id: &str) -> Option<ApprovalRequest> {
        let mut pending = self.pending_approvals.lock().await;
        pending.remove(request_id)
    }

    #[must_use]
    pub async fn get_pending_approvals(&self) -> Vec<ApprovalRequest> {
        let pending = self.pending_approvals.lock().await;
        pending.values().cloned().collect()
    }

    /// Broadcasts an approval message to all approval clients
    pub fn broadcast_approval_message(
        &self,
        message: ApprovalMessage,
    ) -> Result<usize, broadcast::error::SendError<ApprovalMessage>> {
        self.approval_broadcast_tx.send(message)
    }

    /// Get a receiver for approval broadcast messages
    #[must_use]
    pub fn subscribe_to_approval_broadcasts(&self) -> broadcast::Receiver<ApprovalMessage> {
        self.approval_broadcast_tx.subscribe()
    }
}

impl WebSocketClient {
    #[must_use]
    pub fn new(id: String, ip_address: String, user_agent: Option<String>) -> Self {
        Self {
            id,
            ip_address,
            user_agent,
            connected_at: std::time::SystemTime::now(),
        }
    }
}

impl ApprovalWebSocketClient {
    #[must_use]
    pub fn new(id: String, ip_address: String, user_agent: Option<String>) -> Self {
        Self {
            id,
            ip_address,
            user_agent,
            connected_at: std::time::SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_client_management() {
        let session = Session::new("test-session".to_string(), PathBuf::from("/tmp"));

        let client1 = WebSocketClient::new(
            "client1".to_string(),
            "127.0.0.1".to_string(),
            Some("Mozilla/5.0".to_string()),
        );

        let client2 = WebSocketClient::new("client2".to_string(), "127.0.0.2".to_string(), None);

        session.add_client(client1).await;
        session.add_client(client2).await;

        let clients = session.get_clients().await;
        assert_eq!(clients.len(), 2);

        session.remove_client("client1").await;
        let clients = session.get_clients().await;
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].id, "client2");
    }

    #[tokio::test]
    async fn test_write_queue() {
        let session = Session::new("test-session".to_string(), PathBuf::from("/tmp"));

        let msg1 = WriteMessage {
            content: "Hello".to_string(),
            sender_client_id: "client1".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        let msg2 = WriteMessage {
            content: "World".to_string(),
            sender_client_id: "client2".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        session.enqueue_message(msg1.clone()).await;
        session.enqueue_message(msg2.clone()).await;

        let dequeued1 = session.dequeue_message().await.unwrap();
        assert_eq!(dequeued1.content, "Hello");

        let dequeued2 = session.dequeue_message().await.unwrap();
        assert_eq!(dequeued2.content, "World");

        assert!(session.dequeue_message().await.is_none());
    }

    #[tokio::test]
    async fn test_session_status() {
        let session = Session::new("test-session".to_string(), PathBuf::from("/tmp"));

        assert_eq!(session.get_status().await, SessionStatus::Pending);

        session.set_status(SessionStatus::Ready).await;
        assert_eq!(session.get_status().await, SessionStatus::Ready);

        session.set_status(SessionStatus::Failed).await;
        assert_eq!(session.get_status().await, SessionStatus::Failed);
    }
}
