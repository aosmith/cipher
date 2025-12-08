// Cryptography tests for Cipher
// Tests encryption, decryption, signing, and key derivation

use app::database::crypto;

// ===== Recovery Phrase Tests =====

#[test]
fn test_generate_recovery_phrase() {
    let phrase = crypto::generate_recovery_phrase();

    // Should be 24 words (BIP39 256-bit entropy)
    let word_count = phrase.split_whitespace().count();
    assert_eq!(word_count, 24, "Recovery phrase should have 24 words");

    // Each word should be non-empty
    for word in phrase.split_whitespace() {
        assert!(!word.is_empty());
    }
}

#[test]
fn test_recovery_phrase_deterministic_keys() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    let private_key1 = crypto::derive_private_key_from_recovery_phrase(phrase)
        .expect("Should derive key");
    let private_key2 = crypto::derive_private_key_from_recovery_phrase(phrase)
        .expect("Should derive key");

    // Same phrase should produce same key
    assert_eq!(private_key1, private_key2, "Key derivation should be deterministic");
}

#[test]
fn test_different_phrases_different_keys() {
    let phrase1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let phrase2 = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    let private_key1 = crypto::derive_private_key_from_recovery_phrase(phrase1)
        .expect("Should derive key");
    let private_key2 = crypto::derive_private_key_from_recovery_phrase(phrase2)
        .expect("Should derive key");

    // Different phrases should produce different keys
    assert_ne!(private_key1, private_key2, "Different phrases should produce different keys");
}

#[test]
fn test_public_key_derivation() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    let private_key = crypto::derive_private_key_from_recovery_phrase(phrase)
        .expect("Should derive key");
    let public_key = crypto::get_public_key_from_private(&private_key);

    // Public key should be deterministic
    let public_key2 = crypto::get_public_key_from_private(&private_key);
    assert_eq!(public_key, public_key2);

    // Public key should be non-empty
    assert!(!public_key.is_empty());
}

// ===== Signing Tests =====

#[test]
fn test_sign_and_verify() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let private_key = crypto::derive_private_key_from_recovery_phrase(phrase)
        .expect("Should derive key");
    let public_key = crypto::get_public_key_from_private(&private_key);

    let message = "Test message to sign";

    // Sign
    let signature = crypto::sign_message(message, &private_key)
        .expect("Should sign");

    // Verify with correct public key
    let is_valid = crypto::verify_signature(message, &signature, &public_key)
        .expect("Should verify");
    assert!(is_valid, "Signature should be valid");
}

#[test]
fn test_verify_wrong_key_fails() {
    let phrase1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let phrase2 = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    let private_key1 = crypto::derive_private_key_from_recovery_phrase(phrase1)
        .expect("Should derive key");
    let private_key2 = crypto::derive_private_key_from_recovery_phrase(phrase2)
        .expect("Should derive key");
    let public_key2 = crypto::get_public_key_from_private(&private_key2);

    let message = "Test message";

    // Sign with key1
    let signature = crypto::sign_message(message, &private_key1)
        .expect("Should sign");

    // Verify with key2's public key should fail
    let is_valid = crypto::verify_signature(message, &signature, &public_key2)
        .expect("Should verify");
    assert!(!is_valid, "Signature should be invalid with wrong key");
}

#[test]
fn test_verify_wrong_message_fails() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let private_key = crypto::derive_private_key_from_recovery_phrase(phrase)
        .expect("Should derive key");
    let public_key = crypto::get_public_key_from_private(&private_key);

    let message = "Original message";
    let wrong_message = "Tampered message";

    // Sign original
    let signature = crypto::sign_message(message, &private_key)
        .expect("Should sign");

    // Verify with wrong message should fail
    let is_valid = crypto::verify_signature(wrong_message, &signature, &public_key)
        .expect("Should verify");
    assert!(!is_valid, "Signature should be invalid for different message");
}

// ===== Encryption Tests =====

#[test]
fn test_encrypt_decrypt_for_user() {
    // Use two different users with database to get encryption keys
    let temp_dir = tempfile::TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db = app::database::Database::new(&db_path.to_string_lossy())
        .expect("Should create database");

    let device_id1 = app::database::Database::generate_device_id();
    let (alice, _) = db.create_user_first_launch("alice".to_string(), device_id1)
        .expect("Should create alice");

    let device_id2 = app::database::Database::generate_device_id();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), device_id2)
        .expect("Should create bob");

    let plaintext = b"Secret message from Alice to Bob";

    // Alice encrypts for Bob
    let ciphertext = crypto::encrypt_for_user(
        plaintext,
        &bob.encryption_public_key.unwrap(),
        &alice.encryption_private_key.unwrap(),
    ).expect("Should encrypt");

    // Bob decrypts
    let decrypted = crypto::decrypt_from_user(
        &ciphertext,
        &alice.encryption_public_key.unwrap(),
        &bob.encryption_private_key.unwrap(),
    ).expect("Should decrypt");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_encryption_different_ciphertexts() {
    // Use database for encryption keys
    let temp_dir = tempfile::TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db = app::database::Database::new(&db_path.to_string_lossy())
        .expect("Should create database");

    let device_id1 = app::database::Database::generate_device_id();
    let (alice, _) = db.create_user_first_launch("alice".to_string(), device_id1)
        .expect("Should create alice");

    let device_id2 = app::database::Database::generate_device_id();
    let (bob, _) = db.create_user_first_launch("bob".to_string(), device_id2)
        .expect("Should create bob");

    let plaintext = b"Same message";

    // Encrypt twice
    let ciphertext1 = crypto::encrypt_for_user(
        plaintext,
        &bob.encryption_public_key.as_ref().unwrap(),
        &alice.encryption_private_key.as_ref().unwrap(),
    ).expect("Should encrypt");

    let ciphertext2 = crypto::encrypt_for_user(
        plaintext,
        &bob.encryption_public_key.as_ref().unwrap(),
        &alice.encryption_private_key.as_ref().unwrap(),
    ).expect("Should encrypt");

    // Ciphertexts should be different (due to random nonce)
    assert_ne!(ciphertext1, ciphertext2, "Ciphertexts should differ due to random nonce");

    // But both should decrypt to the same plaintext
    let decrypted1 = crypto::decrypt_from_user(
        &ciphertext1,
        &alice.encryption_public_key.as_ref().unwrap(),
        &bob.encryption_private_key.as_ref().unwrap(),
    ).expect("Should decrypt");

    let decrypted2 = crypto::decrypt_from_user(
        &ciphertext2,
        &alice.encryption_public_key.as_ref().unwrap(),
        &bob.encryption_private_key.as_ref().unwrap(),
    ).expect("Should decrypt");

    assert_eq!(decrypted1, plaintext);
    assert_eq!(decrypted2, plaintext);
}

