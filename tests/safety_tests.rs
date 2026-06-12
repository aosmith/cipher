// Safety and validation tests for Cipher
// Tests blocking, muting, input validation, and security measures

use app::Database;
use chrono::Utc;
use tempfile::TempDir;

#[test]
fn test_block_user() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice blocks Bob
    db.block_user(alice.id, bob.id, None).unwrap();

    // Verify block
    let is_blocked = db.is_user_blocked(alice.id, bob.id).unwrap();
    assert!(is_blocked, "Bob should be blocked by Alice");

    // Get blocked users list
    let blocked = db.get_blocked_users(alice.id).unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].blocked_id, bob.id);
}

#[test]
fn test_unblock_user() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Block then unblock
    db.block_user(alice.id, bob.id, None).unwrap();
    db.unblock_user(alice.id, bob.id).unwrap();

    // Verify unblock
    let is_blocked = db.is_user_blocked(alice.id, bob.id).unwrap();
    assert!(!is_blocked, "Bob should no longer be blocked");
}

#[test]
fn test_bidirectional_block_check() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Bob blocks Alice
    db.block_user(bob.id, alice.id, None).unwrap();

    // Check if either has blocked the other
    let is_blocked_either_way = db.is_blocked_either_way(alice.id, bob.id).unwrap();
    assert!(
        is_blocked_either_way,
        "Should detect block in either direction"
    );
}

#[test]
fn test_mute_user() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Mute Bob (all types, with expiry)
    let expires_at = Some((chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339());
    db.mute_user(alice.id, bob.id, true, true, true, expires_at)
        .unwrap();

    // Verify mute
    let is_muted = db.is_user_muted(alice.id, bob.id).unwrap();
    assert!(is_muted, "Bob should be muted");

    // Get muted users
    let muted = db.get_muted_users(alice.id).unwrap();
    assert_eq!(muted.len(), 1);
}

#[test]
fn test_mute_user_permanent() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Mute permanently (no duration)
    db.mute_user(alice.id, bob.id, true, true, true, None)
        .unwrap();

    let is_muted = db.is_user_muted(alice.id, bob.id).unwrap();
    assert!(is_muted, "Bob should be permanently muted");
}

#[test]
fn test_unmute_user() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Mute then unmute
    db.mute_user(alice.id, bob.id, true, true, true, None)
        .unwrap();
    db.unmute_user(alice.id, bob.id).unwrap();

    let is_muted = db.is_user_muted(alice.id, bob.id).unwrap();
    assert!(!is_muted, "Bob should be unmuted");
}

#[test]
fn test_cleanup_expired_mutes() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Mute for 1 second
    let expires_at = Some((Utc::now() + chrono::Duration::seconds(1)).to_rfc3339());
    db.mute_user(alice.id, bob.id, true, true, true, expires_at)
        .unwrap();

    // Wait for expiry
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Cleanup
    db.cleanup_expired_mutes().unwrap();

    // Should no longer be muted
    let is_muted = db.is_user_muted(alice.id, bob.id).unwrap();
    assert!(!is_muted, "Expired mute should be cleaned up");
}

#[test]
fn test_mute_settings() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Mute Bob
    let expires_at = Some((Utc::now() + chrono::Duration::hours(2)).to_rfc3339());
    db.mute_user(alice.id, bob.id, true, true, true, expires_at)
        .unwrap();

    // Get mute settings
    let settings = db.get_mute_settings(alice.id, bob.id).unwrap();
    assert!(settings.is_some());

    let mute_info = settings.unwrap();
    assert_eq!(mute_info.muted_id, bob.id);
}

#[test]
fn test_update_mute_settings() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Mute temporarily
    let expires_at = Some((Utc::now() + chrono::Duration::hours(1)).to_rfc3339());
    db.mute_user(alice.id, bob.id, true, true, true, expires_at)
        .unwrap();

    // Update mute settings
    db.update_mute_settings(alice.id, bob.id, true, true, true)
        .unwrap();

    let settings = db.get_mute_settings(alice.id, bob.id).unwrap().unwrap();
    assert!(settings.mute_messages, "Should have messages muted");
}

#[test]
fn test_username_validation() {
    // Usernames should follow certain rules (e.g., no special characters)
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    // Valid username
    let result1 =
        db.create_user_first_launch("validuser123".to_string(), Database::generate_device_id());
    assert!(result1.is_ok(), "Valid username should be accepted");

    // Empty username (should be handled gracefully)
    let result2 = db.create_user_first_launch("".to_string(), Database::generate_device_id());
    // Implementation might allow or reject - just verify it doesn't crash
    let _ = result2;
}

