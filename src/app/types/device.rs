use serde::{Deserialize, Serialize};

/// Represents a device in a multi-device setup
/// Each user can have multiple devices (phone, tablet, desktop, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Unique device identifier (UUID)
    pub id: String,

    /// User's public key this device belongs to
    pub user_public_key: String,

    /// Optional human-readable name for the device (e.g., "iPhone 13", "MacBook Pro")
    pub device_name: Option<String>,

    /// Timestamp of last sync with this device
    pub last_sync: String,

    /// When the device was first registered
    pub created_at: String,
}

impl Device {
    /// Create a new Device instance
    #[allow(dead_code)]
    pub fn new(
        id: String,
        user_public_key: String,
        device_name: Option<String>,
        last_sync: String,
        created_at: String,
    ) -> Self {
        Device {
            id,
            user_public_key,
            device_name,
            last_sync,
            created_at,
        }
    }
}

/// Response when listing devices for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub device_name: Option<String>,
    pub last_sync: String,
    pub created_at: String,
    pub is_current: bool, // True if this is the device making the request
}
