use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{Message, NotificationMessage, SqliteUuid};
use crate::app::Database;

impl Database {
    pub fn send_encrypted_message(
        &self,
        sender_id: SqliteUuid,
        recipient_id: SqliteUuid,
        content: &str,
        disappear_after_seconds: Option<i64>,
    ) -> SqliteResult<Message> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Get sender's private keys and recipient's public key
        let mut sender_stmt =
            conn.prepare("SELECT private_key, encryption_private_key FROM users WHERE id = ?1")?;
        let sender_keys: (String, String) = sender_stmt.query_row([sender_id], |row| {
            Ok((row.get("private_key")?, row.get("encryption_private_key")?))
        })?;

        let mut recipient_stmt =
            conn.prepare("SELECT encryption_public_key FROM users WHERE id = ?1")?;
        let recipient_public_key: String =
            recipient_stmt.query_row([recipient_id], |row| row.get("encryption_public_key"))?;

        // Encrypt the message
        let encrypted_content =
            Self::encrypt_message(content, &recipient_public_key, &sender_keys.1).map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ABORT),
                    Some(e),
                )
            })?;

        // Sign the original message
        let signature = Self::sign_message(content, &sender_keys.0).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ABORT),
                Some(e),
            )
        })?;

        // Calculate disappears_at timestamp if disappearing message
        let disappears_at = if let Some(seconds) = disappear_after_seconds {
            let expire_time = Utc::now() + chrono::Duration::seconds(seconds);
            Some(expire_time.to_rfc3339())
        } else {
            None
        };

        // Store the message
        let message_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO messages (id, sender_id, recipient_id, content, encrypted, signature, disappear_after_seconds, disappears_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                message_id,
                sender_id,
                recipient_id,
                encrypted_content,
                true,
                signature,
                disappear_after_seconds,
                disappears_at,
                now,
                now
            ],
        )?;

        let message = Message {
            id: message_id,
            sender_id,
            recipient_id,
            content: encrypted_content,
            encrypted: true,
            signature: Some(signature),
            thread_id: None,
            disappear_after_seconds,
            disappears_at,
            created_at: now.clone(),
            updated_at: now,
        };

        // Broadcast notification for new message
        let notification = NotificationMessage {
            notification_type: "new_message".to_string(),
            data: serde_json::to_value(&message).unwrap_or(serde_json::Value::Null),
            timestamp: Utc::now().to_rfc3339(),
            user_id: recipient_id,
        };
        self.notification_server.broadcast(notification);

        Ok(message)
    }

    pub fn get_messages_for_user(&self, user_id: SqliteUuid) -> SqliteResult<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, content, encrypted, signature, thread_id, disappear_after_seconds, disappears_at, created_at, updated_at
             FROM messages WHERE (sender_id = ?1 OR recipient_id = ?1) AND (disappears_at IS NULL OR disappears_at > ?2) ORDER BY created_at DESC"
        )?;

        let message_iter = stmt.query_map(params![user_id, &now], |row| {
            Ok(Message {
                id: row.get("id")?,
                sender_id: row.get("sender_id")?,
                recipient_id: row.get("recipient_id")?,
                content: row.get("content")?,
                encrypted: row.get("encrypted")?,
                signature: row.get("signature")?,
                thread_id: row.get("thread_id")?,
                disappear_after_seconds: row.get("disappear_after_seconds")?,
                disappears_at: row.get("disappears_at")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        let mut messages = Vec::new();
        for message in message_iter {
            messages.push(message?);
        }
        Ok(messages)
    }
}
