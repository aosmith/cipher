// Cryptography tests for Cipher
// Tests Ed25519 signing, X25519 key exchange, ChaCha20Poly1305 encryption
// and key derivation functions

use app::Database;
use tempfile::TempDir;

#[test]
fn test_ed25519_signing_and_verification() {
    // Create test database
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    // Create user (generates Ed25519 keypair)
    let (user, _) = db
        .create_user_first_launch("testuser".to_string(), Database::generate_device_id())
        .unwrap();

    let public_key = user.public_key.unwrap();
    let private_key = user.private_key.unwrap();

    // Test data
    let message = "This is a test message for signing";

    // Sign the message
    let signature = Database::sign_message(message, &private_key);
    assert!(signature.is_ok(), "Message signing should succeed");
    let signature = signature.unwrap();

    // Verify the signature
    let is_valid = Database::verify_signature(message, &signature, &public_key);
    assert!(is_valid, "Signature verification should succeed");

    // Verify that modified message fails verification
    let tampered_message = "This is a TAMPERED message";
    let is_valid = Database::verify_signature(tampered_message, &signature, &public_key);
    assert!(!is_valid, "Tampered message should fail verification");

    // Verify that wrong public key fails verification
    let (other_user, _) = db
        .create_user_first_launch("other".to_string(), Database::generate_device_id())
        .unwrap();
    let other_public_key = other_user.public_key.unwrap();
    let is_valid = Database::verify_signature(message, &signature, &other_public_key);
    assert!(!is_valid, "Wrong public key should fail verification");
}

