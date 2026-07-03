// Tests for rotating signed pre-keys (src/app/database/prekeys.rs) and the
// best-recipient-key selection used by the seal path.

use app::database::prekeys::{best_recipient_key, FRIEND_PREKEY_FRESH_SECS};
use app::Database;
use tempfile::TempDir;

fn db_with_user(name: &str) -> (Database, app::User) {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();
    std::mem::forget(temp_dir);
    let device_id = Database::generate_device_id();
    let (user, _) = db
        .create_user_first_launch(name.to_string(), device_id)
        .unwrap();
    (db, user)
}

#[test]
fn test_ensure_prekey_is_idempotent() {
    let (db, user) = db_with_user("Alice");
    let signing_key = user.private_key.clone().unwrap();

    let first = db.ensure_current_prekey(user.id, &signing_key).unwrap();
    let second = db.ensure_current_prekey(user.id, &signing_key).unwrap();

    // Second call returns the same current pre-key, doesn't mint a new one
    assert_eq!(first.public_key, second.public_key);
    assert_eq!(db.get_prekey_private_keys(user.id).len(), 1);
}

#[test]
fn test_prekey_signature_verifies_against_identity() {
    let (db, user) = db_with_user("Alice");
    let signing_key = user.private_key.clone().unwrap();
    let identity_pub = user.public_key.clone().unwrap();

    let published = db.ensure_current_prekey(user.id, &signing_key).unwrap();

    let ctx = app::crypto::sealed_box::prekey_signing_context(&published.public_key);
    assert!(Database::verify_signature(
        &ctx,
        &published.signature,
        &identity_pub
    ));
}

#[test]
fn test_rotation_keeps_current_plus_previous_only() {
    let (db, user) = db_with_user("Alice");
    let signing_key = user.private_key.clone().unwrap();

    let k1 = db.ensure_current_prekey(user.id, &signing_key).unwrap();
    let k2 = db.rotate_prekey(user.id, &signing_key).unwrap();
    let k3 = db.rotate_prekey(user.id, &signing_key).unwrap();

    assert_ne!(k1.public_key, k2.public_key);
    assert_ne!(k2.public_key, k3.public_key);

    // After two rotations we keep current (k3) + previous (k2), not k1
    let privs = db.get_prekey_private_keys(user.id);
    assert_eq!(privs.len(), 2, "should keep exactly current + previous");

    // Current pre-key is k3
    assert_eq!(
        db.get_current_prekey(user.id).unwrap().public_key,
        k3.public_key
    );
}

#[test]
fn test_set_and_read_friend_prekey() {
    let (db, alice) = db_with_user("Alice");
    // Insert a friend user row (Bob) so we can attach a pre-key to it
    let bob_pub = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=";
    {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, display_name, public_key, encryption_public_key, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                app::SqliteUuid::from_public_key(bob_pub),
                "Bob",
                bob_pub,
                "Ym9iX2VuY3J5cHRpb25fa2V5X3BsYWNlaG9sZGVyXzEyMzQ1", // dummy enc key
                "2024-01-01T00:00:00+00:00",
                "2024-01-01T00:00:00+00:00",
            ],
        )
        .unwrap();
    }
    let _ = alice;

    let bob_prekey = "Ym9iX3ByZWtleV9wdWJsaWNfa2V5X3BsYWNlaG9sZGVyXzk4";
    db.set_friend_prekey(bob_pub, bob_prekey).unwrap();

    let (stored_prekey, updated_at): (Option<String>, Option<i64>) = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT prekey_public, prekey_updated_at FROM users WHERE public_key = ?1",
            rusqlite::params![bob_pub],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    };
    assert_eq!(stored_prekey.as_deref(), Some(bob_prekey));
    assert!(updated_at.is_some());
}

#[test]
fn test_best_recipient_key_prefers_fresh_prekey() {
    let now = chrono::Utc::now().timestamp();
    let identity = Some("identity_key".to_string());
    let prekey = Some("prekey".to_string());

    // Fresh pre-key wins
    assert_eq!(
        best_recipient_key(identity.clone(), prekey.clone(), Some(now)),
        Some("prekey".to_string())
    );
    // Stale pre-key falls back to identity key (delivery over forward secrecy)
    assert_eq!(
        best_recipient_key(
            identity.clone(),
            prekey.clone(),
            Some(now - FRIEND_PREKEY_FRESH_SECS - 1)
        ),
        Some("identity_key".to_string())
    );
    // No pre-key falls back to identity key
    assert_eq!(
        best_recipient_key(identity.clone(), None, None),
        Some("identity_key".to_string())
    );
    // No key at all -> None (nothing to seal to)
    assert_eq!(best_recipient_key(None, None, None), None);
}
