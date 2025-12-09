use crate::app::types::{Message, Post, SqliteUuid};
use crate::app::Database;
use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncData {
    pub posts: Vec<Post>,
    pub messages: Vec<Message>,
    pub friends: Vec<FriendSync>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendSync {
    pub id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub friend_user_id: SqliteUuid,
    pub status: String,
    pub initiated_by: SqliteUuid,
    pub created_at: String,
    pub updated_at: String,
}

impl Database {
    /// Get data that needs to be synced to other devices (data created/updated since last sync)
    pub fn get_sync_data(&self, device_id: &str, user_id: SqliteUuid) -> SqliteResult<SyncData> {
        let conn = self.conn.lock().unwrap();

        // Get last sync timestamps for each table
        let last_post_sync = self.get_last_sync_timestamp(device_id, "posts")?;
        let last_message_sync = self.get_last_sync_timestamp(device_id, "messages")?;
        let last_friend_sync = self.get_last_sync_timestamp(device_id, "p2p_connections")?;

        // Get posts created/updated since last sync
        let mut post_stmt = conn.prepare(
            "SELECT p.id, p.user_id, u.display_name, p.content, p.encrypted, p.pinned, p.shared_post_id, p.share_comment, p.created_at, p.updated_at
             FROM posts p
             INNER JOIN users u ON p.user_id = u.id
             WHERE p.user_id = ?1 AND (p.updated_at > ?2 OR p.created_at > ?2)
             ORDER BY p.updated_at ASC"
        )?;

        let posts = post_stmt
            .query_map(params![user_id, last_post_sync], |row| {
                Ok(Post {
                    id: row.get("id")?,
                    user_id: row.get("user_id")?,
                    display_name: row.get("display_name")?,
                    content: row.get("content")?,
                    encrypted: row.get("encrypted")?,
                    pinned: row.get("pinned")?,
                    shared_post_id: row.get("shared_post_id")?,
                    share_comment: row.get("share_comment")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Get messages sent or received since last sync
        let mut message_stmt = conn.prepare(
            "SELECT id, sender_id, recipient_id, content, encrypted, signature, thread_id,
                    disappear_after_seconds, disappears_at, created_at, updated_at
             FROM messages
             WHERE (sender_id = ?1 OR recipient_id = ?1) AND (updated_at > ?2 OR created_at > ?2)
             ORDER BY updated_at ASC",
        )?;

        let messages = message_stmt
            .query_map(params![user_id, last_message_sync], |row| {
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
                    edited_at: None,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Get friend connections created/updated since last sync
        let mut friend_stmt = conn.prepare(
            "SELECT id, user_id, friend_user_id, status, initiated_by, created_at, updated_at
             FROM p2p_connections
             WHERE user_id = ?1 AND (updated_at > ?2 OR created_at > ?2)
             ORDER BY updated_at ASC",
        )?;

        let friends = friend_stmt
            .query_map(params![user_id, last_friend_sync], |row| {
                Ok(FriendSync {
                    id: row.get("id")?,
                    user_id: row.get("user_id")?,
                    friend_user_id: row.get("friend_user_id")?,
                    status: row.get("status")?,
                    initiated_by: row.get("initiated_by")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SyncData {
            posts,
            messages,
            friends,
        })
    }

    /// Apply synced data from another device with timestamp-based conflict resolution
    pub fn apply_sync_data(&self, sync_data: &SyncData) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Insert or update posts - only if incoming data is newer or doesn't exist
        for post in &sync_data.posts {
            // Check if post exists and get its updated_at timestamp
            let existing_updated_at: Option<String> = conn
                .query_row(
                    "SELECT updated_at FROM posts WHERE id = ?1",
                    params![post.id],
                    |row| row.get("updated_at"),
                )
                .ok();

            // Only update if post doesn't exist or incoming version is newer
            if existing_updated_at.is_none()
                || existing_updated_at.as_ref().unwrap() < &post.updated_at
            {
                conn.execute(
                    "INSERT OR REPLACE INTO posts (id, user_id, content, encrypted, pinned, shared_post_id, share_comment, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        post.id,
                        post.user_id,
                        post.content,
                        post.encrypted,
                        post.pinned,
                        post.shared_post_id,
                        post.share_comment,
                        post.created_at,
                        post.updated_at,
                    ],
                )?;
            }
        }

        // Insert or update messages - only if incoming data is newer or doesn't exist
        for message in &sync_data.messages {
            let existing_updated_at: Option<String> = conn
                .query_row(
                    "SELECT updated_at FROM messages WHERE id = ?1",
                    params![message.id],
                    |row| row.get("updated_at"),
                )
                .ok();

            if existing_updated_at.is_none()
                || existing_updated_at.as_ref().unwrap() < &message.updated_at
            {
                conn.execute(
                    "INSERT OR REPLACE INTO messages
                     (id, sender_id, recipient_id, content, encrypted, signature, thread_id,
                      disappear_after_seconds, disappears_at, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        message.id,
                        message.sender_id,
                        message.recipient_id,
                        message.content,
                        message.encrypted,
                        message.signature,
                        message.thread_id,
                        message.disappear_after_seconds,
                        message.disappears_at,
                        message.created_at,
                        message.updated_at,
                    ],
                )?;
            }
        }

        // Insert or update friend connections - only if incoming data is newer or doesn't exist
        for friend in &sync_data.friends {
            let existing_updated_at: Option<String> = conn
                .query_row(
                    "SELECT updated_at FROM p2p_connections WHERE id = ?1",
                    params![friend.id],
                    |row| row.get("updated_at"),
                )
                .ok();

            if existing_updated_at.is_none()
                || existing_updated_at.as_ref().unwrap() < &friend.updated_at
            {
                conn.execute(
                    "INSERT OR REPLACE INTO p2p_connections
                     (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        friend.id,
                        friend.user_id,
                        friend.friend_user_id,
                        friend.status,
                        friend.initiated_by,
                        friend.created_at,
                        friend.updated_at,
                    ],
                )?;
            }
        }

        Ok(())
    }

    /// Get the last sync timestamp for a specific table and device
    fn get_last_sync_timestamp(&self, device_id: &str, table_name: &str) -> SqliteResult<String> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT last_sync_timestamp FROM sync_state
             WHERE device_id = ?1 AND table_name = ?2",
        )?;

        let result: Result<String, _> = stmt.query_row(params![device_id, table_name], |row| {
            row.get("last_sync_timestamp")
        });

        // If no sync state exists, return a very old timestamp
        Ok(result.unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string()))
    }

    /// Update the last sync timestamp for a specific table and device
    pub fn update_sync_timestamp(&self, device_id: &str, table_name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO sync_state (device_id, table_name, last_sync_timestamp)
             VALUES (?1, ?2, ?3)",
            params![device_id, table_name, now],
        )?;

        Ok(())
    }

    /// Update all sync timestamps for a device after a successful sync
    pub fn update_all_sync_timestamps(&self, device_id: &str) -> SqliteResult<()> {
        self.update_sync_timestamp(device_id, "posts")?;
        self.update_sync_timestamp(device_id, "messages")?;
        self.update_sync_timestamp(device_id, "p2p_connections")?;
        Ok(())
    }

    /// Get sync status - returns count of items that need syncing
    pub fn get_sync_status(
        &self,
        device_id: &str,
        user_id: SqliteUuid,
    ) -> SqliteResult<(usize, usize, usize)> {
        let sync_data = self.get_sync_data(device_id, user_id)?;
        Ok((
            sync_data.posts.len(),
            sync_data.messages.len(),
            sync_data.friends.len(),
        ))
    }
}
