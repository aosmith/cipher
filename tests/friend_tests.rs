// Friend management tests for Cipher
// Tests adding friends, friend invites, QR codes, and friend discovery

use app::Database;
use tempfile::TempDir;

#[test]
fn test_add_friend_bidirectional() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice adds Bob
    db.add_friend(alice.id, bob.id).unwrap();

    // Bob adds Alice
    db.add_friend(bob.id, alice.id).unwrap();

    // Both should see each other in friends list
    let alice_friends = db.get_friends(alice.id).unwrap();
    let bob_friends = db.get_friends(bob.id).unwrap();

    assert_eq!(alice_friends.len(), 1);
    assert_eq!(bob_friends.len(), 1);
    assert_eq!(alice_friends[0].id, bob.id);
    assert_eq!(bob_friends[0].id, alice.id);
}

#[test]
fn test_add_friend_by_public_key() {
    // This test verifies adding a friend via sync and mutual add
    // Both users share the same database (single-device scenario)
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    // Create both users in same DB
    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice sends friend request to Bob
    db.add_friend(alice.id, bob.id).unwrap();

    // Bob adds Alice back - this auto-accepts
    db.add_friend(bob.id, alice.id).unwrap();

    // Verify both see each other as friends
    let alice_friends = db.get_friends(alice.id).unwrap();
    let bob_friends = db.get_friends(bob.id).unwrap();
    assert_eq!(alice_friends.len(), 1);
    assert_eq!(bob_friends.len(), 1);
    assert_eq!(alice_friends[0].public_key, bob.public_key);
    assert_eq!(bob_friends[0].public_key, alice.public_key);
}

#[test]
fn test_friend_invite_creation() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Create friend invite (1 use, valid for 24 hours)
    let invite = db.create_friend_invite(user.id, 1, 24).unwrap();

    assert!(
        invite.invite_code.len() > 0,
        "Invite code should not be empty"
    );
    assert_eq!(invite.creator_id, user.id);
    assert_eq!(
        invite.uses_remaining, 1,
        "Invite should have 1 use remaining"
    );
}

#[test]
fn test_friend_invite_usage() {
    // Invite codes are stored in the database, so both users need to share it
    // In real P2P, the invite code would be transmitted out-of-band (QR, link)
    // and the lookup would happen over the network. For testing, use shared DB.
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Alice creates invite (1 use, 24 hours validity)
    let invite = db.create_friend_invite(alice.id, 1, 24).unwrap();

    // Bob uses invite code
    let result = db.use_friend_invite(bob.id, invite.invite_code.clone());
    assert!(result.is_ok(), "Using friend invite should succeed");

    // Verify friendship created (use_friend_invite creates with status='accepted')
    let bob_friends = db.get_friends(bob.id).unwrap();
    assert_eq!(bob_friends.len(), 1);
    assert_eq!(bob_friends[0].id, alice.id);
}

// QR code functions are implemented as Tauri commands in src/app.rs
// and tested via integration tests rather than unit tests since they require
// the full Tauri app state. The functions are:
// - generate_qr_code: Generates QR code PNG as base64 from any string data
// - generate_friend_qr_code: Generates cipher://add-friend URL and QR code
// - parse_qr_code_data: Parses cipher://add-friend URL into QrCodeData struct
// - scan_qr_code_from_image: Scans QR from image and parses it

#[test]
fn test_friends_of_friends() {
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

    // Alice <-> Bob (friends)
    db.add_friend(alice.id, bob.id).unwrap();
    db.add_friend(bob.id, alice.id).unwrap();

    // Bob <-> Charlie (friends)
    db.add_friend(bob.id, charlie.id).unwrap();
    db.add_friend(charlie.id, bob.id).unwrap();

    // Mark friendships as accepted
    db.conn
        .lock()
        .unwrap()
        .execute("UPDATE p2p_connections SET status = 'accepted'", [])
        .unwrap();

    // Alice queries friends-of-friends
    let fof = db.get_friends_of_friends(alice.id).unwrap();

    // Charlie should appear as friend-of-friend (through Bob)
    let charlie_appears = fof.iter().any(|u| u.id == charlie.id);
    assert!(
        charlie_appears,
        "Charlie should be Alice's friend-of-friend"
    );
}

