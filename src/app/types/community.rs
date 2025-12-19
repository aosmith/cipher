use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};

/// A community is a group of users who can share posts and messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Community {
    pub id: SqliteUuid,
    pub name: String,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub creator_id: SqliteUuid,
    pub member_count: i32, // Computed field, not stored
    pub created_at: String,
    pub updated_at: String,
}

/// A member of a community
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityMember {
    pub id: SqliteUuid,
    pub community_id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub public_key: String,
    pub display_name: Option<String>,
    pub role: String, // "creator" or "member"
    pub invited_by: Option<SqliteUuid>,
    pub joined_at: String,
}

/// A post in a community (links post to community with visibility settings)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityPost {
    pub id: SqliteUuid,
    pub community_id: SqliteUuid,
    pub post_id: SqliteUuid,
    pub show_in_main_feed: bool,
    pub created_at: String,
}

/// An invite code to join a community
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityInvite {
    pub id: SqliteUuid,
    pub community_id: SqliteUuid,
    pub community_name: String, // For display purposes
    pub creator_id: SqliteUuid,
    pub invite_code: String,
    pub uses_remaining: i32,
    pub expires_at: String,
    pub created_at: String,
}

/// Community with full member list (for detail views)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityWithMembers {
    pub community: Community,
    pub members: Vec<CommunityMember>,
}

/// Request to create a new community
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommunityRequest {
    pub name: String,
    pub description: Option<String>,
}

/// Request to create a community invite
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommunityInviteRequest {
    pub community_id: SqliteUuid,
    pub uses_remaining: Option<i32>,
    pub hours_valid: Option<i64>,
}