#[test]
fn test_decrypt_wrong_key_fails() {
    let temp_dir = tempfile::TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db = app::database::Database::new(&db_path.to_string_lossy())
        .expect("Should create database");

    let (alice, _) = db.create_user_first_launch("alice".to_string(),
        app::database::Database::generate_device_id()).expect("Should create");
    let (bob, _) = db.create_user_first_launch("bob".to_string(),
        app::database::Database::generate_device_id()).expect("Should create");
    let (charlie, _) = db.create_user_first_launch("charlie".to_string(),
        app::database::Database::generate_device_id()).expect("Should create");

    let plaintext = b"Secret for Bob only";

    // Alice encrypts for Bob
    let ciphertext = crypto::encrypt_for_user(
        plaintext,
        &bob.encryption_public_key.unwrap(),
        &alice.encryption_private_key.unwrap(),
    ).expect("Should encrypt");

    // Charlie tries to decrypt (should fail)
    let result = crypto::decrypt_from_user(
        &ciphertext,
        &alice.encryption_public_key.unwrap(),
        &charlie.encryption_private_key.unwrap(),
    );

    assert!(result.is_err(), "Charlie should not be able to decrypt");
}

// ===== Recovery Phrase Hash Tests =====

#[test]
fn test_hash_recovery_phrase() {
    let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    let hash1 = crypto::hash_recovery_phrase(phrase);
    let hash2 = crypto::hash_recovery_phrase(phrase);

    // Argon2 uses random salt, so hashes will be different each time
    // But both should be valid Argon2 formatted hashes
    assert!(hash1.starts_with("$argon2id$"), "Hash should be Argon2 format");
    assert!(hash2.starts_with("$argon2id$"), "Hash should be Argon2 format");

    // Hashes should be non-empty and have reasonable length
    assert!(hash1.len() > 50, "Hash should be a full Argon2 string");
    assert!(hash2.len() > 50, "Hash should be a full Argon2 string");
}

#[test]
fn test_hash_different_phrases() {
    let phrase1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let phrase2 = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    let hash1 = crypto::hash_recovery_phrase(phrase1);
    let hash2 = crypto::hash_recovery_phrase(phrase2);

    // Different phrases should produce different hashes
    assert_ne!(hash1, hash2);
}

// ===== Edge Cases =====

#[test]
fn test_empty_message_encryption() {
    let temp_dir = tempfile::TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db = app::database::Database::new(&db_path.to_string_lossy())
        .expect("Should create database");

    let (alice, _) = db.create_user_first_launch("alice".to_string(),
        app::database::Database::generate_device_id()).expect("Should create");
    let (bob, _) = db.create_user_first_launch("bob".to_string(),
        app::database::Database::generate_device_id()).expect("Should create");

    let plaintext = b"";

    let ciphertext = crypto::encrypt_for_user(
        plaintext,
        &bob.encryption_public_key.unwrap(),
        &alice.encryption_private_key.unwrap(),
    ).expect("Should encrypt empty message");

    let decrypted = crypto::decrypt_from_user(
        &ciphertext,
        &alice.encryption_public_key.unwrap(),
        &bob.encryption_private_key.unwrap(),
    ).expect("Should decrypt");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_large_message_encryption() {
    let temp_dir = tempfile::TempDir::new().expect("Should create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db = app::database::Database::new(&db_path.to_string_lossy())
        .expect("Should create database");

    let (alice, _) = db.create_user_first_launch("alice".to_string(),
        app::database::Database::generate_device_id()).expect("Should create");
    let (bob, _) = db.create_user_first_launch("bob".to_string(),
        app::database::Database::generate_device_id()).expect("Should create");

    // 1MB message
    let plaintext: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();

    let ciphertext = crypto::encrypt_for_user(
        &plaintext,
        &bob.encryption_public_key.unwrap(),
        &alice.encryption_private_key.unwrap(),
    ).expect("Should encrypt large message");

    let decrypted = crypto::decrypt_from_user(
        &ciphertext,
        &alice.encryption_public_key.unwrap(),
        &bob.encryption_private_key.unwrap(),
    ).expect("Should decrypt");

    assert_eq!(decrypted, plaintext);
}
