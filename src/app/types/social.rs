use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P2pConnection {
    pub id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub friend_user_id: SqliteUuid,
    pub status: String,
    pub initiated_by: SqliteUuid,
    pub created_at: String,
    pub updated_at: String,
}

// Friend management structures for P2P-friendly features
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendInvite {
    pub id: SqliteUuid,
    pub creator_id: SqliteUuid,
    pub invite_code: String,
    pub public_key: String,
    pub username: String,
    pub uses_remaining: i32,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendExport {
    pub username: String,
    pub public_key: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendImportResult {
    pub added: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentContact {
    pub user_id: SqliteUuid,
    pub username: String,
    pub public_key: String,
    pub last_interaction: String,
    pub interaction_count: i32,
}
