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
    pub comments: Vec<CommentSync>,
    pub reactions: Vec<ReactionSync>,
}

impl SyncData {
    fn empty() -> Self {
        SyncData {
            posts: Vec::new(),
            messages: Vec::new(),
            friends: Vec::new(),
            comments: Vec::new(),
            reactions: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.posts.is_empty()
            && self.messages.is_empty()
            && self.friends.is_empty()
            && self.comments.is_empty()
            && self.reactions.is_empty()
    }

    /// Partition this delta into independent, self-contained SyncData chunks
    /// whose serialized-JSON size stays under `budget_bytes`. A single sealed
    /// envelope carrying the whole delta silently exceeded the gossip message
    /// cap and became undeliverable; apply_sync_data is idempotent and
    /// order-independent, so each chunk can be sent (and applied) on its own.
    /// A single row larger than the budget is still sent in its own chunk,
    /// with a loud log - the publish-path size check gives it a distinct error
    /// if it really is undeliverable.
    pub fn into_chunks(self, budget_bytes: usize) -> Vec<SyncData> {
        let SyncData {
            posts,
            messages,
            friends,
            comments,
            reactions,
        } = self;

        let mut chunks: Vec<SyncData> = Vec::new();
        let mut current = SyncData::empty();
        let mut current_size = 0usize;

        macro_rules! pack {
            ($rows:expr, $field:ident) => {
                for row in $rows {
                    // Serialization of a plain data row can't realistically
                    // fail; treat a failure as oversized so it's logged.
                    let row_size = serde_json::to_string(&row)
                        .map(|s| s.len())
                        .unwrap_or(budget_bytes + 1);
                    if row_size > budget_bytes {
                        eprintln!(
                            "[SYNC] Oversized {} row: {} bytes exceeds the {}-byte chunk budget - sending it in its own chunk (it may exceed the gossip size cap)",
                            stringify!($field),
                            row_size,
                            budget_bytes
                        );
                    }
                    if current_size + row_size > budget_bytes && !current.is_empty() {
                        chunks.push(std::mem::replace(&mut current, SyncData::empty()));
                        current_size = 0;
                    }
                    current.$field.push(row);
                    current_size += row_size;
                }
            };
        }

        pack!(posts, posts);
        pack!(messages, messages);
        pack!(friends, friends);
        pack!(comments, comments);
        pack!(reactions, reactions);

        if !current.is_empty() {
            chunks.push(current);
        }
        chunks
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentSync {
    pub id: SqliteUuid,
    pub post_id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub content: String,
    pub parent_comment_id: Option<SqliteUuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionSync {
    pub id: SqliteUuid,
    pub post_id: SqliteUuid,
    pub user_id: SqliteUuid,
    pub emoji: String,
    pub created_at: String,
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
    /// Get data created/updated after `since_rfc3339`, for syncing to another
    /// of the user's devices. STATELESS RESPONDER: the cutoff comes from the
    /// REQUESTER (its own watermark, minus an overlap window) - the responder
    /// keeps no per-device cursor. The old design gated this on the
    /// responder's own sync_state cursors, which the responder then advanced
    /// whenever it APPLIED an incoming sync - so after device A applied B's
    /// data, A's pre-existing rows were never sent to B.
    pub fn get_sync_data(
        &self,
        user_id: SqliteUuid,
        since_rfc3339: &str,
    ) -> SqliteResult<SyncData> {
        let conn = self.conn.lock().unwrap();

        let last_post_sync = since_rfc3339;
        let last_message_sync = since_rfc3339;
        let last_friend_sync = since_rfc3339;

        // Get posts created/updated since the requester's watermark
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

        // Get comments created/updated since last sync (on user's posts or by user)
        let last_comment_sync = since_rfc3339;
        let mut comment_stmt = conn.prepare(
            "SELECT c.id, c.post_id, c.user_id, c.content, c.parent_comment_id, c.created_at, c.updated_at
             FROM post_comments c
             INNER JOIN posts p ON c.post_id = p.id
             WHERE (c.user_id = ?1 OR p.user_id = ?1) AND (c.updated_at > ?2 OR c.created_at > ?2)
             ORDER BY c.updated_at ASC"
        )?;

        let comments = comment_stmt
            .query_map(params![user_id, last_comment_sync], |row| {
                Ok(CommentSync {
                    id: row.get("id")?,
                    post_id: row.get("post_id")?,
                    user_id: row.get("user_id")?,
                    content: row.get("content")?,
                    parent_comment_id: row.get("parent_comment_id")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Get reactions created since last sync (on user's posts or by user)
        let last_reaction_sync = since_rfc3339;
        let mut reaction_stmt = conn.prepare(
            "SELECT r.id, r.post_id, r.user_id, r.emoji, r.created_at
             FROM post_reactions r
             INNER JOIN posts p ON r.post_id = p.id
             WHERE (r.user_id = ?1 OR p.user_id = ?1) AND r.created_at > ?2
             ORDER BY r.created_at ASC",
        )?;

        let reactions = reaction_stmt
            .query_map(params![user_id, last_reaction_sync], |row| {
                Ok(ReactionSync {
                    id: row.get("id")?,
                    post_id: row.get("post_id")?,
                    user_id: row.get("user_id")?,
                    emoji: row.get("emoji")?,
                    created_at: row.get("created_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SyncData {
            posts,
            messages,
            friends,
            comments,
            reactions,
        })
    }

    /// Apply synced data from another device with timestamp-based conflict resolution.
    /// Runs as a single transaction (BEGIN IMMEDIATE): a mid-loop error rolls
    /// everything back instead of committing a partial, visible sync.
    pub fn apply_sync_data(&self, sync_data: &SyncData) -> SqliteResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        // Insert or update posts - only if incoming data is newer or doesn't exist
        for post in &sync_data.posts {
            // Check if post exists and get its updated_at timestamp
            let existing_updated_at: Option<String> = tx
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
                tx.execute(
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
            let existing_updated_at: Option<String> = tx
                .query_row(
                    "SELECT updated_at FROM messages WHERE id = ?1",
                    params![message.id],
                    |row| row.get("updated_at"),
                )
                .ok();

            if existing_updated_at.is_none()
                || existing_updated_at.as_ref().unwrap() < &message.updated_at
            {
                tx.execute(
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
            let existing_updated_at: Option<String> = tx
                .query_row(
                    "SELECT updated_at FROM p2p_connections WHERE id = ?1",
                    params![friend.id],
                    |row| row.get("updated_at"),
                )
                .ok();

            if existing_updated_at.is_none()
                || existing_updated_at.as_ref().unwrap() < &friend.updated_at
            {
                tx.execute(
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

        // Insert or update comments - only if incoming data is newer or doesn't exist
        for comment in &sync_data.comments {
            let existing_updated_at: Option<String> = tx
                .query_row(
                    "SELECT updated_at FROM post_comments WHERE id = ?1",
                    params![comment.id],
                    |row| row.get("updated_at"),
                )
                .ok();

            if existing_updated_at.is_none()
                || existing_updated_at.as_ref().unwrap() < &comment.updated_at
            {
                tx.execute(
                    "INSERT OR REPLACE INTO post_comments
                     (id, post_id, user_id, content, parent_comment_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        comment.id,
                        comment.post_id,
                        comment.user_id,
                        comment.content,
                        comment.parent_comment_id,
                        comment.created_at,
                        comment.updated_at,
                    ],
                )?;
            }
        }

        // Insert reactions - use INSERT OR IGNORE since reactions don't have updated_at
        for reaction in &sync_data.reactions {
            tx.execute(
                "INSERT OR IGNORE INTO post_reactions
                 (id, post_id, user_id, emoji, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    reaction.id,
                    reaction.post_id,
                    reaction.user_id,
                    reaction.emoji,
                    reaction.created_at,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// The newest row timestamp this device holds across the device-synced
    /// tables, as unix seconds (0 if we hold nothing). The REQUESTER computes
    /// this and puts it (minus an overlap window for clock skew) in its
    /// DeviceSyncRequest, so responders only send what we might be missing.
    /// This replaces the per-device sync_state cursors, which corrupted the
    /// protocol: applying an incoming sync advanced the cursor that gated
    /// what we SENT.
    pub fn get_local_sync_watermark(&self, user_id: SqliteUuid) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();

        let queries: [&str; 5] = [
            "SELECT MAX(MAX(created_at), MAX(updated_at)) FROM posts WHERE user_id = ?1",
            "SELECT MAX(MAX(created_at), MAX(updated_at)) FROM messages
             WHERE sender_id = ?1 OR recipient_id = ?1",
            "SELECT MAX(MAX(created_at), MAX(updated_at)) FROM p2p_connections WHERE user_id = ?1",
            "SELECT MAX(MAX(c.created_at), MAX(c.updated_at)) FROM post_comments c
             INNER JOIN posts p ON c.post_id = p.id
             WHERE c.user_id = ?1 OR p.user_id = ?1",
            "SELECT MAX(r.created_at) FROM post_reactions r
             INNER JOIN posts p ON r.post_id = p.id
             WHERE r.user_id = ?1 OR p.user_id = ?1",
        ];

        let mut newest: i64 = 0;
        for sql in queries {
            let ts: Option<String> = conn.query_row(sql, params![user_id], |row| row.get(0))?;
            if let Some(ts) = ts {
                // Timestamps are written as RFC3339 throughout the codebase
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&ts) {
                    newest = newest.max(dt.timestamp());
                }
            }
        }
        Ok(newest)
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
        self.update_sync_timestamp(device_id, "post_comments")?;
        self.update_sync_timestamp(device_id, "post_reactions")?;
        Ok(())
    }

    /// Get sync status - returns count of items newer than `since_rfc3339`
    pub fn get_sync_status(
        &self,
        user_id: SqliteUuid,
        since_rfc3339: &str,
    ) -> SqliteResult<(usize, usize, usize, usize, usize)> {
        let sync_data = self.get_sync_data(user_id, since_rfc3339)?;
        Ok((
            sync_data.posts.len(),
            sync_data.messages.len(),
            sync_data.friends.len(),
            sync_data.comments.len(),
            sync_data.reactions.len(),
        ))
    }
}

impl Database {
    /// Read-only replay check: has this sealed-envelope message_id already
    /// been recorded as processed? Unlike mark_envelope_seen this NEVER
    /// inserts - envelopes are only marked seen after their handler succeeds
    /// (or when they aren't addressed to us), so a transient handler failure
    /// doesn't permanently discard the envelope.
    pub fn is_envelope_seen(&self, message_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let seen = conn
            .query_row(
                "SELECT 1 FROM seen_envelopes WHERE message_id = ?1",
                params![message_id],
                |_| Ok(()),
            )
            .is_ok();
        Ok(seen)
    }

    /// Record a sealed-envelope message_id for replay protection.
    /// Returns Ok(true) the first time an id is seen, Ok(false) on a replay.
    pub fn mark_envelope_seen(&self, message_id: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp();
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO seen_envelopes (message_id, seen_at) VALUES (?1, ?2)",
            params![message_id, now],
        )?;
        if inserted > 0 {
            // Opportunistic prune: ids older than the 7-day envelope staleness
            // window can never replay successfully, so they're dead weight
            let _ = conn.execute(
                "DELETE FROM seen_envelopes WHERE seen_at < ?1",
                params![now - 8 * 24 * 3600],
            );
        }
        Ok(inserted > 0)
    }
}
