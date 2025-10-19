use app::Database;
use tempfile::TempDir;

#[test]
fn test_alice_bob_basic_messaging() {
    // Create two separate databases (simulating two devices)
    let alice_dir = TempDir::new().unwrap();
    let bob_dir = TempDir::new().unwrap();

    let alice_db_path = alice_dir.path().join("alice.db");
    let bob_db_path = bob_dir.path().join("bob.db");

    let alice_db = Database::new(&alice_db_path.to_string_lossy()).unwrap();
    let bob_db = Database::new(&bob_db_path.to_string_lossy()).unwrap();

    // Create Alice on her device
    let (alice, alice_recovery) = alice_db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Create Bob on his device
    let (bob, bob_recovery) = bob_db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Verify recovery phrases are 24 words
    assert_eq!(alice_recovery.split_whitespace().count(), 24);
    assert_eq!(bob_recovery.split_whitespace().count(), 24);

    // Alice learns about Bob (P2P discovery simulation)
    let bob_in_alice_db = alice_db
        .sync_peer_user(
            "bob",
            bob.public_key.as_ref().unwrap(),
            bob.encryption_public_key.as_ref().unwrap(),
        )
        .unwrap();

    // Bob learns about Alice (P2P discovery simulation)
    let alice_in_bob_db = bob_db
        .sync_peer_user(
            "alice",
            alice.public_key.as_ref().unwrap(),
            alice.encryption_public_key.as_ref().unwrap(),
        )
        .unwrap();

    // Establish friendship
    alice_db.add_friend(alice.id, bob_in_alice_db.id).unwrap();
    bob_db.add_friend(bob.id, alice_in_bob_db.id).unwrap();

    // Accept friendships (in production, this happens via P2P handshake)
    alice_db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![alice.id, bob_in_alice_db.id],
    ).unwrap();

    bob_db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![bob.id, alice_in_bob_db.id],
    ).unwrap();

    // Alice sends encrypted message to Bob
    let message_content = "Hello Bob! This is a secret message.";
    let alice_message = alice_db
        .send_encrypted_message(alice.id, bob_in_alice_db.id, message_content, None)
        .unwrap();

    assert!(alice_message.encrypted, "Message should be encrypted");
    assert_ne!(
        alice_message.content, message_content,
        "Encrypted content should not match plaintext"
    );

    // Simulate P2P sync: Bob receives the encrypted message
    let bob_messages = bob_db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(
        bob_messages.len(),
        0,
        "Bob shouldn't see Alice's local message yet"
    );

    // Manually sync the message to Bob's database (simulates P2P sync)
    bob_db.conn.lock().unwrap().execute(
        "INSERT INTO messages (id, sender_id, recipient_id, content, encrypted, signature, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            alice_message.id,
            alice_in_bob_db.id,
            bob.id,
            &alice_message.content,
            alice_message.encrypted,
            alice_message.signature,
            &alice_message.created_at,
            &alice_message.updated_at
        ],
    ).unwrap();

    // Bob decrypts the message
    let bob_messages = bob_db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(bob_messages.len(), 1);

    let received_message = &bob_messages[0];
    let decrypted = Database::decrypt_message(
        &received_message.content,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(decrypted, message_content);

    // Verify message signature
    if let Some(ref signature) = received_message.signature {
        let is_valid = Database::verify_signature(
            message_content,
            signature,
            alice.public_key.as_ref().unwrap(),
        );
        assert!(is_valid, "Message signature should be valid");
    }
}

