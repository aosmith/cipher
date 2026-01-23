use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: SqliteUuid,
    pub sender_id: SqliteUuid,
    pub recipient_id: SqliteUuid,
    pub content: String,
    pub encrypted: bool,
    pub signature: Option<String>,
    pub thread_id: Option<SqliteUuid>, // For threading - references another message
    pub disappear_after_seconds: Option<i64>, // Time in seconds after which message auto-deletes
    pub disappears_at: Option<String>, // Calculated timestamp when message will be deleted
    pub created_at: String,
    pub updated_at: String,
    pub edited_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageReaction {
    pub id: SqliteUuid,
    pub message_id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub emoji: String,
    pub created_at: String,
}
