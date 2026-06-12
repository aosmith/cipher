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

/// Reference to a blob stored in iroh-blobs
/// Used in P2P messages instead of inline data for large attachments
/// Blob data is encrypted with XChaCha20Poly1305 before storage
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobReference {
    pub id: SqliteUuid,    // Attachment ID (matches MediaAttachment)
    pub file_type: String, // MIME type (e.g., "image/jpeg")
    pub file_size: i64,    // Size in bytes (of original unencrypted data)
    pub blob_hash: String, // iroh-blobs Hash as hex string
    #[serde(default)]
    pub downloaded: bool, // Whether the blob has been downloaded to local store
    #[serde(default)]
    pub encryption_key: Option<String>, // Base64-encoded 32-byte XChaCha20Poly1305 key
}
