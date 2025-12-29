// Database tests for Cipher
// Tests CRUD operations, device management, peer storage, sync operations

use app::Database;
use tempfile::TempDir;

#[test]
fn test_database_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let db = Database::new(&db_path.to_string_lossy());
    assert!(db.is_ok(), "Database initialization should succeed");

    // Verify database file was created
    assert!(db_path.exists(), "Database file should exist");
}

#[test]
fn test_user_crud_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    // Create
    let (user, _) = db.create_user_first_launch(
        "testuser".to_string(),
        Database::generate_device_id()
    ).unwrap();

    assert_eq!(user.display_name, "testuser");
    assert!(user.public_key.is_some());

    // Read
    let found = db.find_user_by_id(user.id).unwrap().unwrap();
    assert_eq!(found.display_name, "testuser");

    // Update
    let updated = db.update_user_profile(
        user.id,
        None, // don't change display_name
        Some("Test bio".to_string()),
        Some("avatar.jpg".to_string())
    ).unwrap();

    assert_eq!(updated.bio, Some("Test bio".to_string()));
    assert_eq!(updated.profile_picture, Some("avatar.jpg".to_string()));
}

#[test]
fn test_device_management() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db.create_user_first_launch(
        "testuser".to_string(),
        "device-1".to_string()
    ).unwrap();

    // Get devices for user
    let user_public_key = user.public_key.as_ref().unwrap();
    let devices = db.get_user_devices(user_public_key).unwrap();
    assert_eq!(devices.len(), 1, "User should have one device");
    assert_eq!(devices[0].id, "device-1");

    // Update device name
    let result = db.update_device_name(&devices[0].id, "My Phone");
    assert!(result.is_ok());

    let devices = db.get_user_devices(user_public_key).unwrap();
    assert_eq!(devices[0].device_name, Some("My Phone".to_string()));
}

#[test]
fn test_node_id_storage() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device_id = Database::generate_device_id();
    let (_user, _) = db.create_user_first_launch("testuser".to_string(), device_id.clone()).unwrap();

    // Store NodeId
    let node_id = "test-node-id-12345";
    let result = db.update_device_node_id(&device_id, node_id);
    assert!(result.is_ok(), "Storing NodeId should succeed");

    // Verify device exists (NodeId field not in Device struct yet)
    let device = db.get_device(&device_id).unwrap();
    assert!(device.is_some(), "Device should exist after NodeId update");
}

#[test]
fn test_relay_url_storage() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device_id = Database::generate_device_id();
    let (_user, _) = db.create_user_first_launch("testuser".to_string(), device_id.clone()).unwrap();

    // Store relay URL
    let relay_url = "https://relay.example.com";
    let result = db.update_device_relay_url(&device_id, relay_url);
    assert!(result.is_ok(), "Storing relay URL should succeed");

    // Verify device exists (relay_url field not in Device struct yet)
    let device = db.get_device(&device_id).unwrap();
    assert!(device.is_some(), "Device should exist after relay URL update");
}

#[test]
fn test_peer_address_management() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device1_id = Database::generate_device_id();
    let device2_id = Database::generate_device_id();

    let (user, _) = db.create_user_first_launch("alice".to_string(), device1_id.clone()).unwrap();

    // Create second device for same user
    let user_public_key = user.public_key.as_ref().unwrap();
    db.conn.lock().unwrap().execute(
        "INSERT INTO devices (id, user_public_key, device_name, last_sync, created_at) VALUES (?1, ?2, NULL, datetime('now'), datetime('now'))",
        rusqlite::params![&device2_id, user_public_key]
    ).unwrap();

    // Store NodeIds for both devices
    db.update_device_node_id(&device1_id, "node-id-1").unwrap();
    db.update_device_node_id(&device2_id, "node-id-2").unwrap();

    // Get peer NodeIds (should exclude the querying device)
    let peer_node_ids = db.get_peer_node_ids(user_public_key, &device1_id).unwrap();

    assert_eq!(peer_node_ids.len(), 1, "Should find one peer device");
    assert_eq!(peer_node_ids[0], "node-id-2");
}

#[test]
fn test_get_all_peer_addrs() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    // Create users with devices
    let device1 = Database::generate_device_id();
    let device2 = Database::generate_device_id();

    let (_alice, _) = db.create_user_first_launch("alice".to_string(), device1.clone()).unwrap();
    let (_bob, _) = db.create_user_first_launch("bob".to_string(), device2.clone()).unwrap();

    // Store node info
    db.update_device_node_id(&device1, "node-alice").unwrap();
    db.update_device_node_id(&device2, "node-bob").unwrap();
    db.update_device_relay_url(&device1, "https://relay1.com").unwrap();
    db.update_device_relay_url(&device2, "https://relay2.com").unwrap();

    // Get all peer addresses
    let peer_addrs = db.get_all_peer_addrs().unwrap();

    assert!(peer_addrs.len() >= 2, "Should find at least 2 peer addresses");

    // Verify structure
    let node_ids: Vec<&str> = peer_addrs.iter().map(|(id, _)| id.as_str()).collect();
    assert!(node_ids.contains(&"node-alice"));
    assert!(node_ids.contains(&"node-bob"));
}