#[test]
fn test_x25519_encryption_and_decryption() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    // Create Alice
    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Create Bob
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    let plaintext = "Secret message from Alice to Bob";

    // Alice encrypts message to Bob
    let encrypted = Database::encrypt_message(
        plaintext,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_ne!(
        encrypted, plaintext,
        "Encrypted text should differ from plaintext"
    );

    // Bob decrypts message from Alice
    let decrypted = Database::decrypt_message(
        &encrypted,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(decrypted, plaintext, "Decrypted text should match original");
}

#[test]
fn test_encryption_is_authenticated() {
    // Verifies that ChaCha20Poly1305 AEAD properly rejects tampered ciphertexts
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    let plaintext = "Authenticated message";
    let mut encrypted = Database::encrypt_message(
        plaintext,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    // Tamper with ciphertext
    if encrypted.len() > 10 {
        unsafe {
            let bytes = encrypted.as_bytes_mut();
            bytes[10] ^= 0xFF; // Flip some bits
        }
    }

    // Decryption should fail due to authentication tag mismatch
    let decrypted = Database::decrypt_message(
        &encrypted,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    );

    assert!(
        decrypted.is_err(),
        "Tampered ciphertext should fail decryption"
    );
}

#[test]
fn test_recovery_phrase_deterministic_key_derivation() {
    // Verify that same recovery phrase always produces same keys
    let temp_dir1 = TempDir::new().unwrap();
    let db1 = Database::new(&temp_dir1.path().join("db1.db").to_string_lossy()).unwrap();

    let (user1, recovery_phrase) = db1
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();

    // Restore with same recovery phrase
    let temp_dir2 = TempDir::new().unwrap();
    let db2 = Database::new(&temp_dir2.path().join("db2.db").to_string_lossy()).unwrap();

    let user2 = db2
        .restore_user_from_recovery_phrase(
            "alice".to_string(),
            recovery_phrase,
            Database::generate_device_id(),
        )
        .unwrap();

    // All keys should be identical
    assert_eq!(
        user1.public_key, user2.public_key,
        "Ed25519 public keys should match"
    );
    assert_eq!(
        user1.private_key, user2.private_key,
        "Ed25519 private keys should match"
    );
    assert_eq!(
        user1.encryption_public_key, user2.encryption_public_key,
        "X25519 public keys should match"
    );
    assert_eq!(
        user1.encryption_private_key, user2.encryption_private_key,
        "X25519 private keys should match"
    );
}

#[test]
fn test_recovery_phrase_format() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (_user, recovery_phrase) = db
        .create_user_first_launch("testuser".to_string(), Database::generate_device_id())
        .unwrap();

    // Should be 24 words from BIP39 wordlist
    let words: Vec<&str> = recovery_phrase.split_whitespace().collect();
    assert_eq!(words.len(), 24, "Recovery phrase should have 24 words");

    // Each word should be lowercase alphabetic
    for word in &words {
        assert!(
            word.chars().all(|c| c.is_ascii_lowercase()),
            "Words should be lowercase"
        );
        assert!(word.len() >= 3, "Words should be at least 3 characters");
    }
}

#[test]
fn test_key_generation_is_cryptographically_random() {
    // Generate multiple users and verify keys are unique
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let mut public_keys = std::collections::HashSet::new();
    let mut encryption_keys = std::collections::HashSet::new();

    for i in 0..10 {
        let (user, _) = db
            .create_user_first_launch(format!("user{}", i), Database::generate_device_id())
            .unwrap();

        let pk = user.public_key.unwrap();
        let epk = user.encryption_public_key.unwrap();

        assert!(
            !public_keys.contains(&pk),
            "Duplicate Ed25519 public key detected"
        );
        assert!(
            !encryption_keys.contains(&epk),
            "Duplicate X25519 public key detected"
        );

        public_keys.insert(pk);
        encryption_keys.insert(epk);
    }

    assert_eq!(
        public_keys.len(),
        10,
        "Should generate 10 unique signing keys"
    );
    assert_eq!(
        encryption_keys.len(),
        10,
        "Should generate 10 unique encryption keys"
    );
}

#[test]
fn test_base64_encoding_is_standard() {
    // Verify that keys use standard base64 encoding
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (user, _) = db
        .create_user_first_launch("testuser".to_string(), Database::generate_device_id())
        .unwrap();

    let public_key = user.public_key.unwrap();

    // Should be valid base64
    let decoded = base64::decode(&public_key);
    assert!(decoded.is_ok(), "Public key should be valid base64");

    // Ed25519 public keys are 32 bytes
    assert_eq!(
        decoded.unwrap().len(),
        32,
        "Decoded public key should be 32 bytes"
    );
}

#[test]
fn test_message_encryption_produces_different_ciphertexts() {
    // Verify that encrypting the same message twice produces different ciphertexts
    // (due to random nonce generation)
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();

    let plaintext = "Same message encrypted twice";

    let encrypted1 = Database::encrypt_message(
        plaintext,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    let encrypted2 = Database::encrypt_message(
        plaintext,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    // Different ciphertexts due to random nonces
    assert_ne!(
        encrypted1, encrypted2,
        "Two encryptions should produce different ciphertexts"
    );

    // But both should decrypt to same plaintext
    let decrypted1 = Database::decrypt_message(
        &encrypted1,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    let decrypted2 = Database::decrypt_message(
        &encrypted2,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(decrypted1, plaintext);
    assert_eq!(decrypted2, plaintext);
}

#[test]
fn test_encryption_with_wrong_recipient_key_fails() {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();

    let (alice, _) = db
        .create_user_first_launch("alice".to_string(), Database::generate_device_id())
        .unwrap();
    let (bob, _) = db
        .create_user_first_launch("bob".to_string(), Database::generate_device_id())
        .unwrap();
    let (eve, _) = db
        .create_user_first_launch("eve".to_string(), Database::generate_device_id())
        .unwrap();

    let plaintext = "Message for Bob";

    // Alice encrypts to Bob
    let encrypted = Database::encrypt_message(
        plaintext,
        bob.encryption_public_key.as_ref().unwrap(),
        alice.encryption_private_key.as_ref().unwrap(),
    )
    .unwrap();

    // Eve tries to decrypt (should fail)
    let eve_decrypt = Database::decrypt_message(
        &encrypted,
        alice.encryption_public_key.as_ref().unwrap(),
        eve.encryption_private_key.as_ref().unwrap(),
    );

    assert!(
        eve_decrypt.is_err(),
        "Eve should not be able to decrypt Alice->Bob message"
    );

    // Bob can decrypt successfully
    let bob_decrypt = Database::decrypt_message(
        &encrypted,
        alice.encryption_public_key.as_ref().unwrap(),
        bob.encryption_private_key.as_ref().unwrap(),
    );

    assert!(bob_decrypt.is_ok());
    assert_eq!(bob_decrypt.unwrap(), plaintext);
}
