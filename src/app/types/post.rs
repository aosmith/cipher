use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub display_name: Option<String>, // Display name of post author (not unique)
    pub content: String,
    pub encrypted: bool,
    pub pinned: bool,
    pub shared_post_id: Option<SqliteUuid>,
    pub share_comment: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostReaction {
    pub id: SqliteUuid,
    pub post_id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub display_name: Option<String>,
    pub emoji: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostComment {
    pub id: SqliteUuid,
    pub post_id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub display_name: Option<String>,
    pub public_key: Option<String>,
    pub content: String,
    pub parent_comment_id: Option<SqliteUuid>,
    pub created_at: String,
    pub updated_at: String,
}
