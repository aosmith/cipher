// Tests for multi-device sync application (src/app/database/sync.rs).
// get_sync_data already has coverage; these focus on apply_sync_data's
// timestamp-based conflict resolution, sync timestamps, and sync status.

use app::database::sync::{CommentSync, FriendSync, ReactionSync, SyncData};
use app::{Database, Message, Post};
use tempfile::TempDir;

fn db_with_user(name: &str) -> (Database, app::User, String) {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();
    std::mem::forget(temp_dir);
    let device_id = Database::generate_device_id();
    let (user, _) = db
        .create_user_first_launch(name.to_string(), device_id.clone())
        .unwrap();
    (db, user, device_id)
}

fn make_post(
    id: app::SqliteUuid,
    user_id: app::SqliteUuid,
    content: &str,
    updated_at: &str,
) -> Post {
    Post {
        id,
        user_id,
        display_name: None,
        content: content.to_string(),
        encrypted: false,
        pinned: false,
        shared_post_id: None,
        share_comment: None,
        created_at: "2024-01-01T00:00:00+00:00".to_string(),
        updated_at: updated_at.to_string(),
    }
}

fn empty_sync() -> SyncData {
    SyncData {
        posts: vec![],
        messages: vec![],
        friends: vec![],
        comments: vec![],
        reactions: vec![],
    }
}

#[test]
fn test_apply_sync_inserts_new_post() {
    let (db, alice, _) = db_with_user("alice");
    let post_id = app::SqliteUuid::new();

    let sync = SyncData {
        posts: vec![make_post(
            post_id,
            alice.id,
            "synced from another device",
            "2026-01-01T00:00:00+00:00",
        )],
        ..empty_sync()
    };
    db.apply_sync_data(&sync).unwrap();

    let posts = db.get_posts(alice.id).unwrap();
    let found = posts
        .iter()
        .find(|p| p.id == post_id)
        .expect("post should be inserted");
    assert_eq!(found.content, "synced from another device");
}

#[test]
fn test_apply_sync_newer_post_wins() {
    let (db, alice, _) = db_with_user("alice");
    // Local post created "now" (2026).
    let local = db.create_post(alice.id, "original content", false).unwrap();

    // Incoming version is strictly newer.
    let sync = SyncData {
        posts: vec![make_post(
            local.id,
            alice.id,
            "edited on other device",
            "2099-01-01T00:00:00+00:00",
        )],
        ..empty_sync()
    };
    db.apply_sync_data(&sync).unwrap();

    let posts = db.get_posts(alice.id).unwrap();
    let found = posts.iter().find(|p| p.id == local.id).unwrap();
    assert_eq!(
        found.content, "edited on other device",
        "newer incoming post should win"
    );
}

#[test]
fn test_apply_sync_older_post_ignored() {
    let (db, alice, _) = db_with_user("alice");
    let local = db.create_post(alice.id, "original content", false).unwrap();

    // Incoming version is older than the local copy.
    let sync = SyncData {
        posts: vec![make_post(
            local.id,
            alice.id,
            "stale content",
            "2020-01-01T00:00:00+00:00",
        )],
        ..empty_sync()
    };
    db.apply_sync_data(&sync).unwrap();

    let posts = db.get_posts(alice.id).unwrap();
    let found = posts.iter().find(|p| p.id == local.id).unwrap();
    assert_eq!(
        found.content, "original content",
        "older incoming post must be ignored"
    );
}

#[test]
fn test_apply_sync_comments() {
    let (db, alice, _) = db_with_user("alice");
    let post = db.create_post(alice.id, "a post", false).unwrap();
    let comment_id = app::SqliteUuid::new();

    let sync = SyncData {
        comments: vec![CommentSync {
            id: comment_id,
            post_id: post.id,
            user_id: alice.id,
            content: "synced comment".to_string(),
            parent_comment_id: None,
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
            updated_at: "2026-01-01T00:00:00+00:00".to_string(),
        }],
        ..empty_sync()
    };
    db.apply_sync_data(&sync).unwrap();

    let comments = db.get_post_comments(post.id).unwrap();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].content, "synced comment");
}

