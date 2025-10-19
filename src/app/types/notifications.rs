use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
// Removed WebSocket imports - using Iroh for notifications now
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Persistent notification stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub data: Option<String>,
    pub read: bool,
    pub created_at: String,
}

// Real-time notification message for Iroh P2P
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationMessage {
    pub notification_type: String, // "new_message", "friend_request", "post_update", etc.
    pub data: serde_json::Value,
    pub timestamp: String,
    pub user_id: SqliteUuid,
}

pub struct NotificationServer {
    pub broadcaster: broadcast::Sender<NotificationMessage>,
    pub is_running: Arc<AtomicBool>,
}

impl Default for NotificationServer {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationServer {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self {
            broadcaster: tx,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self, _port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // WebSocket notification server disabled - using Iroh for notifications
        self.is_running.store(true, Ordering::Relaxed);
        println!("Notification server (stub) - using Iroh for real-time notifications");
        Ok(())
    }

    pub fn broadcast(&self, notification: NotificationMessage) {
        if let Err(e) = self.broadcaster.send(notification) {
            eprintln!("Failed to broadcast notification: {}", e);
        }
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
    }
}

// WebSocket connection handler removed - using Iroh Gossip for notifications
// This stub is kept to prevent breaking changes to existing code
//
// fn handle_websocket_connection() - REMOVED
// Notifications are now sent via Iroh Gossip topics
