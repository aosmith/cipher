use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{Post, PostComment, PostReaction, SqliteUuid};
use crate::app::Database;

impl Database {
    pub fn get_posts(&self, current_user_id: SqliteUuid) -> SqliteResult<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT p.id, p.user_id, u.display_name, p.content, p.encrypted, p.pinned, p.shared_post_id, p.share_comment, p.created_at, p.updated_at
             FROM posts p
             INNER JOIN users u ON p.user_id = u.id
             LEFT JOIN p2p_connections f1 ON p.user_id = f1.user_id AND f1.friend_user_id = ?1
             LEFT JOIN p2p_connections f2 ON p.user_id = f2.friend_user_id AND f2.user_id = ?1
             WHERE p.user_id = ?1 OR f1.friend_user_id = ?1 OR f2.user_id = ?1
             ORDER BY p.pinned DESC, p.created_at DESC"
        )?;

        let post_iter = stmt.query_map([current_user_id], |row| {
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
        })?;

        let mut posts = Vec::new();
        for post in post_iter {
            posts.push(post?);
        }
        Ok(posts)
    }

    pub fn create_post(
        &self,
        user_id: SqliteUuid,
        content: &str,
        encrypted: bool,
    ) -> SqliteResult<Post> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let post_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO posts (id, user_id, content, encrypted, pinned, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![post_id, user_id, content, encrypted, false, &now, &now],
        )?;

        // Get display name for the post author
        let display_name: Option<String> = conn
            .query_row(
                "SELECT display_name FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .ok();

        Ok(Post {
            id: post_id,
            user_id,
            display_name,
            content: content.to_string(),
            encrypted,
            pinned: false,
            shared_post_id: None,
            share_comment: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Share/repost an existing post
    pub fn share_post(
        &self,
        user_id: SqliteUuid,
        original_post_id: SqliteUuid,
        share_comment: Option<String>,
    ) -> SqliteResult<Post> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let post_id = SqliteUuid::new();

        // Get the original post to check if it exists
        let original_content: String = conn.query_row(
            "SELECT content FROM posts WHERE id = ?1",
            [original_post_id],
            |row| row.get("content"),
        )?;

        conn.execute(
            "INSERT INTO posts (id, user_id, content, encrypted, pinned, shared_post_id, share_comment, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![post_id, user_id, &original_content, false, false, original_post_id, share_comment, &now, &now],
        )?;

        // Get display name for the post author
        let display_name: Option<String> = conn
            .query_row(
                "SELECT display_name FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .ok();

        Ok(Post {
            id: post_id,
            user_id,
            display_name,
            content: original_content,
            encrypted: false,
            pinned: false,
            shared_post_id: Some(original_post_id),
            share_comment,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Get the original post that was shared
    pub fn get_shared_post(&self, post_id: SqliteUuid) -> SqliteResult<Option<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.user_id, u.display_name, p.content, p.encrypted, p.pinned, p.shared_post_id, p.share_comment, p.created_at, p.updated_at
             FROM posts p
             INNER JOIN users u ON p.user_id = u.id
             WHERE p.id = ?1"
        )?;

        let result = stmt.query_row([post_id], |row| {
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
        });

        match result {
            Ok(post) => Ok(Some(post)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get all shares of a specific post
    pub fn get_post_shares(&self, original_post_id: SqliteUuid) -> SqliteResult<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.user_id, u.display_name, p.content, p.encrypted, p.pinned, p.shared_post_id, p.share_comment, p.created_at, p.updated_at
             FROM posts p
             INNER JOIN users u ON p.user_id = u.id
             WHERE p.shared_post_id = ?1 ORDER BY p.created_at DESC"
        )?;

        let post_iter = stmt.query_map([original_post_id], |row| {
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
        })?;

        let mut posts = Vec::new();
        for post in post_iter {
            posts.push(post?);
        }
        Ok(posts)
    }

    /// Add a reaction to a post
    pub fn add_post_reaction(
        &self,
        post_id: SqliteUuid,
        user_id: SqliteUuid,
        emoji: &str,
    ) -> SqliteResult<PostReaction> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let reaction_id = SqliteUuid::new();

        conn.execute(
            "INSERT OR REPLACE INTO post_reactions (id, post_id, user_id, emoji, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![reaction_id, post_id, user_id, emoji, &now],
        )?;

        Ok(PostReaction {
            id: reaction_id,
            post_id,
            user_id,
            emoji: emoji.to_string(),
            created_at: now,
        })
    }

    /// Remove a reaction from a post
    pub fn remove_post_reaction(
        &self,
        post_id: SqliteUuid,
        user_id: SqliteUuid,
        emoji: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM post_reactions WHERE post_id = ?1 AND user_id = ?2 AND emoji = ?3",
            params![post_id, user_id, emoji],
        )?;
        Ok(())
    }

    /// Get all reactions for a post
    pub fn get_post_reactions(&self, post_id: SqliteUuid) -> SqliteResult<Vec<PostReaction>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, post_id, user_id, emoji, created_at FROM post_reactions WHERE post_id = ?1 ORDER BY created_at DESC"
        )?;

        let reaction_iter = stmt.query_map([post_id], |row| {
            Ok(PostReaction {
                id: row.get("id")?,
                post_id: row.get("post_id")?,
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

    /// Get reaction summary for a post (emoji with counts)
    pub fn get_post_reaction_summary(
        &self,
        post_id: SqliteUuid,
    ) -> SqliteResult<Vec<(String, i32)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT emoji, COUNT(*) as count FROM post_reactions WHERE post_id = ?1 GROUP BY emoji ORDER BY count DESC"
        )?;

        let summary_iter = stmt.query_map([post_id], |row| {
            Ok((row.get::<_, String>("emoji")?, row.get::<_, i32>("count")?))
        })?;

        let mut summary = Vec::new();
        for item in summary_iter {
            summary.push(item?);
        }
        Ok(summary)
    }

    /// Check if a user has reacted to a post with a specific emoji
    #[allow(dead_code)]
    pub fn has_user_reacted(
        &self,
        post_id: SqliteUuid,
        user_id: SqliteUuid,
        emoji: &str,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM post_reactions WHERE post_id = ?1 AND user_id = ?2 AND emoji = ?3",
            params![post_id, user_id, emoji],
            |row| row.get("COUNT(*)")
        )?;
        Ok(count > 0)
    }

    /// Add a comment to a post
    pub fn add_post_comment(
        &self,
        post_id: SqliteUuid,
        user_id: SqliteUuid,
        content: &str,
        parent_comment_id: Option<SqliteUuid>,
    ) -> SqliteResult<PostComment> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let comment_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO post_comments (id, post_id, user_id, content, parent_comment_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![comment_id, post_id, user_id, content, parent_comment_id, &now, &now],
        )?;

        Ok(PostComment {
            id: comment_id,
            post_id,
            user_id,
            content: content.to_string(),
            parent_comment_id,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Get all comments for a post (top-level only, for threading support)
    pub fn get_post_comments(&self, post_id: SqliteUuid) -> SqliteResult<Vec<PostComment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, post_id, user_id, content, parent_comment_id, created_at, updated_at
             FROM post_comments
             WHERE post_id = ?1 AND parent_comment_id IS NULL
             ORDER BY created_at ASC",
        )?;

        let comment_iter = stmt.query_map([post_id], |row| {
            Ok(PostComment {
                id: row.get("id")?,
                post_id: row.get("post_id")?,
                user_id: row.get("user_id")?,
                content: row.get("content")?,
                parent_comment_id: row.get("parent_comment_id")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        let mut comments = Vec::new();
        for comment in comment_iter {
            comments.push(comment?);
        }
        Ok(comments)
    }

    /// Get replies to a specific comment
    pub fn get_comment_replies(
        &self,
        parent_comment_id: SqliteUuid,
    ) -> SqliteResult<Vec<PostComment>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, post_id, user_id, content, parent_comment_id, created_at, updated_at
             FROM post_comments
             WHERE parent_comment_id = ?1
             ORDER BY created_at ASC",
        )?;

        let comment_iter = stmt.query_map([parent_comment_id], |row| {
            Ok(PostComment {
                id: row.get("id")?,
                post_id: row.get("post_id")?,
                user_id: row.get("user_id")?,
                content: row.get("content")?,
                parent_comment_id: row.get("parent_comment_id")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        let mut comments = Vec::new();
        for comment in comment_iter {
            comments.push(comment?);
        }
        Ok(comments)
    }

    /// Delete a comment
    pub fn delete_post_comment(
        &self,
        comment_id: SqliteUuid,
        user_id: SqliteUuid,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Verify ownership
        let owner_id: SqliteUuid = conn.query_row(
            "SELECT user_id FROM post_comments WHERE id = ?1",
            [comment_id],
            |row| row.get("user_id"),
        )?;

        if owner_id != user_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        conn.execute("DELETE FROM post_comments WHERE id = ?1", [comment_id])?;
        Ok(())
    }

    /// Get comment count for a post
    pub fn get_post_comment_count(&self, post_id: SqliteUuid) -> SqliteResult<i32> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM post_comments WHERE post_id = ?1",
            [post_id],
            |row| row.get("COUNT(*)"),
        )?;
        Ok(count)
    }

    /// Edit a post (only owner can edit)
    pub fn edit_post(
        &self,
        post_id: SqliteUuid,
        user_id: SqliteUuid,
        new_content: &str,
    ) -> SqliteResult<Post> {
        let conn = self.conn.lock().unwrap();

        // Verify ownership
        let owner_id: SqliteUuid = conn.query_row(
            "SELECT user_id FROM posts WHERE id = ?1",
            [post_id],
            |row| row.get("user_id"),
        )?;

        if owner_id != user_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE posts SET content = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_content, &now, post_id],
        )?;

        // Return updated post
        let mut stmt = conn.prepare(
            "SELECT p.id, p.user_id, u.display_name, p.content, p.encrypted, p.pinned, p.shared_post_id, p.share_comment, p.created_at, p.updated_at
             FROM posts p
             INNER JOIN users u ON p.user_id = u.id
             WHERE p.id = ?1"
        )?;

        stmt.query_row([post_id], |row| {
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
        })
    }

    /// Delete a post (only owner can delete, cascade deletes handled by database)
    pub fn delete_post(&self, post_id: SqliteUuid, user_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Verify ownership
        let owner_id: SqliteUuid = conn.query_row(
            "SELECT user_id FROM posts WHERE id = ?1",
            [post_id],
            |row| row.get("user_id"),
        )?;

        if owner_id != user_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        // Delete the post (cascade deletes will handle reactions, comments, and media)
        conn.execute("DELETE FROM posts WHERE id = ?1", [post_id])?;
        Ok(())
    }
}
