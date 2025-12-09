use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QrCodeData {
    pub display_name: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAttachment {
    pub id: SqliteUuid,
    pub post_id: SqliteUuid,
    pub file_type: String,
    pub file_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAttachmentWithData {
    pub id: SqliteUuid,
    pub post_id: SqliteUuid,
    pub file_type: String,
    pub file_size: i64,
    pub data: String, // Base64 encoded
}
