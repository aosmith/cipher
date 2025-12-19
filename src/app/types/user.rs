use super::uuid::SqliteUuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: SqliteUuid,
    pub display_name: String,
    pub public_key: Option<String>,
    #[serde(skip_serializing)] // CRITICAL: Never serialize private keys
    #[allow(dead_code)] // Field is used but not detected by dead code analysis
    pub private_key: Option<String>,
    pub encryption_public_key: Option<String>,
    #[serde(skip_serializing)] // CRITICAL: Never serialize private keys
    #[allow(dead_code)] // Field is used but not detected by dead code analysis
    pub encryption_private_key: Option<String>,
    pub device_id: Option<String>,
    pub bio: Option<String>,
    pub profile_picture: Option<String>,
    /// Cryptographic signature of profile data (display_name|bio|profile_picture)
    /// Used by friends to verify the profile hasn't been tampered with
    pub profile_signature: Option<String>,
    #[serde(skip_serializing)] // Never serialize recovery phrase hash
    #[allow(dead_code)] // Field is used but not detected by dead code analysis
    pub recovery_phrase_hash: Option<String>,
    pub recovery_phrase_shown: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct KeyPair {
    pub public_key: String,
    #[serde(skip_serializing)] // CRITICAL: Never serialize private keys
    pub private_key: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct NewUser {
    pub display_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserWithRecoveryPhrase {
    pub user: User,
    pub recovery_phrase: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct UserRegistrationResponse {
    pub user: User,
    pub recovery_phrase: String,
}