#[test]
fn test_sync_peer_user() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    // Create Alice on Device 1
    let (alice_original, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();

    // Create Bob's database (different device)
    let temp_dir2 = TempDir::new().unwrap();
    let bob_db = Database::new(&temp_dir2.path().join("bob.db").to_string_lossy()).unwrap();

    // Bob syncs Alice's public info to his database
    let alice_on_bob_device = bob_db.sync_peer_user(
        "alice",
        alice_original.public_key.as_ref().unwrap(),
        alice_original.encryption_public_key.as_ref().unwrap()
    ).unwrap();

    assert_eq!(alice_on_bob_device.display_name, "alice");
    assert_eq!(alice_on_bob_device.public_key, alice_original.public_key);

    // Private keys should NOT be synced
    assert!(alice_on_bob_device.private_key.is_none());
    assert!(alice_on_bob_device.encryption_private_key.is_none());
}

#[test]
#[ignore = "Friendship status model not implemented - system is key-based"]
fn test_friend_management() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    // Add friend
    let result = db.add_friend(alice.id, bob.id);
    assert!(result.is_ok(), "Adding friend should succeed");

    // Accept friendship
    db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![alice.id, bob.id]
    ).unwrap();

    // Get friends
    let friends = db.get_friends(alice.id).unwrap();
    assert_eq!(friends.len(), 1, "Alice should have 1 friend");
    assert_eq!(friends[0].id, bob.id);
}

#[test]
#[ignore = "Friendship status model not implemented - system is key-based"]
fn test_friend_public_keys_retrieval() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();
    let (charlie, _) = db.create_user_first_launch("charlie".to_string(), Database::generate_device_id()).unwrap();

    // Add friends
    db.add_friend(alice.id, bob.id).unwrap();
    db.add_friend(alice.id, charlie.id).unwrap();

    // Accept friendships
    db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1",
        rusqlite::params![alice.id]
    ).unwrap();

    // Get friend public keys
    let friend_keys = db.get_friend_public_keys(alice.id).unwrap();

    assert_eq!(friend_keys.len(), 2, "Should have 2 friend public keys");
    assert!(friend_keys.contains(bob.public_key.as_ref().unwrap()));
    assert!(friend_keys.contains(charlie.public_key.as_ref().unwrap()));
}

#[test]
fn test_post_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db.create_user_first_launch("testuser".to_string(), Database::generate_device_id()).unwrap();

    // Create post
    let post = db.create_post(user.id, "Test post content", false).unwrap();
    assert_eq!(post.content, "Test post content");
    assert_eq!(post.user_id, user.id);

    // Get posts
    let posts = db.get_posts(user.id).unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].content, "Test post content");
}

#[test]
fn test_message_storage() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db.create_user_first_launch("alice".to_string(), Database::generate_device_id()).unwrap();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), Database::generate_device_id()).unwrap();

    // Send message
    let message = db.send_encrypted_message(
        alice.id,
        bob.id,
        "Hello Bob!",
        None
    ).unwrap();

    assert_eq!(message.sender_id, alice.id);
    assert_eq!(message.recipient_id, bob.id);
    assert!(message.encrypted);

    // Retrieve messages
    let messages = db.get_messages_for_user(bob.id).unwrap();
    assert_eq!(messages.len(), 1);
}

#[test]
fn test_uuid_blob_storage() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db.create_user_first_launch("testuser".to_string(), Database::generate_device_id()).unwrap();

    // UUID should be stored as 16-byte BLOB and be retrievable
    let retrieved = db.find_user_by_id(user.id).unwrap().unwrap();
    assert_eq!(retrieved.id, user.id, "UUID should be stored and retrieved correctly");
}

#[test]
fn test_sync_data_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device_id = Database::generate_device_id();
    let (user, _) = db.create_user_first_launch("alice".to_string(), device_id.clone()).unwrap();

    // Create some data
    db.create_post(user.id, "Test post", false).unwrap();

    // Get sync data
    let sync_data = db.get_sync_data(&device_id, user.id).unwrap();

    assert_eq!(sync_data.posts.len(), 1);
    assert_eq!(sync_data.posts[0].content, "Test post");
}

#[test]
fn test_device_count() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let device1 = Database::generate_device_id();
    let (user, _) = db.create_user_first_launch("testuser".to_string(), device1.clone()).unwrap();

    // Initial count
    let user_public_key = user.public_key.as_ref().unwrap();
    let count = db.get_device_count(user_public_key).unwrap();
    assert_eq!(count, 1);

    // Add another device
    let device2 = Database::generate_device_id();
    db.conn.lock().unwrap().execute(
        "INSERT INTO devices (id, user_public_key, device_name, last_sync, created_at) VALUES (?1, ?2, NULL, datetime('now'), datetime('now'))",
        rusqlite::params![device2, user_public_key]
    ).unwrap();

    let count = db.get_device_count(user_public_key).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn test_concurrent_database_access() {
    // SQLite with WAL mode should support concurrent readers
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (user, _) = db.create_user_first_launch("testuser".to_string(), Database::generate_device_id()).unwrap();

    // Multiple readers should work
    let db1 = Database::new(&db_path.to_string_lossy()).unwrap();
    let db2 = Database::new(&db_path.to_string_lossy()).unwrap();

    let user1 = db1.find_user_by_id(user.id);
    let user2 = db2.find_user_by_id(user.id);

    assert!(user1.is_ok());
    assert!(user2.is_ok());
    assert_eq!(user1.unwrap().unwrap().id, user2.unwrap().unwrap().id);
}
