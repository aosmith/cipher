// Tests for the communities database module (src/app/database/communities.rs)
// Covers community CRUD, membership, invite codes, and community posts.

use app::Database;
use tempfile::TempDir;

/// Create an isolated database with a single registered user.
fn db_with_user(name: &str) -> (Database, app::User) {
    let temp_dir = TempDir::new().unwrap();
    // Keep the TempDir alive for the lifetime of the DB by leaking it: the
    // process is short-lived (a single test) so the file is cleaned up on exit.
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();
    std::mem::forget(temp_dir);
    let (user, _) = db
        .create_user_first_launch(name.to_string(), Database::generate_device_id())
        .unwrap();
    (db, user)
}

#[test]
fn test_create_community_adds_creator_as_member() {
    let (db, alice) = db_with_user("alice");

    let community = db
        .create_community(alice.id, "Rustaceans", Some("People who like Rust"))
        .unwrap();

    assert_eq!(community.name, "Rustaceans");
    assert_eq!(
        community.description.as_deref(),
        Some("People who like Rust")
    );
    assert_eq!(community.creator_id, alice.id);
    assert_eq!(
        community.member_count, 1,
        "creator should be the first member"
    );

    // The creator should be stored with the 'creator' role.
    let members = db.get_community_members(community.id).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].user_id, alice.id);
    assert_eq!(members[0].role, "creator");
}

#[test]
fn test_get_community_nonexistent_returns_none() {
    let (db, _alice) = db_with_user("alice");
    let missing = db.get_community(app::SqliteUuid::new()).unwrap();
    assert!(missing.is_none());
}

#[test]
fn test_get_community_with_members() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    let bob = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap()
        .0;
    db.add_community_member(
        community.id,
        bob.id,
        "bob-enc-key",
        Some("bob"),
        Some(alice.id),
    )
    .unwrap();

    let with_members = db
        .get_community_with_members(community.id)
        .unwrap()
        .expect("community should exist");

    assert_eq!(with_members.community.id, community.id);
    assert_eq!(with_members.members.len(), 2);
}

#[test]
fn test_get_user_communities() {
    let (db, alice) = db_with_user("alice");
    db.create_community(alice.id, "Alpha", None).unwrap();
    db.create_community(alice.id, "Beta", None).unwrap();

    let communities = db.get_user_communities(alice.id).unwrap();
    assert_eq!(communities.len(), 2);
    // Returned ordered by name ascending.
    assert_eq!(communities[0].name, "Alpha");
    assert_eq!(communities[1].name, "Beta");

    // A user who is a member of nothing gets an empty list.
    let stranger = app::SqliteUuid::new();
    assert!(db.get_user_communities(stranger).unwrap().is_empty());
}

#[test]
fn test_update_community() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Old Name", None).unwrap();

    let updated = db
        .update_community(community.id, "New Name", Some("now with a description"))
        .unwrap();

    assert_eq!(updated.name, "New Name");
    assert_eq!(
        updated.description.as_deref(),
        Some("now with a description")
    );

    // Confirm it persisted.
    let fetched = db.get_community(community.id).unwrap().unwrap();
    assert_eq!(fetched.name, "New Name");
}

#[test]
fn test_delete_community_only_creator() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    // A non-creator cannot delete the community.
    let stranger = app::SqliteUuid::new();
    let denied = db.delete_community(community.id, stranger).unwrap();
    assert!(!denied, "non-creator must not be able to delete");
    assert!(db.get_community(community.id).unwrap().is_some());

    // The creator can.
    let deleted = db.delete_community(community.id, alice.id).unwrap();
    assert!(deleted);
    assert!(db.get_community(community.id).unwrap().is_none());
}

#[test]
fn test_add_and_remove_members() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    let bob = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap()
        .0;
    db.add_community_member(
        community.id,
        bob.id,
        "bob-enc-key",
        Some("bob"),
        Some(alice.id),
    )
    .unwrap();
    assert_eq!(db.get_community_members(community.id).unwrap().len(), 2);

    // Removing a regular member succeeds.
    let removed = db.remove_community_member(community.id, bob.id).unwrap();
    assert!(removed);
    assert_eq!(db.get_community_members(community.id).unwrap().len(), 1);

    // Removing a non-member returns false (nothing deleted).
    let again = db.remove_community_member(community.id, bob.id).unwrap();
    assert!(!again);
}

#[test]
fn test_cannot_remove_creator() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    let removed = db.remove_community_member(community.id, alice.id).unwrap();
    assert!(!removed, "the creator must not be removable");
    assert_eq!(db.get_community_members(community.id).unwrap().len(), 1);
}

