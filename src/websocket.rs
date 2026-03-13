//! WebSocket support for real-time features
//! Live notifications, deal updates, dashboard data

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::{error::AppError, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    Notification {
        id: String,
        title: String,
        message: String,
        level: String, // info, success, warning, error
        timestamp: chrono::DateTime<chrono::Utc>,
        user_id: Option<String>,
    },
    LeadUpdate {
        lead_id: Uuid,
        action: String, // created, updated, assigned, status_changed
        data: serde_json::Value,
        user_id: Option<String>,
    },
    DealUpdate {
        deal_id: Uuid,
        action: String,
        data: serde_json::Value,
        user_id: Option<String>,
    },
    InteractionUpdate {
        interaction_id: Uuid,
        action: String,
        data: serde_json::Value,
        user_id: Option<String>,
    },
    WorkflowExecution {
        workflow_id: Uuid,
        run_id: Uuid,
        status: String,
        node_id: Option<String>,
        result: Option<serde_json::Value>,
        user_id: Option<String>,
    },
    DashboardUpdate {
        widget: String,
        data: serde_json::Value,
        user_id: Option<String>,
    },
    SystemStatus {
        status: String,
        message: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    Heartbeat {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

#[derive(Clone)]
pub struct WebSocketManager {
    connections: Arc<RwLock<HashMap<String, broadcast::Sender<WebSocketMessage>>>>,
    message_channel: broadcast::Sender<WebSocketMessage>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        let (message_channel, _) = broadcast::channel(1000);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            message_channel,
        }
    }

    pub async fn add_connection(&self, connection_id: String) -> broadcast::Sender<WebSocketMessage> {
        let (tx, _) = broadcast::channel(100);
        self.connections.write().await.insert(connection_id.clone(), tx.clone());
        
        // Send welcome message
        let welcome = WebSocketMessage::SystemStatus {
            status: "connected".to_string(),
            message: "WebSocket connection established".to_string(),
            timestamp: chrono::Utc::now(),
        };
        let _ = tx.send(welcome);
        
        tx
    }

    pub async fn remove_connection(&self, connection_id: &str) {
        self.connections.write().await.remove(connection_id);
    }

    pub async fn broadcast(&self, message: WebSocketMessage) {
        let _ = self.message_channel.send(message);
    }

    pub async fn send_to_user(&self, _user_id: &str, message: WebSocketMessage) {
        let connections = self.connections.read().await;
        for (_conn_id, tx) in connections.iter() {
            // In a real implementation, you'd map connection IDs to user IDs
            // For now, we'll broadcast to all connections
            let _ = tx.send(message.clone());
        }
    }

    pub async fn get_connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/ws", get(websocket_handler))
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let connection_id = Uuid::new_v4().to_string();
    let tx = state.websocket_manager.add_connection(connection_id.clone()).await;
    let mut broadcast_rx = tx.subscribe();

    let (mut sender, mut receiver) = socket.split();

    // Task for outgoing messages: Broadcast -> Client
    let connection_id_clone = connection_id.clone();
    let websocket_manager_clone = state.websocket_manager.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            let message_text = match serde_json::to_string(&msg) {
                Ok(text) => text,
                Err(e) => {
                    tracing::error!("Failed to serialize WebSocket message: {}", e);
                    continue;
                }
            };
            if sender.send(Message::Text(message_text)).await.is_err() {
                break;
            }
        }
        websocket_manager_clone.remove_connection(&connection_id_clone).await;
    });

    // Task for incoming messages: Client -> Server
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                    match ws_message {
                        WebSocketMessage::Heartbeat { .. } => {
                            let response = WebSocketMessage::Heartbeat {
                                timestamp: chrono::Utc::now(),
                            };
                            let _ = tx.send(response);
                        }
                        _ => {
                            tracing::debug!("Received WebSocket message: {:?}", ws_message);
                        }
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

// Helper functions for sending specific message types

pub async fn send_notification(
    websocket_manager: &WebSocketManager,
    title: &str,
    message: &str,
    level: &str,
    user_id: Option<&str>,
) {
    let notification = WebSocketMessage::Notification {
        id: Uuid::new_v4().to_string(),
        title: title.to_string(),
        message: message.to_string(),
        level: level.to_string(),
        timestamp: chrono::Utc::now(),
        user_id: user_id.map(|s| s.to_string()),
    };

    if let Some(user_id) = user_id {
        websocket_manager.send_to_user(user_id, notification).await;
    } else {
        websocket_manager.broadcast(notification).await;
    }
}

pub async fn send_lead_update(
    websocket_manager: &WebSocketManager,
    lead_id: Uuid,
    action: &str,
    data: serde_json::Value,
    user_id: Option<&str>,
) {
    let update = WebSocketMessage::LeadUpdate {
        lead_id,
        action: action.to_string(),
        data,
        user_id: user_id.map(|s| s.to_string()),
    };

    if let Some(user_id) = user_id {
        websocket_manager.send_to_user(user_id, update).await;
    } else {
        websocket_manager.broadcast(update).await;
    }
}

pub async fn send_deal_update(
    websocket_manager: &WebSocketManager,
    deal_id: Uuid,
    action: &str,
    data: serde_json::Value,
    user_id: Option<&str>,
) {
    let update = WebSocketMessage::DealUpdate {
        deal_id,
        action: action.to_string(),
        data,
        user_id: user_id.map(|s| s.to_string()),
    };

    if let Some(user_id) = user_id {
        websocket_manager.send_to_user(user_id, update).await;
    } else {
        websocket_manager.broadcast(update).await;
    }
}

pub async fn send_workflow_execution_update(
    websocket_manager: &WebSocketManager,
    workflow_id: Uuid,
    run_id: Uuid,
    status: &str,
    node_id: Option<String>,
    result: Option<serde_json::Value>,
    user_id: Option<&str>,
) {
    let update = WebSocketMessage::WorkflowExecution {
        workflow_id,
        run_id,
        status: status.to_string(),
        node_id,
        result,
        user_id: user_id.map(|s| s.to_string()),
    };

    if let Some(user_id) = user_id {
        websocket_manager.send_to_user(user_id, update).await;
    } else {
        websocket_manager.broadcast(update).await;
    }
}

pub async fn send_dashboard_update(
    websocket_manager: &WebSocketManager,
    widget: &str,
    data: serde_json::Value,
    user_id: Option<&str>,
) {
    let update = WebSocketMessage::DashboardUpdate {
        widget: widget.to_string(),
        data,
        user_id: user_id.map(|s| s.to_string()),
    };

    if let Some(user_id) = user_id {
        websocket_manager.send_to_user(user_id, update).await;
    } else {
        websocket_manager.broadcast(update).await;
    }
}
