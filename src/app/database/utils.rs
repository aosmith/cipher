use chrono::Utc;
use rusqlite::Result as SqliteResult;

use crate::app::Database;

impl Database {
    pub fn cleanup_expired_messages(&self) -> SqliteResult<usize> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let deleted = conn.execute(
            "DELETE FROM messages WHERE disappears_at IS NOT NULL AND disappears_at < ?1",
            [&now],
        )?;

        Ok(deleted)
    }
}