#[test]
fn test_message_content_length() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Normal message
    let result1 = db.send_encrypted_message(alice.id, bob.id, "Normal message", None);
    assert!(result1.is_ok());

    // Very long message (10KB)
    let long_message = "A".repeat(10000);
    let result2 = db.send_encrypted_message(alice.id, bob.id, &long_message, None);
    // Should either succeed or fail gracefully
    let _ = result2;
}

#[test]
fn test_post_content_length() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Normal post
    let result1 = db.create_post(user.id, "Normal post", false);
    assert!(result1.is_ok());

    // Very long post
    let long_post = "Content ".repeat(1000);
    let result2 = db.create_post(user.id, &long_post, false);
    // Should either succeed or fail gracefully
    let _ = result2;
}

#[test]
fn test_sql_injection_prevention() {
    // Verify that prepared statements prevent SQL injection
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Try SQL injection in message content
    let injection_attempt = "'; DROP TABLE users; --";
    let result = db.create_post(alice.id, injection_attempt, false);

    // Should succeed (treated as literal string)
    assert!(
        result.is_ok(),
        "Prepared statements should prevent SQL injection"
    );

    // Verify database still intact
    let user_check = db.find_user_by_id(alice.id);
    assert!(user_check.is_ok(), "Database should not be corrupted");
}

#[test]
fn test_notification_creation() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Create notification
    let notification = db
        .create_notification(
            user.id,
            "friend_request",
            "New Friend Request",
            "You have a new friend request",
            None,
        )
        .unwrap();

    assert_eq!(notification.user_id, user.id);
    assert_eq!(notification.notification_type, "friend_request");

    // Get notifications
    let notifications = db.get_notifications(user.id).unwrap();
    assert_eq!(notifications.len(), 1);

    // Get unread count
    let unread_count = db.get_unread_notification_count(user.id).unwrap();
    assert_eq!(unread_count, 1);
}

#[test]
fn test_mark_notification_as_read() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    let notification = db
        .create_notification(user.id, "message", "New Message", "New message", None)
        .unwrap();

    // Mark as read
    db.mark_notification_read(notification.id, user.id).unwrap();

    // Verify read status
    let unread_count = db.get_unread_notification_count(user.id).unwrap();
    assert_eq!(unread_count, 0);
}

#[test]
fn test_mark_all_notifications_read() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Create multiple notifications
    for i in 0..5 {
        db.create_notification(
            user.id,
            "message",
            &format!("Title {}", i),
            &format!("Message {}", i),
            None,
        )
        .unwrap();
    }

    // Mark all as read
    db.mark_all_notifications_read(user.id).unwrap();

    // Verify
    let unread_count = db.get_unread_notification_count(user.id).unwrap();
    assert_eq!(unread_count, 0);
}

#[test]
fn test_delete_notification() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    let notification = db
        .create_notification(user.id, "test", "Test Title", "Test notification", None)
        .unwrap();

    // Delete
    db.delete_notification(notification.id, user.id).unwrap();

    // Verify deletion
    let notifications = db.get_notifications(user.id).unwrap();
    assert_eq!(notifications.len(), 0);
}

#[test]
fn test_cleanup_old_notifications() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Create notification
    db.create_notification(user.id, "old", "Old Title", "Old notification", None)
        .unwrap();

    // In a real scenario, we'd wait or manipulate timestamps
    // For now, just verify the cleanup method doesn't crash
    let result = db.cleanup_old_notifications(1); // 1 day threshold
    assert!(result.is_ok(), "Cleanup should not fail");
}

#[test]
fn test_block_self_prevention() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Try to block self - should fail
    let result = db.block_user(user.id, user.id, None);
    assert!(result.is_err(), "Should not be able to block yourself");
}

#[test]
fn test_concurrent_block_and_unblock() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Rapid block/unblock
    for _ in 0..10 {
        db.block_user(alice.id, bob.id, None).unwrap();
        db.unblock_user(alice.id, bob.id).unwrap();
    }

    // Should end in unblocked state
    let is_blocked = db.is_user_blocked(alice.id, bob.id).unwrap();
    assert!(!is_blocked);
}

#[test]
fn test_empty_blocked_users_list() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Get blocked users (should be empty)
    let blocked = db.get_blocked_users(user.id).unwrap();
    assert_eq!(blocked.len(), 0);
}

#[test]
fn test_empty_muted_users_list() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Get muted users (should be empty)
    let muted = db.get_muted_users(user.id).unwrap();
    assert_eq!(muted.len(), 0);
}
