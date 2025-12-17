use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};
use serde::{Deserialize, Serialize};

use crate::app::Database;

/// Global app settings response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub storage_limit_bytes: i64,
    pub relay_limit_bytes: i64,
    pub storage_used_bytes: i64,
    pub relay_used_bytes: i64,
    pub relay_reset_at: String,
}

impl Database {
    /// Get a single setting value
    pub fn get_setting(&self, key: &str) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Set a single setting value
    pub fn set_setting(&self, key: &str, value: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            params![key, value, now],
        )?;

        Ok(())
    }

    /// Get all app settings at once
    pub fn get_app_settings(&self) -> SqliteResult<AppSettings> {
        // Default storage: 4 GB (4294967296 bytes)
        let storage_limit = self
            .get_setting("storage_limit_bytes")?
            .unwrap_or_else(|| "4294967296".to_string())
            .parse::<i64>()
            .unwrap_or(4294967296);

        // Default relay: 2 GB/month (2147483648 bytes)
        let relay_limit = self
            .get_setting("relay_limit_bytes")?
            .unwrap_or_else(|| "2147483648".to_string())
            .parse::<i64>()
            .unwrap_or(2147483648);

        let storage_used = self
            .get_setting("storage_used_bytes")?
            .unwrap_or_else(|| "0".to_string())
            .parse::<i64>()
            .unwrap_or(0);

        let relay_used = self
            .get_setting("relay_used_bytes")?
            .unwrap_or_else(|| "0".to_string())
            .parse::<i64>()
            .unwrap_or(0);

        let relay_reset_at = self
            .get_setting("relay_reset_at")?
            .unwrap_or_else(|| Utc::now().to_rfc3339());

        Ok(AppSettings {
            storage_limit_bytes: storage_limit,
            relay_limit_bytes: relay_limit,
            storage_used_bytes: storage_used,
            relay_used_bytes: relay_used,
            relay_reset_at,
        })
    }

    /// Set storage limit in bytes
    pub fn set_storage_limit(&self, bytes: i64) -> SqliteResult<()> {
        self.set_setting("storage_limit_bytes", &bytes.to_string())
    }

    /// Set relay limit in bytes (-1 for unlimited)
    pub fn set_relay_limit(&self, bytes: i64) -> SqliteResult<()> {
        self.set_setting("relay_limit_bytes", &bytes.to_string())
    }

    /// Add to storage used counter
    pub fn add_storage_used(&self, bytes: i64) -> SqliteResult<i64> {
        let current = self
            .get_setting("storage_used_bytes")?
            .unwrap_or_else(|| "0".to_string())
            .parse::<i64>()
            .unwrap_or(0);

        let new_total = current + bytes;
        self.set_setting("storage_used_bytes", &new_total.to_string())?;
        Ok(new_total)
    }

    /// Add to relay used counter (with monthly reset check)
    pub fn add_relay_used(&self, bytes: i64) -> SqliteResult<i64> {
        // Check if we need to reset (monthly)
        let reset_at = self
            .get_setting("relay_reset_at")?
            .unwrap_or_else(|| Utc::now().to_rfc3339());

        let reset_time = chrono::DateTime::parse_from_rfc3339(&reset_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let now = Utc::now();
        let current = if now.signed_duration_since(reset_time).num_days() >= 30 {
            // Reset counter and update reset time
            self.set_setting("relay_reset_at", &now.to_rfc3339())?;
            self.set_setting("relay_used_bytes", "0")?;
            0
        } else {
            self.get_setting("relay_used_bytes")?
                .unwrap_or_else(|| "0".to_string())
                .parse::<i64>()
                .unwrap_or(0)
        };

        let new_total = current + bytes;
        self.set_setting("relay_used_bytes", &new_total.to_string())?;
        Ok(new_total)
    }

    /// Check if storage quota allows adding more bytes
    pub fn can_store(&self, bytes: i64) -> SqliteResult<bool> {
        let settings = self.get_app_settings()?;
        if settings.storage_limit_bytes <= 0 {
            return Ok(false); // Storage disabled
        }
        Ok(settings.storage_used_bytes + bytes <= settings.storage_limit_bytes)
    }

    /// Check if relay quota allows relaying more bytes
    pub fn can_relay(&self, bytes: i64) -> SqliteResult<bool> {
        let settings = self.get_app_settings()?;
        if settings.relay_limit_bytes == -1 {
            return Ok(true); // Unlimited
        }
        if settings.relay_limit_bytes <= 0 {
            return Ok(false); // Relay disabled
        }
        Ok(settings.relay_used_bytes + bytes <= settings.relay_limit_bytes)
    }
}
