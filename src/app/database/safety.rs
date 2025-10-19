use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{BlockedUser, MutedUser, SqliteUuid};
use crate::app::Database;

impl Database {
    // ========== Blocking Functions ==========

    /// Block a user
    pub fn block_user(
        &self,
        blocker_id: SqliteUuid,
        blocked_id: SqliteUuid,
        reason: Option<String>,
    ) -> SqliteResult<BlockedUser> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let block_id = SqliteUuid::new();

        // Can't block yourself
        if blocker_id == blocked_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        conn.execute(
            "INSERT OR REPLACE INTO blocked_users (id, blocker_id, blocked_id, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![block_id, blocker_id, blocked_id, reason, &now],
        )?;

        Ok(BlockedUser {
            id: block_id,
            blocker_id,
            blocked_id,
            reason,
            created_at: now,
        })
    }

    /// Unblock a user
    pub fn unblock_user(&self, blocker_id: SqliteUuid, blocked_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM blocked_users WHERE blocker_id = ?1 AND blocked_id = ?2",
            params![blocker_id, blocked_id],
        )?;
        Ok(())
    }

    /// Check if a user is blocked
    pub fn is_user_blocked(
        &self,
        blocker_id: SqliteUuid,
        blocked_id: SqliteUuid,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM blocked_users WHERE blocker_id = ?1 AND blocked_id = ?2",
            params![blocker_id, blocked_id],
            |row| row.get("COUNT(*)"),
        )?;
        Ok(count > 0)
    }

    /// Get all users blocked by a user
    pub fn get_blocked_users(&self, blocker_id: SqliteUuid) -> SqliteResult<Vec<BlockedUser>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, blocker_id, blocked_id, reason, created_at
             FROM blocked_users
             WHERE blocker_id = ?1
             ORDER BY created_at DESC",
        )?;

        let blocked_iter = stmt.query_map([blocker_id], |row| {
            Ok(BlockedUser {
                id: row.get("id")?,
                blocker_id: row.get("blocker_id")?,
                blocked_id: row.get("blocked_id")?,
                reason: row.get("reason")?,
                created_at: row.get("created_at")?,
            })
        })?;

        let mut blocked_users = Vec::new();
        for blocked in blocked_iter {
            blocked_users.push(blocked?);
        }
        Ok(blocked_users)
    }

    /// Check if blocked in either direction (mutual block check)
    pub fn is_blocked_either_way(
        &self,
        user1_id: SqliteUuid,
        user2_id: SqliteUuid,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM blocked_users
             WHERE (blocker_id = ?1 AND blocked_id = ?2)
             OR (blocker_id = ?2 AND blocked_id = ?1)",
            params![user1_id, user2_id],
            |row| row.get("COUNT(*)"),
        )?;
        Ok(count > 0)
    }

    // ========== Muting Functions ==========

    /// Mute a user
    pub fn mute_user(
        &self,
        muter_id: SqliteUuid,
        muted_id: SqliteUuid,
        mute_notifications: bool,
        mute_messages: bool,
        mute_posts: bool,
        expires_at: Option<String>,
    ) -> SqliteResult<MutedUser> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let mute_id = SqliteUuid::new();

        // Can't mute yourself
        if muter_id == muted_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        conn.execute(
            "INSERT OR REPLACE INTO muted_users
             (id, muter_id, muted_id, mute_notifications, mute_messages, mute_posts, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                mute_id,
                muter_id,
                muted_id,
                mute_notifications,
                mute_messages,
                mute_posts,
                expires_at,
                &now
            ],
        )?;

        Ok(MutedUser {
            id: mute_id,
            muter_id,
            muted_id,
            mute_notifications,
            mute_messages,
            mute_posts,
            expires_at,
            created_at: now,
        })
    }

    /// Unmute a user
    pub fn unmute_user(&self, muter_id: SqliteUuid, muted_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM muted_users WHERE muter_id = ?1 AND muted_id = ?2",
            params![muter_id, muted_id],
        )?;
        Ok(())
    }

    /// Check if a user is muted
    pub fn is_user_muted(&self, muter_id: SqliteUuid, muted_id: SqliteUuid) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM muted_users
             WHERE muter_id = ?1 AND muted_id = ?2
             AND (expires_at IS NULL OR expires_at > ?3)",
            params![muter_id, muted_id, now],
            |row| row.get("COUNT(*)"),
        )?;
        Ok(count > 0)
    }

    /// Get mute settings for a specific user
    pub fn get_mute_settings(
        &self,
        muter_id: SqliteUuid,
        muted_id: SqliteUuid,
    ) -> SqliteResult<Option<MutedUser>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let result = conn.query_row(
            "SELECT id, muter_id, muted_id, mute_notifications, mute_messages, mute_posts, expires_at, created_at
             FROM muted_users
             WHERE muter_id = ?1 AND muted_id = ?2
             AND (expires_at IS NULL OR expires_at > ?3)",
            params![muter_id, muted_id, now],
            |row| {
                Ok(MutedUser {
                    id: row.get("id")?,
                    muter_id: row.get("muter_id")?,
                    muted_id: row.get("muted_id")?,
                    mute_notifications: row.get("mute_notifications")?,
                    mute_messages: row.get("mute_messages")?,
                    mute_posts: row.get("mute_posts")?,
                    expires_at: row.get("expires_at")?,
                    created_at: row.get("created_at")?,
                })
            },
        );

        match result {
            Ok(muted_user) => Ok(Some(muted_user)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get all users muted by a user
    pub fn get_muted_users(&self, muter_id: SqliteUuid) -> SqliteResult<Vec<MutedUser>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, muter_id, muted_id, mute_notifications, mute_messages, mute_posts, expires_at, created_at
             FROM muted_users
             WHERE muter_id = ?1
             AND (expires_at IS NULL OR expires_at > ?2)
             ORDER BY created_at DESC"
        )?;

        let muted_iter = stmt.query_map(params![muter_id, now], |row| {
            Ok(MutedUser {
                id: row.get("id")?,
                muter_id: row.get("muter_id")?,
                muted_id: row.get("muted_id")?,
                mute_notifications: row.get("mute_notifications")?,
                mute_messages: row.get("mute_messages")?,
                mute_posts: row.get("mute_posts")?,
                expires_at: row.get("expires_at")?,
                created_at: row.get("created_at")?,
            })
        })?;

        let mut muted_users = Vec::new();
        for muted in muted_iter {
            muted_users.push(muted?);
        }
        Ok(muted_users)
    }

    /// Clean up expired mutes
    pub fn cleanup_expired_mutes(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "DELETE FROM muted_users WHERE expires_at IS NOT NULL AND expires_at <= ?1",
            [now],
        )?;
        Ok(())
    }

    /// Update mute settings for a user
    pub fn update_mute_settings(
        &self,
        muter_id: SqliteUuid,
        muted_id: SqliteUuid,
        mute_notifications: bool,
        mute_messages: bool,
        mute_posts: bool,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE muted_users
             SET mute_notifications = ?1, mute_messages = ?2, mute_posts = ?3
             WHERE muter_id = ?4 AND muted_id = ?5",
            params![
                mute_notifications,
                mute_messages,
                mute_posts,
                muter_id,
                muted_id
            ],
        )?;
        Ok(())
    }
}
