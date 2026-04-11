//! WebSocket handler and connection manager for real-time events.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use finima_auth::jwt;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// WebSocket message types
// ---------------------------------------------------------------------------

/// Messages sent to connected WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    UploadProgress {
        upload_id: Uuid,
        parsed: usize,
        total: usize,
    },
    CategorizationProgress {
        upload_id: Uuid,
        categorized: usize,
        total: usize,
        flagged: usize,
    },
    CategorizationComplete {
        upload_id: Uuid,
        total: usize,
        flagged: usize,
    },
    RecurringDetected {
        count: usize,
    },
}

// ---------------------------------------------------------------------------
// Connection manager
// ---------------------------------------------------------------------------

/// Manages WebSocket connections grouped by user ID.
///
/// Each user can have multiple concurrent connections (e.g. multiple tabs).
/// Messages are broadcast to all connections for a given user.
#[derive(Clone, Default)]
pub struct WsConnectionManager {
    connections: Arc<RwLock<HashMap<Uuid, Vec<mpsc::Sender<WsMessage>>>>>,
}

impl WsConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new connection for a user. Returns the receiving end.
    pub async fn register(&self, user_id: Uuid) -> mpsc::Receiver<WsMessage> {
        let (tx, rx) = mpsc::channel(64);
        let mut conns = self.connections.write().await;
        conns.entry(user_id).or_default().push(tx);
        rx
    }

    /// Remove closed senders for a user (called on disconnect).
    pub async fn cleanup(&self, user_id: Uuid) {
        let mut conns = self.connections.write().await;
        if let Some(senders) = conns.get_mut(&user_id) {
            senders.retain(|tx| !tx.is_closed());
            if senders.is_empty() {
                conns.remove(&user_id);
            }
        }
    }

    /// Send a message to all connections for a given user.
    pub async fn send_to_user(&self, user_id: Uuid, msg: WsMessage) {
        let conns = self.connections.read().await;
        if let Some(senders) = conns.get(&user_id) {
            for tx in senders {
                let _ = tx.send(msg.clone()).await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
}

/// WS /api/ws?token=<jwt>
///
/// Upgrade to WebSocket. Authenticates via query param JWT token.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<WsQuery>,
) -> impl IntoResponse {
    // Authenticate via query param token
    let jwt_secret = &state.config().auth.jwt_secret;
    let claims = match jwt::decode_token(&params.token, jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id))
        .into_response()
}

/// Handle an established WebSocket connection.
///
/// Registers with the connection manager, forwards outgoing messages,
/// and cleans up on disconnect.
async fn handle_socket(socket: WebSocket, state: AppState, user_id: Uuid) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Register this connection
    let mut rx = state.ws_manager().register(user_id).await;

    // Spawn a task to forward WsMessage -> WebSocket text frames
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Read incoming messages (we don't process client messages, just keep alive)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(_) => {
                    // axum handles pong automatically
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish (disconnect)
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Clean up
    state.ws_manager().cleanup(user_id).await;
    tracing::debug!(user_id = %user_id, "WebSocket connection closed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_message_serializes_as_tagged_json() {
        let msg = WsMessage::CategorizationComplete {
            upload_id: Uuid::nil(),
            total: 42,
            flagged: 3,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"categorization_complete\""));
        assert!(json.contains("\"total\":42"));
        assert!(json.contains("\"flagged\":3"));
    }

    #[test]
    fn ws_message_upload_progress_serializes() {
        let msg = WsMessage::UploadProgress {
            upload_id: Uuid::nil(),
            parsed: 10,
            total: 100,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"upload_progress\""));
    }

    #[tokio::test]
    async fn connection_manager_register_and_send() {
        let manager = WsConnectionManager::new();
        let user_id = Uuid::new_v4();
        let mut rx = manager.register(user_id).await;

        let msg = WsMessage::RecurringDetected { count: 5 };
        manager.send_to_user(user_id, msg).await;

        let received = rx.recv().await.unwrap();
        match received {
            WsMessage::RecurringDetected { count } => assert_eq!(count, 5),
            _ => panic!("Wrong message type"),
        }
    }

    #[tokio::test]
    async fn connection_manager_cleanup_removes_closed() {
        let manager = WsConnectionManager::new();
        let user_id = Uuid::new_v4();
        let rx = manager.register(user_id).await;

        // Drop receiver to close the channel
        drop(rx);

        manager.cleanup(user_id).await;

        let conns = manager.connections.read().await;
        assert!(!conns.contains_key(&user_id));
    }
}
