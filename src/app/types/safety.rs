use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedUser {
    pub id: SqliteUuid,
    pub blocker_id: SqliteUuid,
    pub blocked_id: SqliteUuid,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutedUser {
    pub id: SqliteUuid,
    pub muter_id: SqliteUuid,
    pub muted_id: SqliteUuid,
    pub mute_notifications: bool,
    pub mute_messages: bool,
    pub mute_posts: bool,
    pub expires_at: Option<String>,
    pub created_at: String,
}
