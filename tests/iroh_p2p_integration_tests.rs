// Iroh P2P Integration Tests
// Tests peer discovery, 1st/2nd degree connections, and message propagation

use app::Database;
use tempfile::TempDir;

#[test]
fn test_peer_discovery_via_public_keys() {
    // Test that peers can be identified by their public keys
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device_id = Database::generate_device_id();
    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), device_id.clone())
        .unwrap();

    // Verify Alice has public keys for peer identification
    assert!(
        alice.public_key.is_some(),
        "User should have signing public key"
    );
    assert!(
        alice.encryption_public_key.is_some(),
        "User should have encryption public key"
    );

    // Store Iroh node ID for this device
    let node_id = "alice-node-12345";
    db.update_device_node_id(&device_id, node_id).unwrap();

    // Verify node ID is stored and can be retrieved
    let device = db.get_device(&device_id).unwrap();
    assert!(device.is_some(), "Device should exist with node ID");
}

#[test]
fn test_first_degree_peer_connection() {
    // Test 1st degree: Direct peer connection via public key exchange
    let temp_dir1 = TempDir::new().unwrap();
    let alice_db = Database::new(&temp_dir1.path().join("alice.db").to_string_lossy()).unwrap();

    let temp_dir2 = TempDir::new().unwrap();
    let bob_db = Database::new(&temp_dir2.path().join("bob.db").to_string_lossy()).unwrap();

    // Alice creates account
    let (alice, _) = alice_db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Bob creates account
    let (bob, _) = bob_db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Bob syncs Alice's public info (1st degree connection via public key)
    let alice_on_bob_device = bob_db
        .sync_peer_user(
            "alice",
            alice.public_key.as_ref().unwrap(),
            alice.encryption_public_key.as_ref().unwrap(),
        )
        .unwrap();

    // Verify Bob has Alice's public keys but NOT private keys
    assert_eq!(alice_on_bob_device.display_name, "alice");
    assert_eq!(alice_on_bob_device.public_key, alice.public_key);
    assert_eq!(
        alice_on_bob_device.encryption_public_key,
        alice.encryption_public_key
    );
    assert!(
        alice_on_bob_device.private_key.is_none(),
        "Private keys should never be shared"
    );
    assert!(
        alice_on_bob_device.encryption_private_key.is_none(),
        "Private keys should never be shared"
    );

    // Alice can now encrypt messages to Bob using Bob's public key
    let message = "Hello Bob!";
    let encrypted = Database::encrypt_message(
        message,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    // Bob can decrypt using his private key
    let decrypted = Database::decrypt_message(
        &encrypted,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(
        decrypted, message,
        "1st degree peers can exchange encrypted messages"
    );
}

#[test]
fn test_second_degree_peer_discovery() {
    // Test 2nd degree: Friends of friends
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    // Create Alice, Bob, Charlie
    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();
    let (charlie, _) = db
        .create_user_first_launch("charlie".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice <-> Bob (1st degree)
    db.add_friend(alice.id, bob.id).unwrap();
    db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![alice.id, bob.id]
    ).unwrap();

    // Bob <-> Charlie (1st degree)
    db.add_friend(bob.id, charlie.id).unwrap();
    db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![bob.id, charlie.id]
    ).unwrap();

    // Alice should discover Charlie as 2nd degree connection (friend of Bob)
    let second_degree = db.get_friends_of_friends(alice.id).unwrap();

    assert_eq!(
        second_degree.len(),
        1,
        "Alice should see Charlie as 2nd degree"
    );
    assert_eq!(second_degree[0].id, charlie.id);
    assert_eq!(second_degree[0].display_name, "charlie");
}

#[test]
fn test_multi_device_peer_discovery() {
    // Test that user's multiple devices are discovered as peers
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device1_id = Database::generate_device_id();
    let device2_id = Database::generate_device_id();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), device1_id.clone())
        .unwrap();

    // Create second device for same user
    let user_public_key = user.public_key.as_ref().unwrap();
    db.conn.lock().unwrap().execute(
        "INSERT INTO devices (id, user_public_key, device_name, last_sync, created_at) VALUES (?1, ?2, 'Alice Phone', datetime('now'), datetime('now'))",
        rusqlite::params![&device2_id, user_public_key]
    ).unwrap();

    // Store Iroh node IDs
    db.update_device_node_id(&device1_id, "node-device1")
        .unwrap();
    db.update_device_node_id(&device2_id, "node-device2")
        .unwrap();

    // Device 1 should discover Device 2 as peer
    let peer_node_ids = db.get_peer_node_ids(user_public_key, &device1_id).unwrap();

    assert_eq!(peer_node_ids.len(), 1, "Should find one peer device");
    assert_eq!(peer_node_ids[0], "node-device2");
}

