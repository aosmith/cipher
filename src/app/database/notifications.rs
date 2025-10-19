use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{Notification, SqliteUuid};
use crate::app::Database;

impl Database {
    /// Create a new notification
    pub fn create_notification(
        &self,
        user_id: SqliteUuid,
        notification_type: &str,
        title: &str,
        message: &str,
        data: Option<String>,
    ) -> SqliteResult<Notification> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let notification_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO notifications (id, user_id, notification_type, title, message, data, read, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![notification_id, user_id, notification_type, title, message, data, &now],
        )?;

        Ok(Notification {
            id: notification_id,
            user_id,
            notification_type: notification_type.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            data,
            read: false,
            created_at: now,
        })
    }

    /// Get all notifications for a user
    pub fn get_notifications(&self, user_id: SqliteUuid) -> SqliteResult<Vec<Notification>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, notification_type, title, message, data, read, created_at
             FROM notifications
             WHERE user_id = ?1
             ORDER BY created_at DESC",
        )?;

        let notification_iter = stmt.query_map([user_id], |row| {
            Ok(Notification {
                id: row.get("id")?,
                user_id: row.get("user_id")?,
                notification_type: row.get("notification_type")?,
                title: row.get("title")?,
                message: row.get("message")?,
                data: row.get("data")?,
                read: row.get("read")?,
                created_at: row.get("created_at")?,
            })
        })?;

        let mut notifications = Vec::new();
        for notification in notification_iter {
            notifications.push(notification?);
        }
        Ok(notifications)
    }

    /// Get unread notifications for a user
    pub fn get_unread_notifications(&self, user_id: SqliteUuid) -> SqliteResult<Vec<Notification>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, notification_type, title, message, data, read, created_at
             FROM notifications
             WHERE user_id = ?1 AND read = 0
             ORDER BY created_at DESC",
        )?;

        let notification_iter = stmt.query_map([user_id], |row| {
            Ok(Notification {
                id: row.get("id")?,
                user_id: row.get("user_id")?,
                notification_type: row.get("notification_type")?,
                title: row.get("title")?,
                message: row.get("message")?,
                data: row.get("data")?,
                read: row.get("read")?,
                created_at: row.get("created_at")?,
            })
        })?;

        let mut notifications = Vec::new();
        for notification in notification_iter {
            notifications.push(notification?);
        }
        Ok(notifications)
    }

    /// Mark a notification as read
    pub fn mark_notification_read(
        &self,
        notification_id: SqliteUuid,
        user_id: SqliteUuid,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Verify ownership before marking as read
        let owner_id: SqliteUuid = conn.query_row(
            "SELECT user_id FROM notifications WHERE id = ?1",
            [notification_id],
            |row| row.get("user_id"),
        )?;

        if owner_id != user_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        conn.execute(
            "UPDATE notifications SET read = 1 WHERE id = ?1",
            [notification_id],
        )?;
        Ok(())
    }

    /// Mark all notifications as read for a user
    pub fn mark_all_notifications_read(&self, user_id: SqliteUuid) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE notifications SET read = 1 WHERE user_id = ?1 AND read = 0",
            [user_id],
        )?;
        Ok(())
    }

    /// Delete a notification
    pub fn delete_notification(
        &self,
        notification_id: SqliteUuid,
        user_id: SqliteUuid,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        // Verify ownership before deleting
        let owner_id: SqliteUuid = conn.query_row(
            "SELECT user_id FROM notifications WHERE id = ?1",
            [notification_id],
            |row| row.get("user_id"),
        )?;

        if owner_id != user_id {
            return Err(rusqlite::Error::InvalidQuery);
        }

        conn.execute("DELETE FROM notifications WHERE id = ?1", [notification_id])?;
        Ok(())
    }

    /// Get unread notification count for a user
    pub fn get_unread_notification_count(&self, user_id: SqliteUuid) -> SqliteResult<i32> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ?1 AND read = 0",
            [user_id],
            |row| row.get("COUNT(*)"),
        )?;
        Ok(count)
    }

    /// Delete old read notifications (older than specified days)
    pub fn cleanup_old_notifications(&self, days: i32) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);
        let cutoff_str = cutoff_date.to_rfc3339();

        conn.execute(
            "DELETE FROM notifications WHERE read = 1 AND created_at < ?1",
            [cutoff_str],
        )?;
        Ok(())
    }
}
