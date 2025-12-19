use rusqlite::{params, Result as SqliteResult};
use serde::{Deserialize, Serialize};

use crate::app::Database;

/// Global app settings response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub storage_limit_bytes: i64,
    pub storage_used_bytes: i64,
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
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            params![key, value, now],
        )?;

        Ok(())
    }

    /// Get all app settings at once
    pub fn get_app_settings(&self) -> SqliteResult<AppSettings> {
        // Default storage: 10 GB (10737418240 bytes)
        let storage_limit = self
            .get_setting("storage_limit_bytes")?
            .unwrap_or_else(|| "10737418240".to_string())
            .parse::<i64>()
            .unwrap_or(10737418240);

        let storage_used = self
            .get_setting("storage_used_bytes")?
            .unwrap_or_else(|| "0".to_string())
            .parse::<i64>()
            .unwrap_or(0);

        Ok(AppSettings {
            storage_limit_bytes: storage_limit,
            storage_used_bytes: storage_used,
        })
    }

    /// Set storage limit in bytes
    pub fn set_storage_limit(&self, bytes: i64) -> SqliteResult<()> {
        self.set_setting("storage_limit_bytes", &bytes.to_string())
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

    /// Check if storage quota allows adding more bytes
    pub fn can_store(&self, bytes: i64) -> SqliteResult<bool> {
        let settings = self.get_app_settings()?;
        if settings.storage_limit_bytes <= 0 {
            return Ok(false); // Storage disabled
        }
        Ok(settings.storage_used_bytes + bytes <= settings.storage_limit_bytes)
    }
}
