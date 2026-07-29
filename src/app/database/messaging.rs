use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{Message, MessageReaction, NotificationMessage, SqliteUuid};
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
            edited_at: None,
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

    /// Persist an incoming P2P direct message. The plaintext arrived inside a
    /// sealed envelope (sender already authenticated); store it encrypted at
    /// rest to the RECIPIENT's own encryption key - the same representation
    /// messages addressed to us have when they arrive via device sync from
    /// the sender's device (decryptable with the recipient's private key), so
    /// the conversation view renders both sides uniformly.
    /// Returns Ok(true) when inserted, Ok(false) when the message id already
    /// exists (gossip can redeliver the same envelope).
    pub fn store_received_message(
        &self,
        message_id: SqliteUuid,
        sender_id: SqliteUuid,
        recipient_id: SqliteUuid,
        content: &str,
        thread_id: Option<SqliteUuid>,
        created_at_rfc3339: &str,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();

        // Encrypt to the recipient's (our own) key so the plaintext never
        // sits in the database
        let recipient_public_key: String = conn.query_row(
            "SELECT encryption_public_key FROM users WHERE id = ?1",
            [recipient_id],
            |row| row.get("encryption_public_key"),
        )?;

        // encrypt_message uses an ephemeral sender key internally; the sender
        // private key parameter is unused (we don't hold the remote sender's)
        let encrypted_content =
            Self::encrypt_message(content, &recipient_public_key, "").map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ABORT),
                    Some(e),
                )
            })?;

        // No signature: the sender's signature covered the sealed envelope,
        // not this locally re-encrypted representation
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO messages
             (id, sender_id, recipient_id, content, encrypted, signature, thread_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                message_id,
                sender_id,
                recipient_id,
                encrypted_content,
                true,
                Option::<String>::None,
                thread_id,
                created_at_rfc3339,
                created_at_rfc3339,
            ],
        )?;

        if inserted > 0 {
            // Same in-app notification the send path broadcasts
            let message = Message {
                id: message_id,
                sender_id,
                recipient_id,
                content: encrypted_content,
                encrypted: true,
                signature: None,
                thread_id,
                disappear_after_seconds: None,
                disappears_at: None,
                created_at: created_at_rfc3339.to_string(),
                updated_at: created_at_rfc3339.to_string(),
                edited_at: None,
            };
            let notification = NotificationMessage {
                notification_type: "new_message".to_string(),
                data: serde_json::to_value(&message).unwrap_or(serde_json::Value::Null),
                timestamp: Utc::now().to_rfc3339(),
                user_id: recipient_id,
            };
            self.notification_server.broadcast(notification);
        }

        Ok(inserted > 0)
    }

    pub fn get_messages_for_user(&self, user_id: SqliteUuid) -> SqliteResult<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, content, encrypted, signature, thread_id, disappear_after_seconds, disappears_at, created_at, updated_at, edited_at
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
                edited_at: row.get("edited_at")?,
            })
        })?;

        let mut messages = Vec::new();
        for message in message_iter {
            messages.push(message?);
        }
        Ok(messages)
    }

    /// Add a reaction to a message
    pub fn add_message_reaction(
        &self,
        message_id: SqliteUuid,
        user_id: SqliteUuid,
        emoji: &str,
    ) -> SqliteResult<MessageReaction> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let reaction_id = SqliteUuid::new();

        conn.execute(
            "INSERT OR REPLACE INTO message_reactions (id, message_id, user_id, emoji, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![reaction_id, message_id, user_id, emoji, &now],
        )?;

        Ok(MessageReaction {
            id: reaction_id,
            message_id,
            user_id,
            emoji: emoji.to_string(),
            created_at: now,
        })
    }

    /// Get all reactions for a message
    pub fn get_message_reactions(
        &self,
        message_id: SqliteUuid,
    ) -> SqliteResult<Vec<MessageReaction>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, message_id, user_id, emoji, created_at
             FROM message_reactions
             WHERE message_id = ?1
             ORDER BY created_at ASC",
        )?;

        let reaction_iter = stmt.query_map([message_id], |row| {
            Ok(MessageReaction {
                id: row.get("id")?,
                message_id: row.get("message_id")?,
                user_id: row.get("user_id")?,
                emoji: row.get("emoji")?,
                created_at: row.get("created_at")?,
            })
        })?;

        let mut reactions = Vec::new();
        for reaction in reaction_iter {
            reactions.push(reaction?);
        }
        Ok(reactions)
    }

    /// Remove a reaction from a message
    pub fn remove_message_reaction(
        &self,
        message_id: SqliteUuid,
        user_id: SqliteUuid,
        emoji: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM message_reactions WHERE message_id = ?1 AND user_id = ?2 AND emoji = ?3",
            params![message_id, user_id, emoji],
        )?;
        Ok(())
    }

    /// Reply to a message (creates a new message with thread_id pointing to parent)
    pub fn reply_to_message(
        &self,
        sender_id: SqliteUuid,
        recipient_id: SqliteUuid,
        content: &str,
        parent_message_id: SqliteUuid,
        disappear_after_seconds: Option<i64>,
    ) -> SqliteResult<Message> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Get sender's private keys and recipient's public key
        let sender_keys: (String, String) = conn.query_row(
            "SELECT private_key, encryption_private_key FROM users WHERE id = ?1",
            [sender_id],
            |row| Ok((row.get("private_key")?, row.get("encryption_private_key")?)),
        )?;

        let recipient_public_key: String = conn.query_row(
            "SELECT encryption_public_key FROM users WHERE id = ?1",
            [recipient_id],
            |row| row.get("encryption_public_key"),
        )?;

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

        // Calculate disappears_at timestamp
        let disappears_at = disappear_after_seconds
            .map(|seconds| (Utc::now() + chrono::Duration::seconds(seconds)).to_rfc3339());

        let message_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO messages (id, sender_id, recipient_id, content, encrypted, signature, thread_id, disappear_after_seconds, disappears_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                message_id,
                sender_id,
                recipient_id,
                encrypted_content,
                true,
                signature,
                parent_message_id,
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
            thread_id: Some(parent_message_id),
            disappear_after_seconds,
            disappears_at,
            created_at: now.clone(),
            updated_at: now,
            edited_at: None,
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

    /// Get all messages in a thread (the original message plus all replies)
    pub fn get_message_thread(&self, thread_id: SqliteUuid) -> SqliteResult<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Get the original message and all replies to it
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, content, encrypted, signature, thread_id, disappear_after_seconds, disappears_at, created_at, updated_at, edited_at
             FROM messages
             WHERE (id = ?1 OR thread_id = ?1)
               AND (disappears_at IS NULL OR disappears_at > ?2)
             ORDER BY created_at ASC",
        )?;

        let message_iter = stmt.query_map(params![thread_id, &now], |row| {
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
                edited_at: row.get("edited_at")?,
            })
        })?;

        let mut messages = Vec::new();
        for message in message_iter {
            messages.push(message?);
        }
        Ok(messages)
    }

    /// Search messages by content (searches decrypted content would require decryption)
    /// For encrypted messages, this searches the encrypted blob which won't match plaintext
    /// For a proper search, messages would need to be decrypted first or indexed separately
    pub fn search_messages(&self, user_id: SqliteUuid, query: &str) -> SqliteResult<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let search_pattern = format!("%{}%", query);

        // Note: This only works for unencrypted messages
        // For encrypted messages, you'd need to decrypt and search in memory
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, content, encrypted, signature, thread_id, disappear_after_seconds, disappears_at, created_at, updated_at, edited_at
             FROM messages
             WHERE (sender_id = ?1 OR recipient_id = ?1)
               AND encrypted = 0
               AND content LIKE ?2
               AND (disappears_at IS NULL OR disappears_at > ?3)
             ORDER BY created_at DESC",
        )?;

        let message_iter = stmt.query_map(params![user_id, &search_pattern, &now], |row| {
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
                edited_at: row.get("edited_at")?,
            })
        })?;

        let mut messages = Vec::new();
        for message in message_iter {
            messages.push(message?);
        }
        Ok(messages)
    }

    /// Edit a message (only sender can edit)
    pub fn edit_message(
        &self,
        message_id: SqliteUuid,
        user_id: SqliteUuid,
        new_content: &str,
    ) -> SqliteResult<Message> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Verify the user is the sender
        let (sender_id, recipient_id): (SqliteUuid, SqliteUuid) = conn.query_row(
            "SELECT sender_id, recipient_id FROM messages WHERE id = ?1",
            [message_id],
            |row| Ok((row.get("sender_id")?, row.get("recipient_id")?)),
        )?;

        if sender_id != user_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        // Get keys for re-encryption
        let sender_keys: (String, String) = conn.query_row(
            "SELECT private_key, encryption_private_key FROM users WHERE id = ?1",
            [user_id],
            |row| Ok((row.get("private_key")?, row.get("encryption_private_key")?)),
        )?;

        let recipient_public_key: String = conn.query_row(
            "SELECT encryption_public_key FROM users WHERE id = ?1",
            [recipient_id],
            |row| row.get("encryption_public_key"),
        )?;

        // Re-encrypt the new content
        let encrypted_content =
            Self::encrypt_message(new_content, &recipient_public_key, &sender_keys.1).map_err(
                |e| {
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ABORT),
                        Some(e),
                    )
                },
            )?;

        // Re-sign the new content
        let signature = Self::sign_message(new_content, &sender_keys.0).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ABORT),
                Some(e),
            )
        })?;

        // Persist edited_at so every read path reports the edit, not just the
        // value returned from this call
        conn.execute(
            "UPDATE messages SET content = ?1, encrypted = 1, signature = ?2, updated_at = ?3, edited_at = ?3 WHERE id = ?4",
            params![encrypted_content, signature, &now, message_id],
        )?;

        // Fetch and return the updated message
        let mut stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, content, encrypted, signature, thread_id, disappear_after_seconds, disappears_at, created_at, updated_at, edited_at
             FROM messages WHERE id = ?1",
        )?;

        stmt.query_row([message_id], |row| {
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
                edited_at: row.get("edited_at")?,
            })
        })
    }

    /// Delete a message (only sender or recipient can delete)
    pub fn delete_message(&self, message_id: SqliteUuid, user_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Verify the user is either sender or recipient
        let (sender_id, recipient_id): (SqliteUuid, SqliteUuid) = conn.query_row(
            "SELECT sender_id, recipient_id FROM messages WHERE id = ?1",
            [message_id],
            |row| Ok((row.get("sender_id")?, row.get("recipient_id")?)),
        )?;

        if sender_id != user_id && recipient_id != user_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        // Delete the message (ON DELETE CASCADE will handle reactions)
        conn.execute("DELETE FROM messages WHERE id = ?1", [message_id])?;
        Ok(())
    }
}