#[test]
fn test_apply_sync_reactions_idempotent() {
    let (db, alice, _) = db_with_user("alice");
    let post = db.create_post(alice.id, "a post", false).unwrap();
    let reaction_id = app::SqliteUuid::new();

    let reaction = ReactionSync {
        id: reaction_id,
        post_id: post.id,
        user_id: alice.id,
        emoji: "👍".to_string(),
        created_at: "2026-01-01T00:00:00+00:00".to_string(),
    };
    let sync = SyncData {
        reactions: vec![reaction.clone()],
        ..empty_sync()
    };

    // Applying the same reaction twice must not create duplicates (INSERT OR IGNORE).
    db.apply_sync_data(&sync).unwrap();
    db.apply_sync_data(&sync).unwrap();

    let reactions = db.get_post_reactions(post.id).unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].emoji, "👍");
}

#[test]
fn test_apply_sync_friends() {
    let (db, alice, device_id) = db_with_user("alice");
    let bob = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap()
        .0;

    let friend_id = app::SqliteUuid::new();
    let sync = SyncData {
        friends: vec![FriendSync {
            id: friend_id,
            user_id: alice.id,
            friend_user_id: bob.id,
            status: "accepted".to_string(),
            initiated_by: alice.id,
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
            updated_at: "2026-01-01T00:00:00+00:00".to_string(),
        }],
        ..empty_sync()
    };
    db.apply_sync_data(&sync).unwrap();

    // A fresh device's sync view should surface the applied friend connection.
    let (_, _, friends, _, _) = db.get_sync_status(&device_id, alice.id).unwrap();
    assert_eq!(friends, 1);
}

#[test]
fn test_get_sync_status_counts() {
    let (db, alice, device_id) = db_with_user("alice");
    db.create_post(alice.id, "p1", false).unwrap();
    db.create_post(alice.id, "p2", false).unwrap();

    let (posts, messages, _friends, _comments, _reactions) =
        db.get_sync_status(&device_id, alice.id).unwrap();
    assert_eq!(posts, 2);
    assert_eq!(messages, 0);
}

#[test]
fn test_update_sync_timestamp_excludes_already_synced() {
    let (db, alice, device_id) = db_with_user("alice");
    db.create_post(alice.id, "before sync", false).unwrap();

    // Before marking synced, the post shows as pending.
    let (before, _, _, _, _) = db.get_sync_status(&device_id, alice.id).unwrap();
    assert_eq!(before, 1);

    // After marking all tables synced, nothing created earlier is pending.
    db.update_all_sync_timestamps(&device_id).unwrap();
    let (after, _, _, _, _) = db.get_sync_status(&device_id, alice.id).unwrap();
    assert_eq!(
        after, 0,
        "posts created before the sync timestamp must not re-sync"
    );
}

#[test]
fn test_apply_sync_messages() {
    let (db, alice, _) = db_with_user("alice");
    let bob = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap()
        .0;
    let message_id = app::SqliteUuid::new();

    let sync = SyncData {
        messages: vec![Message {
            id: message_id,
            sender_id: alice.id,
            recipient_id: bob.id,
            content: "synced message".to_string(),
            encrypted: true,
            signature: None,
            thread_id: None,
            disappear_after_seconds: None,
            disappears_at: None,
            created_at: "2026-01-01T00:00:00+00:00".to_string(),
            updated_at: "2026-01-01T00:00:00+00:00".to_string(),
            edited_at: None,
        }],
        ..empty_sync()
    };
    db.apply_sync_data(&sync).unwrap();

    let messages = db.get_messages_for_user(alice.id).unwrap();
    assert!(messages
        .iter()
        .any(|m| m.id == message_id && m.content == "synced message"));
}