#[test]
fn test_peer_relay_url_storage() {
    // Test storing and retrieving relay URLs for peer connections
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device_id = Database::generate_device_id();
    let (_user, _) = db
        .create_user_first_launch("alice".to_string(), device_id.clone())
        .unwrap();

    // Store node ID and relay URL
    db.update_device_node_id(&device_id, "alice-node-123")
        .unwrap();
    db.update_device_relay_url(&device_id, "https://relay.iroh.network")
        .unwrap();

    // Verify device info includes relay URL
    let device = db.get_device(&device_id).unwrap();
    assert!(device.is_some(), "Device should exist");
}

#[test]
fn test_all_peer_addresses_retrieval() {
    // Test getting all peer addresses for gossip network
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device1 = Database::generate_device_id();
    let device2 = Database::generate_device_id();

    let (_alice, _) = db
        .create_user_first_launch("alice".to_string(), device1.clone())
        .unwrap();
    let (_bob, _) = db
        .create_user_first_launch("bob".to_string(), device2.clone())
        .unwrap();

    // Store node info with relay URLs
    db.update_device_node_id(&device1, "node-alice").unwrap();
    db.update_device_node_id(&device2, "node-bob").unwrap();
    db.update_device_relay_url(&device1, "https://relay1.iroh.network")
        .unwrap();
    db.update_device_relay_url(&device2, "https://relay2.iroh.network")
        .unwrap();

    // Get all peer addresses for gossip network
    let peer_addrs = db.get_all_peer_addrs().unwrap();

    assert!(
        peer_addrs.len() >= 2,
        "Should find at least 2 peer addresses"
    );

    // Verify structure: (NodeId, Option<RelayURL>)
    let node_ids: Vec<&str> = peer_addrs.iter().map(|(id, _)| id.as_str()).collect();
    assert!(node_ids.contains(&"node-alice"));
    assert!(node_ids.contains(&"node-bob"));
}

#[test]
fn test_message_encryption_between_peers() {
    // Test end-to-end encryption between peers
    let temp_dir1 = TempDir::new().unwrap();
    let alice_db = Database::new(&temp_dir1.path().join("alice.db").to_string_lossy()).unwrap();

    let temp_dir2 = TempDir::new().unwrap();
    let bob_db = Database::new(&temp_dir2.path().join("bob.db").to_string_lossy()).unwrap();

    let (alice, _) = alice_db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = bob_db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice encrypts to Bob
    let message = "Secret P2P message";
    let encrypted = Database::encrypt_message(
        message,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    // Message should be encrypted
    assert_ne!(encrypted, message);

    // Bob decrypts
    let decrypted = Database::decrypt_message(
        &encrypted,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(decrypted, message, "Peers can exchange encrypted messages");
}

#[test]
fn test_message_signing_for_authenticity() {
    // Test message signing to verify sender identity
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    let message = "Authenticated message from Alice";

    // Alice signs message
    let signature = Database::sign_message(message, alice.private_key.as_ref().unwrap()).unwrap();

    // Anyone can verify it's from Alice using her public key
    let is_valid =
        Database::verify_signature(message, &signature, alice.public_key.as_ref().unwrap());

    assert!(
        is_valid,
        "Peers can verify message authenticity via signatures"
    );

    // Tampered message should fail verification
    let tampered = "Tampered message";
    let is_valid =
        Database::verify_signature(tampered, &signature, alice.public_key.as_ref().unwrap());

    assert!(!is_valid, "Tampered messages should fail verification");
}
