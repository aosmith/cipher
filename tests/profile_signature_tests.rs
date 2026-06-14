// Tests for profile signing/verification and recovery-phrase hashing
// (src/app/database/crypto.rs). These paths guard against profile tampering
// and protect the recovery phrase at rest.

use app::database::crypto;
use app::Database;

// A known-valid 12-word BIP39 mnemonic and a deliberately invalid one.
// The invalid phrase contains a token ("notaword") that is not in the BIP39
// wordlist, so it fails parsing regardless of checksum leniency.
const VALID_PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const INVALID_PHRASE: &str =
    "abandon abandon notaword abandon abandon abandon abandon abandon abandon abandon abandon about";

/// Derive a real Ed25519 signing keypair (base64) from a recovery phrase.
fn keypair(display_name: &str) -> (String, String) {
    let private =
        crypto::derive_private_key_from_recovery_phrase(VALID_PHRASE, display_name).unwrap();
    let public = crypto::get_public_key_from_private(&private);
    (private, public)
}

#[test]
fn test_sign_and_verify_profile() {
    let (private, public) = keypair("alice");

    let sig = Database::sign_profile_data(&private, "Alice", "my bio", "avatar.png").unwrap();
    assert!(Database::verify_profile_signature(
        &public,
        "Alice",
        "my bio",
        "avatar.png",
        &sig
    ));
}

#[test]
fn test_verify_profile_fails_on_tampered_field() {
    let (private, public) = keypair("alice");
    let sig = Database::sign_profile_data(&private, "Alice", "my bio", "avatar.png").unwrap();

    // Any change to the signed fields must invalidate the signature.
    assert!(!Database::verify_profile_signature(
        &public,
        "Mallory",
        "my bio",
        "avatar.png",
        &sig
    ));
    assert!(!Database::verify_profile_signature(
        &public,
        "Alice",
        "edited bio",
        "avatar.png",
        &sig
    ));
    assert!(!Database::verify_profile_signature(
        &public,
        "Alice",
        "my bio",
        "other.png",
        &sig
    ));
}

#[test]
fn test_verify_profile_fails_with_wrong_key() {
    let (private, _public) = keypair("alice");
    let (_other_priv, other_public) = keypair("bob");

    let sig = Database::sign_profile_data(&private, "Alice", "bio", "pic").unwrap();
    assert!(
        !Database::verify_profile_signature(&other_public, "Alice", "bio", "pic", &sig),
        "a different key must not verify the signature"
    );
}

#[test]
fn test_profile_signature_handles_empty_fields() {
    let (private, public) = keypair("alice");
    // Empty bio / picture is the common case for a fresh profile.
    let sig = Database::sign_profile_data(&private, "Alice", "", "").unwrap();
    assert!(Database::verify_profile_signature(
        &public, "Alice", "", "", &sig
    ));
}

#[test]
fn test_validate_recovery_phrase() {
    assert!(Database::validate_recovery_phrase(VALID_PHRASE));
    assert!(!Database::validate_recovery_phrase(INVALID_PHRASE));
    assert!(!Database::validate_recovery_phrase("not even close"));
    assert!(!Database::validate_recovery_phrase(""));
}

#[test]
fn test_hash_recovery_phrase_secure_is_salted() {
    let h1 = Database::hash_recovery_phrase_secure(VALID_PHRASE).unwrap();
    let h2 = Database::hash_recovery_phrase_secure(VALID_PHRASE).unwrap();

    // Argon2id uses a random salt, so the same phrase yields different hashes,
    // and the plaintext never appears in the hash.
    assert_ne!(h1, h2);
    assert!(!h1.contains(VALID_PHRASE));
    assert!(h1.starts_with("$argon2id$"));
}

#[test]
fn test_verify_recovery_phrase_roundtrip() {
    let hash = Database::hash_recovery_phrase_secure(VALID_PHRASE).unwrap();

    assert!(Database::verify_recovery_phrase(VALID_PHRASE, &hash).unwrap());
    assert!(!Database::verify_recovery_phrase("a different phrase", &hash).unwrap());
}

#[test]
fn test_verify_recovery_phrase_rejects_malformed_hash() {
    assert!(Database::verify_recovery_phrase(VALID_PHRASE, "not-a-valid-hash").is_err());
}

#[test]
fn test_derive_keys_is_deterministic_and_name_bound() {
    let (s1, e1) = Database::derive_keys_from_recovery_phrase(VALID_PHRASE, "alice").unwrap();
    let (s2, e2) = Database::derive_keys_from_recovery_phrase(VALID_PHRASE, "alice").unwrap();
    // Same inputs -> same keys.
    assert_eq!(s1, s2);
    assert_eq!(e1, e2);

    // Different display name -> different identity (keys differ).
    let (s3, _e3) = Database::derive_keys_from_recovery_phrase(VALID_PHRASE, "bob").unwrap();
    assert_ne!(s1, s3, "display name must be bound into key derivation");

    // Signing and encryption seeds are distinct domains.
    assert_ne!(s1, e1);
}

#[test]
fn test_derive_keys_rejects_invalid_inputs() {
    assert!(Database::derive_keys_from_recovery_phrase(INVALID_PHRASE, "alice").is_err());
    assert!(Database::derive_keys_from_recovery_phrase(VALID_PHRASE, "   ").is_err());
}
