use crate::app::types::SqliteUuid;
use crate::app::Database;
use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

impl Database {
    /// Set typing indicator for a conversation
    pub fn set_typing_indicator(
        &self,
        user_id: SqliteUuid,
        conversation_partner_id: SqliteUuid,
        is_typing: bool,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO typing_indicators (user_id, conversation_partner_id, is_typing, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![user_id, conversation_partner_id, is_typing, now],
        )?;

        Ok(())
    }

    /// Get typing indicator status for a user in a conversation
    pub fn get_typing_indicator(
        &self,
        user_id: SqliteUuid,
        conversation_partner_id: SqliteUuid,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();

        // Get typing status and check if it's recent (within last 10 seconds)
        let result: Result<(bool, String), _> = conn.query_row(
            "SELECT is_typing, updated_at FROM typing_indicators
             WHERE user_id = ?1 AND conversation_partner_id = ?2",
            params![user_id, conversation_partner_id],
            |row| Ok((row.get("is_typing")?, row.get("updated_at")?)),
        );

        match result {
            Ok((is_typing, updated_at)) => {
                // Parse the timestamp and check if it's recent (within 10 seconds)
                let now = Utc::now();
                if let Ok(updated) = chrono::DateTime::parse_from_rfc3339(&updated_at) {
                    let duration = now.signed_duration_since(updated);
                    if duration.num_seconds() > 10 {
                        // Typing indicator expired, return false
                        return Ok(false);
                    }
                }
                Ok(is_typing)
            }
            Err(_) => Ok(false), // No typing indicator found
        }
    }

    /// Clear old typing indicators (cleanup function)
    pub fn cleanup_old_typing_indicators(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let cutoff = (Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();

        conn.execute(
            "DELETE FROM typing_indicators WHERE updated_at < ?1",
            params![cutoff],
        )?;

        Ok(())
    }

    /// Clear typing indicator for a specific conversation
    pub fn clear_typing_indicator(
        &self,
        user_id: SqliteUuid,
        conversation_partner_id: SqliteUuid,
    ) -> SqliteResult<()> {
        self.set_typing_indicator(user_id, conversation_partner_id, false)
    }
}