#[test]
fn test_get_community_member_public_keys_skips_empty() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    // Member with a real key.
    let bob = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap()
        .0;
    db.add_community_member(community.id, bob.id, "bob-enc-key", Some("bob"), None)
        .unwrap();
    // Member with an empty key should be excluded by the query.
    let carol = db
        .create_user_first_launch("carol".to_string(), Database::generate_device_id())
        .unwrap()
        .0;
    db.add_community_member(community.id, carol.id, "", Some("carol"), None)
        .unwrap();

    let keys = db.get_community_member_public_keys(community.id).unwrap();
    assert!(keys.contains(&"bob-enc-key".to_string()));
    assert!(!keys.iter().any(|k| k.is_empty()));
    // Alice's encryption key (set at registration) is also present.
    assert!(keys
        .iter()
        .any(|k| k == alice.encryption_public_key.as_ref().unwrap()));
}

#[test]
fn test_is_community_member() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    assert!(db.is_community_member(community.id, alice.id).unwrap());

    let stranger = app::SqliteUuid::new();
    assert!(!db.is_community_member(community.id, stranger).unwrap());
}

#[test]
fn test_create_and_use_invite() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    let invite = db
        .create_community_invite(community.id, alice.id, 2, 24)
        .unwrap();
    assert_eq!(invite.community_name, "Club");
    assert_eq!(invite.uses_remaining, 2);
    assert!(!invite.invite_code.is_empty());

    // Bob is a real, separately-registered user.
    let (_, bob) = (
        (),
        db.create_user_first_launch("bob".to_string(), Database::generate_device_id())
            .unwrap()
            .0,
    );

    let joined = db
        .use_community_invite(bob.id, &invite.invite_code)
        .unwrap()
        .expect("valid invite should return the community");
    assert_eq!(joined.id, community.id);
    assert!(db.is_community_member(community.id, bob.id).unwrap());

    // Uses remaining decremented from 2 to 1.
    let invites = db.get_community_invites(community.id).unwrap();
    assert_eq!(invites.len(), 1);
    assert_eq!(invites[0].uses_remaining, 1);
}

#[test]
fn test_use_invite_invalid_code_returns_none() {
    let (db, alice) = db_with_user("alice");
    db.create_community(alice.id, "Club", None).unwrap();

    let result = db
        .use_community_invite(alice.id, "NOT-A-REAL-CODE")
        .unwrap();
    assert!(result.is_none());
}

#[test]
fn test_use_invite_expired_returns_none() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    // hours_valid of -1 puts the expiry in the past.
    let invite = db
        .create_community_invite(community.id, alice.id, 5, -1)
        .unwrap();

    let (_, bob) = (
        (),
        db.create_user_first_launch("bob".to_string(), Database::generate_device_id())
            .unwrap()
            .0,
    );

    let result = db
        .use_community_invite(bob.id, &invite.invite_code)
        .unwrap();
    assert!(result.is_none(), "expired invite must not allow joining");
    assert!(!db.is_community_member(community.id, bob.id).unwrap());
}

#[test]
fn test_use_invite_exhausted_returns_none() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    // Only one use allowed.
    let invite = db
        .create_community_invite(community.id, alice.id, 1, 24)
        .unwrap();

    let bob = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap()
        .0;
    let carol = db
        .create_user_first_launch("carol".to_string(), Database::generate_device_id())
        .unwrap()
        .0;

    // First use consumes the single available use.
    assert!(db
        .use_community_invite(bob.id, &invite.invite_code)
        .unwrap()
        .is_some());
    // Second distinct user is rejected.
    assert!(db
        .use_community_invite(carol.id, &invite.invite_code)
        .unwrap()
        .is_none());
    assert!(!db.is_community_member(community.id, carol.id).unwrap());
}

#[test]
fn test_use_invite_already_member_does_not_decrement() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();
    let invite = db
        .create_community_invite(community.id, alice.id, 3, 24)
        .unwrap();

    // The creator is already a member; using the invite is a no-op join that
    // still returns the community but must not consume a use.
    let result = db
        .use_community_invite(alice.id, &invite.invite_code)
        .unwrap();
    assert!(result.is_some());

    let invites = db.get_community_invites(community.id).unwrap();
    assert_eq!(
        invites[0].uses_remaining, 3,
        "already-member join must not decrement uses"
    );
}

#[test]
fn test_community_posts() {
    let (db, alice) = db_with_user("alice");
    let community = db.create_community(alice.id, "Club", None).unwrap();

    let post = db.create_post(alice.id, "Hello community", false).unwrap();
    let link = db
        .create_community_post(community.id, post.id, true)
        .unwrap();
    assert_eq!(link.community_id, community.id);
    assert_eq!(link.post_id, post.id);
    assert!(link.show_in_main_feed);

    let posts = db.get_community_posts(community.id).unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].id, post.id);
    assert_eq!(posts[0].content, "Hello community");
}
