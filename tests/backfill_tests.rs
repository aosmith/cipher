// Tests for friend-content backfill: the DB watermark/query helpers that decide
// what a friend re-sends when a peer comes back online, and the sealed
// FriendSyncRequest / is_backfill wire fields.

use app::crypto::{ContentPayload, GossipEnvelope};
use app::{Database, SqliteUuid};
use base64::{engine::general_purpose, Engine as _};
use tempfile::TempDir;

fn fresh_db() -> Database {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();
    std::mem::forget(temp_dir);
    db
}

fn insert_post(db: &Database, author: SqliteUuid, content: &str, created_at: &str) -> SqliteUuid {
    let id = SqliteUuid::new();
    let conn = db.conn.lock().unwrap();
    // ensure the author exists (FK-free but get_posts joins users; not needed here)
    conn.execute(
        "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![author, "Author", format!("pk_{}", author), created_at],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO posts (id, user_id, content, encrypted, pinned, created_at, updated_at)
         VALUES (?1, ?2, ?3, 0, 0, ?4, ?4)",
        rusqlite::params![id, author, content, created_at],
    )
    .unwrap();
    id
}

#[test]
fn test_newest_post_time_watermark() {
    let db = fresh_db();
    let author = SqliteUuid::new();
    assert_eq!(db.newest_post_time_from_author(author).unwrap(), None);

    insert_post(&db, author, "old", "2026-01-01T00:00:00+00:00");
    insert_post(&db, author, "new", "2026-03-01T00:00:00+00:00");
    insert_post(&db, author, "mid", "2026-02-01T00:00:00+00:00");

    // Watermark is the MAX created_at, so a requester asks only for newer posts
    assert_eq!(
        db.newest_post_time_from_author(author).unwrap().as_deref(),
        Some("2026-03-01T00:00:00+00:00")
    );
}

#[test]
fn test_authored_posts_since_bounds_and_orders() {
    let db = fresh_db();
    let me = SqliteUuid::new();
    let other = SqliteUuid::new();

    insert_post(&db, me, "jan", "2026-01-01T00:00:00+00:00");
    insert_post(&db, me, "feb", "2026-02-01T00:00:00+00:00");
    insert_post(&db, me, "mar", "2026-03-01T00:00:00+00:00");
    // Another author's post must never be returned
    insert_post(&db, other, "not mine", "2026-04-01T00:00:00+00:00");

    // since = end of Jan -> only feb + mar, newest first
    let posts = db
        .get_authored_posts_since(me, "2026-01-15T00:00:00+00:00", 30)
        .unwrap();
    let contents: Vec<String> = posts.iter().map(|(_, c)| c.clone()).collect();
    assert_eq!(contents, vec!["mar".to_string(), "feb".to_string()]);

    // limit is honored
    let one = db
        .get_authored_posts_since(me, "2026-01-15T00:00:00+00:00", 1)
        .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].1, "mar");

    // since after the newest -> nothing
    let none = db
        .get_authored_posts_since(me, "2026-12-01T00:00:00+00:00", 30)
        .unwrap();
    assert!(none.is_empty());
}

fn x25519_keypair() -> (String, String) {
    use rand::Rng;
    use x25519_dalek::{PublicKey, StaticSecret};
    let mut secret = [0u8; 32];
    rand::thread_rng().fill(&mut secret);
    let sk = StaticSecret::from(secret);
    (
        general_purpose::STANDARD.encode(PublicKey::from(&sk).as_bytes()),
        general_purpose::STANDARD.encode(secret),
    )
}

fn ed25519_keypair() -> (String, String) {
    use rand::Rng;
    let mut secret = [0u8; 32];
    rand::thread_rng().fill(&mut secret);
    let sk = ed25519_dalek::SigningKey::from_bytes(&secret);
    (
        general_purpose::STANDARD.encode(sk.verifying_key().to_bytes()),
        general_purpose::STANDARD.encode(secret),
    )
}

#[test]
fn test_friend_sync_request_seals_and_authenticates() {
    let (sender_pub, sender_priv) = ed25519_keypair();
    let (recipient_pub, recipient_priv) = x25519_keypair();

    let payload = ContentPayload::FriendSyncRequest {
        since: 1_700_000_000,
        sent_at: 1_700_000_500,
    };
    let envelope =
        GossipEnvelope::seal(&payload, &[recipient_pub], &sender_pub, &sender_priv).unwrap();

    let decrypted = envelope
        .try_decrypt(&recipient_priv)
        .expect("should decrypt");
    assert_eq!(decrypted.sender_public_key, sender_pub);
    match decrypted.payload {
        ContentPayload::FriendSyncRequest { since, .. } => assert_eq!(since, 1_700_000_000),
        _ => panic!("wrong payload"),
    }
}

#[test]
fn test_backfill_post_flag_roundtrips() {
    // A backfilled post carries is_backfill=true so the receiver renders it
    // silently; a normal post defaults to false.
    let normal =
        r#"{"Post":{"post_id":"p","content":"c","node_id":"n","blob_refs":[],"sent_at":1}}"#;
    match serde_json::from_str::<ContentPayload>(normal).unwrap() {
        ContentPayload::Post { is_backfill, .. } => assert!(!is_backfill),
        _ => panic!("wrong payload"),
    }

    let backfill = r#"{"Post":{"post_id":"p","content":"c","node_id":"n","blob_refs":[],"sent_at":1,"is_backfill":true}}"#;
    match serde_json::from_str::<ContentPayload>(backfill).unwrap() {
        ContentPayload::Post { is_backfill, .. } => assert!(is_backfill),
        _ => panic!("wrong payload"),
    }
}