// export_friends_list and import_friends_list are Tauri commands in src/app.rs
// They are tested via integration tests since they require Tauri app State.
// - export_friends_list: Returns Vec<FriendExport> with display_name, public_key, bio, added_at
// - import_friends_list: Takes JSON string, returns FriendImportResult with added/skipped/errors

#[test]
fn test_recent_contacts_tracking() {
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

    // Alice interacts with Bob
    db.update_recent_contact(alice.id, bob.id).unwrap();

    // Wait a bit then interact with Charlie
    std::thread::sleep(std::time::Duration::from_millis(10));
    db.update_recent_contact(alice.id, charlie.id).unwrap();

    // Get recent contacts - Charlie should be first (most recent)
    let contacts = db.get_recent_contacts(alice.id, Some(10)).unwrap();
    assert_eq!(contacts.len(), 2);
    assert_eq!(
        contacts[0].id, charlie.id,
        "Most recent contact should be first"
    );
    assert_eq!(contacts[1].id, bob.id);

    // Interact with Bob again
    std::thread::sleep(std::time::Duration::from_millis(10));
    db.update_recent_contact(alice.id, bob.id).unwrap();

    // Now Bob should be first
    let contacts = db.get_recent_contacts(alice.id, Some(10)).unwrap();
    assert_eq!(contacts[0].id, bob.id, "Bob should now be most recent");
}

#[test]
fn test_friend_search() {
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

    // Alice is friends with Bob and Charlie
    db.add_friend(alice.id, bob.id).unwrap();
    db.add_friend(bob.id, alice.id).unwrap();
    db.add_friend(alice.id, charlie.id).unwrap();
    db.add_friend(charlie.id, alice.id).unwrap();

    // Mark friendships as accepted
    db.conn
        .lock()
        .unwrap()
        .execute("UPDATE p2p_connections SET status = 'accepted'", [])
        .unwrap();

    // Search for "bob" - should find Bob
    let results = db.search_friends(alice.id, "bob").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].display_name, "bob");

    // Search for "b" - should find Bob only (Charlie doesn't have 'b')
    let results = db.search_friends(alice.id, "b").unwrap();
    assert_eq!(results.len(), 1);

    // Search for "li" - should find Alice... wait, we're searching Alice's friends
    // so it should find Charlie (char"li"e)
    let results = db.search_friends(alice.id, "li").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].display_name, "charlie");

    // Search for non-existent name
    let results = db.search_friends(alice.id, "xyz").unwrap();
    assert_eq!(results.len(), 0);
}

#[test]
fn test_prevent_duplicate_friendships() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // First add should succeed (creates pending request)
    db.add_friend(alice.id, bob.id).unwrap();

    // Second add should fail (duplicate prevention)
    let result = db.add_friend(alice.id, bob.id);
    assert!(
        result.is_err(),
        "Second add_friend should fail as duplicate"
    );

    // Verify only one connection exists
    let conn = db.conn.lock().unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
            rusqlite::params![alice.id, bob.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "Should only have one connection entry");
}

#[test]
fn test_friend_status_management() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    // Add friend (starts as 'pending')
    db.add_friend(alice.id, bob.id).unwrap();

    // Check initial status
    let conn = db.conn.lock().unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
            rusqlite::params![alice.id, bob.id],
            |row| row.get(0),
        )
        .unwrap();
    drop(conn);

    assert_eq!(status, "pending");

    // Accept friendship
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![alice.id, bob.id],
    )
    .unwrap();
    drop(conn);

    // Verify status changed
    let conn = db.conn.lock().unwrap();
    let new_status: String = conn
        .query_row(
            "SELECT status FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
            rusqlite::params![alice.id, bob.id],
            |row| row.get(0),
        )
        .unwrap();
    drop(conn);

    assert_eq!(new_status, "accepted");
}

#[test]
fn test_cannot_add_self_as_friend() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Try to add self as friend
    let result = db.add_friend(user.id, user.id);

    // Should fail or be handled gracefully
    if result.is_ok() {
        let friends = db.get_friends(user.id).unwrap();
        let has_self = friends.iter().any(|f| f.id == user.id);
        assert!(
            !has_self,
            "User should not appear in their own friends list"
        );
    }
}
