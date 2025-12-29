use app::Database;
use tempfile::TempDir;

#[test]
fn test_create_user_first_launch_success() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let result =
        db.create_user_first_launch("testuser".to_string(), Database::generate_device_id());

    assert!(result.is_ok(), "User creation should succeed");
    let (user, recovery_phrase) = result.unwrap();

    assert_eq!(user.display_name, "testuser");
    assert!(user.public_key.is_some(), "Public key should be generated");
    assert!(
        user.private_key.is_some(),
        "Private key should be generated"
    );
    assert!(
        user.encryption_public_key.is_some(),
        "Encryption public key should be generated"
    );
    assert!(
        user.encryption_private_key.is_some(),
        "Encryption private key should be generated"
    );
    assert!(
        user.recovery_phrase_hash.is_some(),
        "Recovery phrase should be hashed"
    );

    // Recovery phrase should be 24 words
    let words: Vec<&str> = recovery_phrase.split_whitespace().collect();
    assert_eq!(words.len(), 24, "Recovery phrase should be 24 words");
}

#[test]
fn test_restore_from_recovery_phrase() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    // Create user and save recovery phrase
    let (original_user, recovery_phrase) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Simulate restore on new device with new database
    let temp_dir2 = TempDir::new().unwrap();
    let db_path2 = temp_dir2.path().join("test2.db");
    let db2 = Database::new(&db_path2.to_string_lossy()).unwrap();

    let restored_user = db2
        .restore_user_from_recovery_phrase(
            "alice".to_string(),
            recovery_phrase,
            Database::generate_device_id(),
        )
        .unwrap();

    // Keys should be identical
    assert_eq!(
        original_user.public_key, restored_user.public_key,
        "Public keys should match"
    );
    assert_eq!(
        original_user.private_key, restored_user.private_key,
        "Private keys should match"
    );
    assert_eq!(
        original_user.encryption_public_key, restored_user.encryption_public_key,
        "Encryption public keys should match"
    );
    assert_eq!(
        original_user.encryption_private_key, restored_user.encryption_private_key,
        "Encryption private keys should match"
    );
}

#[test]
fn test_find_user_by_display_name() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (created_user, _) = db
        .create_user_first_launch("findme".to_string(), Database::generate_device_id())
        .unwrap();

    let found = db.find_user_by_display_name("findme").unwrap();
    assert!(found.is_some(), "User should be found");

    let user = found.unwrap();
    assert_eq!(user.id, created_user.id);
    assert_eq!(user.display_name, "findme");

    let not_found = db.find_user_by_display_name("nonexistent").unwrap();
    assert!(not_found.is_none(), "Non-existent user should return None");
}

#[test]
fn test_find_user_by_public_key() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (created_user, _) = db
        .create_user_first_launch("pubkeytest".to_string(), Database::generate_device_id())
        .unwrap();

    let public_key = created_user.public_key.clone().unwrap();
    let found = db.find_user_by_public_key(&public_key).unwrap();

    assert!(found.is_some(), "User should be found by public key");
    let user = found.unwrap();
    assert_eq!(user.display_name, "pubkeytest");
    assert_eq!(user.public_key, Some(public_key));

    // Private keys should not be exposed when looking up other users
    assert!(
        user.private_key.is_none(),
        "Private key should not be exposed"
    );
    assert!(
        user.encryption_private_key.is_none(),
        "Encryption private key should not be exposed"
    );
}

#[test]
fn test_recovery_phrase_is_deterministic() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    // Create user
    let (user1, recovery_phrase) = db
        .create_user_first_launch("deterministic".to_string(), Database::generate_device_id())
        .unwrap();

    // Restore with same recovery phrase should produce same keys
    let temp_dir2 = TempDir::new().unwrap();
    let db_path2 = temp_dir2.path().join("test2.db");
    let db2 = Database::new(&db_path2.to_string_lossy()).unwrap();

    let user2 = db2
        .restore_user_from_recovery_phrase(
            "deterministic".to_string(),
            recovery_phrase,
            Database::generate_device_id(),
        )
        .unwrap();

    // Keys should be identical because they're derived from recovery phrase
    assert_eq!(
        user1.public_key, user2.public_key,
        "Public keys should match"
    );
    assert_eq!(
        user1.private_key, user2.private_key,
        "Private keys should match"
    );
    assert_eq!(
        user1.encryption_public_key, user2.encryption_public_key,
        "Encryption public keys should match"
    );
    assert_eq!(
        user1.encryption_private_key, user2.encryption_private_key,
        "Encryption private keys should match"
    );
}

#[test]
fn test_different_recovery_phrases_generate_different_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (user1, _) = db
        .create_user_first_launch("keytest1".to_string(), Database::generate_device_id())
        .unwrap();

    let (user2, _) = db
        .create_user_first_launch("keytest2".to_string(), Database::generate_device_id())
        .unwrap();

    assert_ne!(
        user1.public_key, user2.public_key,
        "Different users should have different public keys"
    );
    assert_ne!(
        user1.encryption_public_key, user2.encryption_public_key,
        "Different users should have different encryption keys"
    );
}

#[test]
fn test_uuid_storage_as_blob() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("uuidtest".to_string(), Database::generate_device_id())
        .unwrap();

    // UUID should be stored and retrievable
    let found = db.find_user_by_display_name("uuidtest").unwrap().unwrap();
    assert_eq!(
        user.id, found.id,
        "UUID should be stored and retrieved correctly"
    );
}

#[test]
fn test_update_user_profile() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("profiletest".to_string(), Database::generate_device_id())
        .unwrap();

    let updated = db.update_user_profile(
        user.id,
        None, // don't change display_name
        Some("Test bio".to_string()),
        Some("profile.jpg".to_string()),
    );

    assert!(updated.is_ok(), "Profile update should succeed");
    let updated_user = updated.unwrap();
    assert_eq!(updated_user.bio, Some("Test bio".to_string()));
    assert_eq!(
        updated_user.profile_picture,
        Some("profile.jpg".to_string())
    );
}

#[test]
fn test_get_user_keys() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("keyretrieval".to_string(), Database::generate_device_id())
        .unwrap();

    let keys = db.get_user_keys(user.id);
    assert!(keys.is_ok(), "Should retrieve user keys");

    let (private_key, encryption_private_key) = keys.unwrap();
    assert_eq!(private_key, user.private_key.unwrap());
    assert_eq!(encryption_private_key, user.encryption_private_key.unwrap());
}

#[test]
fn test_recovery_phrase_not_stored_in_plaintext() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (user, recovery_phrase) = db
        .create_user_first_launch("securetest".to_string(), Database::generate_device_id())
        .unwrap();

    let hash = user.recovery_phrase_hash.unwrap();
    assert_ne!(
        hash, recovery_phrase,
        "Recovery phrase should not be stored in plaintext"
    );
    assert!(
        hash.starts_with("$argon2id$"),
        "Recovery phrase should be Argon2id hashed"
    );
}