#[test]
fn test_alice_bob_bidirectional_messaging() {
    // Create two separate databases
    let alice_dir = TempDir::new().unwrap();
    let bob_dir = TempDir::new().unwrap();

    let alice_db = Database::new(&alice_dir.path().join("alice.db").to_string_lossy()).unwrap();
    let bob_db = Database::new(&bob_dir.path().join("bob.db").to_string_lossy()).unwrap();

    // Create users
    let (alice, _) = alice_db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = bob_db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Sync peer users
    let bob_in_alice_db = alice_db
        .sync_peer_user(
            "bob",
            bob.public_key.as_ref().unwrap(),
            bob.encryption_public_key.as_ref().unwrap(),
        )
        .unwrap();

    let alice_in_bob_db = bob_db
        .sync_peer_user(
            "alice",
            alice.public_key.as_ref().unwrap(),
            alice.encryption_public_key.as_ref().unwrap(),
        )
        .unwrap();

    // Alice -> Bob
    let msg1 = alice_db
        .send_encrypted_message(alice.id, bob_in_alice_db.id, "Hi Bob!", None)
        .unwrap();

    // Bob -> Alice
    let msg2 = bob_db
        .send_encrypted_message(bob.id, alice_in_bob_db.id, "Hi Alice!", None)
        .unwrap();

    // Verify both messages are encrypted
    assert!(msg1.encrypted);
    assert!(msg2.encrypted);
    assert_ne!(msg1.content, "Hi Bob!");
    assert_ne!(msg2.content, "Hi Alice!");

    // Decrypt Bob's message
    let decrypted1 = Database::decrypt_message(
        &msg1.content,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!(decrypted1, "Hi Bob!");

    // Decrypt Alice's message
    let decrypted2 = Database::decrypt_message(
        &msg2.content,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!(decrypted2, "Hi Alice!");
}

#[test]
fn test_alice_bob_recovery_phrase_restore() {
    // Alice creates account on Device 1
    let device1_dir = TempDir::new().unwrap();
    let device1_db =
        Database::new(&device1_dir.path().join("device1.db").to_string_lossy()).unwrap();

    let (alice_original, recovery_phrase) = device1_db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice loses Device 1 and restores on Device 2 using recovery phrase
    let device2_dir = TempDir::new().unwrap();
    let device2_db =
        Database::new(&device2_dir.path().join("device2.db").to_string_lossy()).unwrap();

    let alice_restored = device2_db
        .restore_user_from_recovery_phrase(
            "alice".to_string(),
            recovery_phrase,
            Database::generate_device_id(),
        )
        .unwrap();

    // Keys should be identical (deterministic key derivation)
    assert_eq!(alice_original.public_key, alice_restored.public_key);
    assert_eq!(alice_original.private_key, alice_restored.private_key);
    assert_eq!(
        alice_original.encryption_public_key,
        alice_restored.encryption_public_key
    );
    assert_eq!(
        alice_original.encryption_private_key,
        alice_restored.encryption_private_key
    );

    // Bob creates account
    let bob_dir = TempDir::new().unwrap();
    let bob_db = Database::new(&bob_dir.path().join("bob.db").to_string_lossy()).unwrap();
    let (bob, _) = bob_db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Bob sends message to Alice (using her public key)
    let alice_in_bob_db = bob_db
        .sync_peer_user(
            "alice",
            alice_original.public_key.as_ref().unwrap(),
            alice_original.encryption_public_key.as_ref().unwrap(),
        )
        .unwrap();

    let message_content = "Welcome back, Alice!";
    let encrypted_message = bob_db
        .send_encrypted_message(bob.id, alice_in_bob_db.id, message_content, None)
        .unwrap();

    // Alice on Device 2 can decrypt the message (because she has the same keys)
    let decrypted = Database::decrypt_message(
        &encrypted_message.content,
        bob.encryption_public_key.as_ref().unwrap(),
        alice_restored.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(decrypted, message_content);
}

#[test]
fn test_alice_bob_message_cannot_be_decrypted_by_others() {
    // Create Alice, Bob, and Eve
    let alice_dir = TempDir::new().unwrap();
    let bob_dir = TempDir::new().unwrap();
    let eve_dir = TempDir::new().unwrap();

    let alice_db = Database::new(&alice_dir.path().join("alice.db").to_string_lossy()).unwrap();
    let bob_db = Database::new(&bob_dir.path().join("bob.db").to_string_lossy()).unwrap();
    let eve_db = Database::new(&eve_dir.path().join("eve.db").to_string_lossy()).unwrap();

    let (alice, _) = alice_db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = bob_db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();
    let (eve, _) = eve_db
        .create_user_first_launch("eve".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice syncs Bob
    let bob_in_alice_db = alice_db
        .sync_peer_user(
            "bob",
            bob.public_key.as_ref().unwrap(),
            bob.encryption_public_key.as_ref().unwrap(),
        )
        .unwrap();

    // Alice sends encrypted message to Bob
    let message_content = "Secret message for Bob only";
    let encrypted_message = alice_db
        .send_encrypted_message(alice.id, bob_in_alice_db.id, message_content, None)
        .unwrap();

    // Eve tries to decrypt the message (should fail)
    let eve_decrypt_attempt = Database::decrypt_message(
        &encrypted_message.content,
        alice.encryption_public_key.as_ref().unwrap(),
        eve.encryption_private_key.as_ref().unwrap(),
    );

    assert!(
        eve_decrypt_attempt.is_err(),
        "Eve should not be able to decrypt Alice->Bob message"
    );

    // Bob can decrypt it successfully
    let bob_decrypt = Database::decrypt_message(
        &encrypted_message.content,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(bob_decrypt, message_content);
}
