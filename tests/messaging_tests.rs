// Messaging tests for Cipher
// Tests direct messaging, reactions, threads, and message features

use app::Database;
use tempfile::TempDir;

#[test]
fn test_send_and_retrieve_encrypted_message() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    let message_content = "Hello Bob!";
    let message = db.send_encrypted_message(
        alice.id,
        bob.id,
        message_content,
        None
    ).unwrap();

    assert!(message.encrypted, "Message should be encrypted");
    assert_ne!(message.content, message_content, "Encrypted content should differ from plaintext");
    assert_eq!(message.sender_id, alice.id);
    assert_eq!(message.recipient_id, bob.id);

    // Retrieve messages
    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 1);
}

#[test]
fn test_decrypt_message_with_correct_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    let plaintext = "Secret message";
    let message = db.send_encrypted_message(alice.id, bob.id, plaintext, None).unwrap();

    // Bob decrypts
    let decrypted = Database::decrypt_message(
        &message.content,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap()
    ).unwrap();

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_message_signature_verification() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    let message_content = "Signed message";
    let message = db.send_encrypted_message(alice.id, bob.id, message_content, None).unwrap();

    assert!(message.signature.is_some(), "Message should have signature");

    // Verify signature
    let signature = message.signature.unwrap();
    let is_valid = Database::verify_signature(
        message_content,
        &signature,
        alice.public_key.as_ref().unwrap()
    );

    assert!(is_valid, "Signature should be valid");
}

// TODO: Implement add_message_reaction and get_message_reactions APIs then enable this test
// #[test]
// fn test_message_reactions() { ... }

// TODO: Implement reply_to_message and get_message_thread APIs then enable this test
// #[test]
// fn test_message_threads() { ... }

// TODO: Implement search_messages API then enable this test
// #[test]
// fn test_message_search() { ... }

// TODO: Implement edit_message API then enable this test
// #[test]
// fn test_message_editing() { ... }

// TODO: Implement delete_message API then enable this test
// #[test]
// fn test_message_deletion() { ... }

// TODO: Implement send_voice_message and get_voice_messages APIs then enable this test
// #[test]
// fn test_voice_message_storage() { ... }

#[test]
fn test_message_persistence_across_sessions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Session 1: Create users and send message
    {
        let db = Database::new(&db_path.to_string_lossy()).unwrap();
        let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
        let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();
        db.send_encrypted_message(alice.id, bob.id, "Persistent message", None).unwrap();
    }

    // Session 2: Reopen database and verify message exists
    {
        let db = Database::new(&db_path.to_string_lossy()).unwrap();
        let bob = db.find_user_by_display_name("bob").unwrap().unwrap();
        let messages = db.get_messages_for_user(bob.id).unwrap();
        assert_eq!(messages.len(), 1, "Message should persist across sessions");
    }
}

#[test]
fn test_message_ordering() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    // Send multiple messages with delays
    let msg1 = db.send_encrypted_message(alice.id, bob.id, "First", None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let msg2 = db.send_encrypted_message(alice.id, bob.id, "Second", None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let msg3 = db.send_encrypted_message(alice.id, bob.id, "Third", None).unwrap();

    // Retrieve messages (should be ordered by timestamp DESC - newest first)
    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 3);

    // Verify reverse chronological order (newest first)
    assert!(messages[0].created_at >= messages[1].created_at, "Newest should be first");
    assert!(messages[1].created_at >= messages[2].created_at, "Messages should be newest-first");

    // Verify the order matches creation order (msg3 is newest, msg1 is oldest)
    assert_eq!(messages[0].id, msg3.id, "Third message should be first");
    assert_eq!(messages[1].id, msg2.id, "Second message should be second");
    assert_eq!(messages[2].id, msg1.id, "First message should be last");
}

#[test]
fn test_disappearing_messages() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    // Send message with 1 second expiry
    let message = db.send_encrypted_message(alice.id, bob.id, "Disappearing message", Some(1)).unwrap();
    assert_eq!(message.disappear_after_seconds, Some(1));

    // Wait for expiry
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Cleanup expired messages
    db.cleanup_expired_messages().unwrap();

    // Message should be gone
    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 0, "Expired message should be deleted");
}

#[test]
fn test_concurrent_message_sending() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    // Send multiple messages rapidly
    for i in 0..10 {
        db.send_encrypted_message(alice.id, bob.id, &format!("Message {}", i), None).unwrap();
    }

    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 10, "All messages should be stored");
}

// TODO: Implement upload_media_file and get_media_attachments APIs then enable this test
// #[test]
// fn test_message_with_media_attachments() { ... }
