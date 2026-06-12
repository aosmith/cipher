// Messaging tests for Cipher
// Tests direct messaging, reactions, threads, and message features

use app::Database;
use tempfile::TempDir;

#[test]
fn test_send_and_retrieve_encrypted_message() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    let message_content = "Hello Bob!";
    let message = db
        .send_encrypted_message(alice.id, bob.id, message_content, None)
        .unwrap();

    assert!(message.encrypted, "Message should be encrypted");
    assert_ne!(
        message.content, message_content,
        "Encrypted content should differ from plaintext"
    );
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

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    let plaintext = "Secret message";
    let message = db
        .send_encrypted_message(alice.id, bob.id, plaintext, None)
        .unwrap();

    // Bob decrypts
    let decrypted = Database::decrypt_message(
        &message.content,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_message_signature_verification() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    let message_content = "Signed message";
    let message = db
        .send_encrypted_message(alice.id, bob.id, message_content, None)
        .unwrap();

    assert!(message.signature.is_some(), "Message should have signature");

    // Verify signature
    let signature = message.signature.unwrap();
    let is_valid = Database::verify_signature(
        message_content,
        &signature,
        alice.public_key.as_ref().unwrap(),
    );

    assert!(is_valid, "Signature should be valid");
}

#[test]
fn test_message_reactions() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice sends a message
    let message = db
        .send_encrypted_message(alice.id, bob.id, "Hello!", None)
        .unwrap();

    // Bob reacts to the message
    let reaction = db.add_message_reaction(message.id, bob.id, "👍").unwrap();
    assert_eq!(reaction.emoji, "👍");
    assert_eq!(reaction.user_id, bob.id);

    // Get reactions
    let reactions = db.get_message_reactions(message.id).unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");

    // Alice also reacts
    db.add_message_reaction(message.id, alice.id, "❤️").unwrap();
    let reactions = db.get_message_reactions(message.id).unwrap();
    assert_eq!(reactions.len(), 2);

    // Remove Bob's reaction
    db.remove_message_reaction(message.id, bob.id, "👍")
        .unwrap();
    let reactions = db.get_message_reactions(message.id).unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "❤️");
}

#[test]
fn test_message_threads() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice sends original message
    let original = db
        .send_encrypted_message(alice.id, bob.id, "Original message", None)
        .unwrap();

    // Bob replies to it
    let reply1 = db
        .reply_to_message(bob.id, alice.id, "Reply 1", original.id, None)
        .unwrap();
    assert_eq!(reply1.thread_id, Some(original.id));

    // Alice replies to thread
    let reply2 = db
        .reply_to_message(alice.id, bob.id, "Reply 2", original.id, None)
        .unwrap();
    assert_eq!(reply2.thread_id, Some(original.id));

    // Get the thread
    let thread = db.get_message_thread(original.id).unwrap();
    assert_eq!(thread.len(), 3, "Thread should have original + 2 replies");
    assert_eq!(
        thread[0].id, original.id,
        "First message should be original"
    );
}

// Note: search_messages only works on unencrypted messages
// Since all messages in Cipher are encrypted, this test is limited
#[test]
fn test_message_search() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // All messages are encrypted by default in Cipher, so search won't find them
    db.send_encrypted_message(alice.id, bob.id, "Hello world", None)
        .unwrap();
    db.send_encrypted_message(alice.id, bob.id, "Goodbye world", None)
        .unwrap();

    // Search should return empty for encrypted messages
    let results = db.search_messages(alice.id, "world").unwrap();
    assert_eq!(
        results.len(),
        0,
        "Encrypted messages shouldn't be searchable by plaintext"
    );
}

#[test]
fn test_message_editing() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice sends a message
    let message = db
        .send_encrypted_message(alice.id, bob.id, "Original", None)
        .unwrap();
    let original_content = message.content.clone();

    // Alice edits it
    let edited = db
        .edit_message(message.id, alice.id, "Edited content")
        .unwrap();
    assert_ne!(
        edited.content, original_content,
        "Content should change after edit"
    );
    assert!(edited.edited_at.is_some(), "edited_at should be set");

    // Bob tries to edit - should fail
    let result = db.edit_message(message.id, bob.id, "Hacked!");
    assert!(result.is_err(), "Non-sender should not be able to edit");
}

#[test]
fn test_message_deletion() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();
    let (charlie, _) = db
        .create_user_first_launch("charlie".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice sends a message to Bob
    let message = db
        .send_encrypted_message(alice.id, bob.id, "Delete me", None)
        .unwrap();

    // Verify message exists
    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 1);

    // Charlie (not involved) tries to delete - should fail
    let result = db.delete_message(message.id, charlie.id);
    assert!(
        result.is_err(),
        "Uninvolved user should not be able to delete"
    );

    // Bob (recipient) can delete
    db.delete_message(message.id, bob.id).unwrap();

    // Message should be gone
    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 0);
}

// Voice messages are not yet implemented in Cipher.
// When implemented, they would likely use the existing media attachment system
// with audio/* MIME types and duration metadata.

#[test]
fn test_message_persistence_across_sessions() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Session 1: Create users and send message
    {
        let db = Database::new(&db_path.to_string_lossy()).unwrap();
        let (alice, _) = db
            .create_user_first_launch("alice".to_string(), Database::generate_device_id())
            .unwrap();
        let (bob, _) = db
            .create_user_first_launch("bob".to_string(), Database::generate_device_id())
            .unwrap();
        db.send_encrypted_message(alice.id, bob.id, "Persistent message", None)
            .unwrap();
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

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Send multiple messages with delays
    let msg1 = db
        .send_encrypted_message(alice.id, bob.id, "First", None)
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let msg2 = db
        .send_encrypted_message(alice.id, bob.id, "Second", None)
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let msg3 = db
        .send_encrypted_message(alice.id, bob.id, "Third", None)
        .unwrap();

    // Retrieve messages (should be ordered by timestamp DESC - newest first)
    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 3);

    // Verify reverse chronological order (newest first)
    assert!(
        messages[0].created_at >= messages[1].created_at,
        "Newest should be first"
    );
    assert!(
        messages[1].created_at >= messages[2].created_at,
        "Messages should be newest-first"
    );

    // Verify the order matches creation order (msg3 is newest, msg1 is oldest)
    assert_eq!(messages[0].id, msg3.id, "Third message should be first");
    assert_eq!(messages[1].id, msg2.id, "Second message should be second");
    assert_eq!(messages[2].id, msg1.id, "First message should be last");
}

#[test]
fn test_disappearing_messages() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Send message with 1 second expiry
    let message = db
        .send_encrypted_message(alice.id, bob.id, "Disappearing message", Some(1))
        .unwrap();
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

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Send multiple messages rapidly
    for i in 0..10 {
        db.send_encrypted_message(alice.id, bob.id, &format!("Message {}", i), None)
            .unwrap();
    }

    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 10, "All messages should be stored");
}

// upload_media_file and get_media_attachments exist as Tauri commands in src/app.rs
// Media attachments are tied to posts (media_attachments.post_id), not messages.
// Message attachments would require extending the schema to support message_id.
// See test_post_with_media_attachment in feed_tests.rs for post media testing.
