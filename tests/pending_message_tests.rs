// Tests for the offline message queue (src/app/database/pending_messages.rs).
// Exercises the full queue lifecycle: enqueue, retry accounting, send, and clear.

use app::Database;
use tempfile::TempDir;

fn db_with_user(name: &str) -> (Database, app::User) {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();
    std::mem::forget(temp_dir);
    let (user, _) = db
        .create_user_first_launch(name.to_string(), Database::generate_device_id())
        .unwrap();
    (db, user)
}

#[test]
fn test_queue_and_get_pending_message() {
    let (db, alice) = db_with_user("alice");

    let id = db
        .queue_pending_message(alice.id, "post", "{\"content\":\"hi\"}", 3)
        .unwrap();

    let pending = db.get_pending_messages(alice.id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, id);
    assert_eq!(pending[0].message_type, "post");
    assert_eq!(pending[0].content_json, "{\"content\":\"hi\"}");
    assert_eq!(pending[0].retry_count, 0);
    assert_eq!(pending[0].max_retries, 3);

    assert_eq!(db.get_pending_message_count(alice.id).unwrap(), 1);
}

#[test]
fn test_mark_message_sent_removes_it() {
    let (db, alice) = db_with_user("alice");
    let id = db
        .queue_pending_message(alice.id, "message", "payload", 3)
        .unwrap();

    db.mark_message_sent(id).unwrap();

    assert!(db.get_pending_messages(alice.id).unwrap().is_empty());
    assert_eq!(db.get_pending_message_count(alice.id).unwrap(), 0);
}

#[test]
fn test_increment_retry_count() {
    let (db, alice) = db_with_user("alice");
    let id = db
        .queue_pending_message(alice.id, "post", "payload", 3)
        .unwrap();

    db.increment_retry_count(id).unwrap();
    db.increment_retry_count(id).unwrap();

    let pending = db.get_pending_messages(alice.id).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].retry_count, 2);
    assert!(pending[0].last_attempt_at.is_some());
}

#[test]
fn test_retries_exhausted_excluded_from_pending() {
    let (db, alice) = db_with_user("alice");
    // max_retries = 1: after one retry, retry_count is no longer < max_retries.
    let id = db
        .queue_pending_message(alice.id, "post", "payload", 1)
        .unwrap();

    assert_eq!(db.get_pending_message_count(alice.id).unwrap(), 1);
    db.increment_retry_count(id).unwrap();

    // The message has hit its retry ceiling and is filtered out of the ready set.
    assert!(db.get_pending_messages(alice.id).unwrap().is_empty());
    assert_eq!(db.get_pending_message_count(alice.id).unwrap(), 0);
}

#[test]
fn test_remove_pending_message() {
    let (db, alice) = db_with_user("alice");
    let id = db
        .queue_pending_message(alice.id, "post", "payload", 3)
        .unwrap();

    db.remove_pending_message(id).unwrap();
    assert!(db.get_pending_messages(alice.id).unwrap().is_empty());
}

#[test]
fn test_clear_pending_messages() {
    let (db, alice) = db_with_user("alice");
    db.queue_pending_message(alice.id, "post", "a", 3).unwrap();
    db.queue_pending_message(alice.id, "post", "b", 3).unwrap();
    db.queue_pending_message(alice.id, "message", "c", 3)
        .unwrap();
    assert_eq!(db.get_pending_message_count(alice.id).unwrap(), 3);

    db.clear_pending_messages(alice.id).unwrap();
    assert!(db.get_pending_messages(alice.id).unwrap().is_empty());
}

#[test]
fn test_pending_messages_scoped_per_user() {
    let (db, alice) = db_with_user("alice");
    let bob = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap()
        .0;

    db.queue_pending_message(alice.id, "post", "alice-msg", 3)
        .unwrap();
    db.queue_pending_message(bob.id, "post", "bob-msg", 3)
        .unwrap();

    // Each user only sees their own queue.
    assert_eq!(db.get_pending_message_count(alice.id).unwrap(), 1);
    assert_eq!(db.get_pending_message_count(bob.id).unwrap(), 1);

    db.clear_pending_messages(alice.id).unwrap();
    assert_eq!(db.get_pending_message_count(alice.id).unwrap(), 0);
    assert_eq!(
        db.get_pending_message_count(bob.id).unwrap(),
        1,
        "bob's queue is unaffected"
    );
}
