use rusqlite::{params, Result as SqliteResult};
use crate::app::types::SqliteUuid;
use crate::app::Database;

/// Pending message for offline delivery
#[derive(Debug, Clone)]
pub struct PendingMessage {
    pub id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub message_type: String, // "post" or "message"
    pub content_json: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub created_at: String,
    pub last_attempt_at: Option<String>,
}

impl Database {
    /// Queue a message for offline delivery
    pub fn queue_pending_message(
        &self,
        user_id: SqliteUuid,
        message_type: &str,
        content_json: &str,
        max_retries: i32,
    ) -> SqliteResult<SqliteUuid> {
        let conn = self.conn.lock().unwrap();
        let message_id = SqliteUuid::new();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO pending_messages (id, user_id, message_type, content_json, retry_count, max_retries, created_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
            params![message_id, user_id, message_type, content_json, max_retries, &now],
        )?;

        println!(
            "[QUEUE] ✓ Queued {} message (ID: {})",
            message_type, message_id
        );
        Ok(message_id)
    }

    /// Get all pending messages ready for retry
    pub fn get_pending_messages(&self, user_id: SqliteUuid) -> SqliteResult<Vec<PendingMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, message_type, content_json, retry_count, max_retries, created_at, last_attempt_at
             FROM pending_messages
             WHERE user_id = ?1 AND retry_count < max_retries
             ORDER BY created_at ASC",
        )?;

        let messages = stmt
            .query_map([user_id], |row| {
                Ok(PendingMessage {
                    id: row.get("id")?,
                    user_id: row.get("user_id")?,
                    message_type: row.get("message_type")?,
                    content_json: row.get("content_json")?,
                    retry_count: row.get("retry_count")?,
                    max_retries: row.get("max_retries")?,
                    created_at: row.get("created_at")?,
                    last_attempt_at: row.get("last_attempt_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(messages)
    }

    /// Mark a pending message as successfully sent and remove it
    pub fn mark_message_sent(&self, message_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM pending_messages WHERE id = ?1",
            [message_id],
        )?;
        println!("[QUEUE] ✓ Removed sent message from queue (ID: {})", message_id);
        Ok(())
    }

    /// Increment retry count for a pending message
    pub fn increment_retry_count(&self, message_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE pending_messages
             SET retry_count = retry_count + 1, last_attempt_at = ?1
             WHERE id = ?2",
            params![&now, message_id],
        )?;
        Ok(())
    }

    /// Remove message from queue (failed max retries)
    pub fn remove_pending_message(&self, message_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM pending_messages WHERE id = ?1",
            [message_id],
        )?;
        println!(
            "[QUEUE] ✗ Removed message from queue after max retries (ID: {})",
            message_id
        );
        Ok(())
    }

    /// Get pending message count for user
    pub fn get_pending_message_count(&self, user_id: SqliteUuid) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pending_messages WHERE user_id = ?1 AND retry_count < max_retries",
            [user_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Clear all pending messages (manual sync)
    pub fn clear_pending_messages(&self, user_id: SqliteUuid) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM pending_messages WHERE user_id = ?1",
            [user_id],
        )?;

        // Get count of deleted messages for logging
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pending_messages WHERE user_id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(count)
    }
}
