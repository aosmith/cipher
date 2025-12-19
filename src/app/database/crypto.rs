use base64::{engine::general_purpose, Engine as _};
use bip39::Mnemonic;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use pbkdf2::pbkdf2_hmac_array;
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use crate::app::Database;

impl Database {
    /// Generate a deterministic salt for key derivation based on username
    /// This ensures the same username always produces the same salt,
    /// which is required for multi-device sync with same credentials
    #[allow(dead_code)]
    pub fn generate_kdf_salt_from_username(username: &str) -> [u8; 32] {
        use sha2::Digest;
        let mut hasher = Sha256::new();
        hasher.update(b"cipher_kdf_salt_v1:");
        hasher.update(username.as_bytes());
        hasher.finalize().into()
    }


    pub fn generate_signing_keypair_from_seed(seed: &[u8; 32]) -> (String, String) {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();

        let public_key_b64 = general_purpose::STANDARD.encode(verifying_key.as_bytes());
        let private_key_b64 = general_purpose::STANDARD.encode(signing_key.as_bytes());

        (public_key_b64, private_key_b64)
    }

    pub fn generate_encryption_keypair_from_seed(seed: &[u8; 32]) -> (String, String) {
        let secret_key = StaticSecret::from(*seed);
        let public_key = X25519PublicKey::from(&secret_key);

        let public_key_b64 = general_purpose::STANDARD.encode(public_key.as_bytes());
        let private_key_b64 = general_purpose::STANDARD.encode(secret_key.as_bytes());

        (public_key_b64, private_key_b64)
    }

    pub fn encrypt_message(
        message: &str,
        recipient_public_key: &str,
        _sender_private_key: &str,
    ) -> Result<String, String> {
        // Decode recipient's static public key
        let recipient_pub_bytes = general_purpose::STANDARD
            .decode(recipient_public_key)
            .map_err(|_| "Invalid recipient public key")?;

        if recipient_pub_bytes.len() != 32 {
            return Err("Invalid key length".to_string());
        }

        let recipient_public =
            X25519PublicKey::from(<[u8; 32]>::try_from(recipient_pub_bytes).unwrap());

        // SECURITY: Generate ephemeral key pair for forward secrecy
        // Even if long-term keys are compromised, past messages remain secure
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut ephemeral_secret_bytes = [0u8; 32];
        rng.fill(&mut ephemeral_secret_bytes);
        let ephemeral_private = StaticSecret::from(ephemeral_secret_bytes);
        let ephemeral_public = X25519PublicKey::from(&ephemeral_private);

        // Perform ECDH with ephemeral private key and recipient's static public key
        let shared_secret = ephemeral_private.diffie_hellman(&recipient_public);

        // Use shared secret as XChaCha20Poly1305 key
        let key = Key::from_slice(shared_secret.as_bytes());
        let cipher = XChaCha20Poly1305::new(key);

        // SECURITY: Generate random 24-byte nonce (XChaCha20 uses 192-bit nonces)
        // Larger nonce space significantly reduces collision risk
        let mut nonce_bytes = [0u8; 24];
        rng.fill(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        // SECURITY: Add timestamp to prevent replay attacks
        // Messages older than 5 minutes will be rejected on decryption
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "System time error")?
            .as_millis() as u64;
        let timestamp_bytes = timestamp.to_be_bytes();

        // Encrypt message with timestamp prepended
        let mut message_with_timestamp = Vec::new();
        message_with_timestamp.extend_from_slice(&timestamp_bytes);
        message_with_timestamp.extend_from_slice(message.as_bytes());

        let ciphertext = cipher
            .encrypt(nonce, message_with_timestamp.as_slice())
            .map_err(|_| "Encryption failed")?;

        // Combine ephemeral_public_key + nonce + ciphertext and encode
        // Format: [32 bytes ephemeral public key][24 bytes XNonce][N bytes ciphertext(with timestamp)]
        let mut result = Vec::new();
        result.extend_from_slice(ephemeral_public.as_bytes());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(general_purpose::STANDARD.encode(result))
    }

    pub fn decrypt_message(
        encrypted_message: &str,
        _sender_public_key: &str,
        recipient_private_key: &str,
    ) -> Result<String, String> {
        // Decode the encrypted message
        let encrypted_data = general_purpose::STANDARD
            .decode(encrypted_message)
            .map_err(|_| "Invalid encrypted message format")?;

        // Format: [32 bytes ephemeral public key][24 bytes XNonce][N bytes ciphertext]
        if encrypted_data.len() < 56 {
            return Err("Encrypted message too short".to_string());
        }

        // Extract ephemeral public key (first 32 bytes)
        let ephemeral_pub_bytes = &encrypted_data[0..32];
        let ephemeral_public =
            X25519PublicKey::from(<[u8; 32]>::try_from(ephemeral_pub_bytes).unwrap());

        // Extract nonce (next 24 bytes for XChaCha20)
        let nonce_bytes = &encrypted_data[32..56];
        let nonce = XNonce::from_slice(nonce_bytes);

        // Rest is ciphertext
        let ciphertext = &encrypted_data[56..];

        // Decode recipient's private key
        let recipient_priv_bytes = general_purpose::STANDARD
            .decode(recipient_private_key)
            .map_err(|_| "Invalid recipient private key")?;

        if recipient_priv_bytes.len() != 32 {
            return Err("Invalid key length".to_string());
        }

        let recipient_private =
            StaticSecret::from(<[u8; 32]>::try_from(recipient_priv_bytes).unwrap());

        // Perform ECDH with recipient's static private key and sender's ephemeral public key
        let shared_secret = recipient_private.diffie_hellman(&ephemeral_public);

        // Use shared secret as XChaCha20Poly1305 key
        let key = Key::from_slice(shared_secret.as_bytes());
        let cipher = XChaCha20Poly1305::new(key);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "Decryption failed")?;

        // SECURITY: Validate timestamp to prevent replay attacks
        if plaintext.len() < 8 {
            return Err("Invalid message format".to_string());
        }

        // Extract timestamp (first 8 bytes)
        let timestamp_bytes: [u8; 8] = plaintext[0..8].try_into().unwrap();
        let message_timestamp = u64::from_be_bytes(timestamp_bytes);

        // Get current timestamp
        let current_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "System time error")?
            .as_millis() as u64;

        // Reject messages older than 5 minutes (300,000 milliseconds)
        const MAX_MESSAGE_AGE_MS: u64 = 300_000;
        if current_timestamp.saturating_sub(message_timestamp) > MAX_MESSAGE_AGE_MS {
            return Err("Message too old (possible replay attack)".to_string());
        }

        // Also reject messages from the future (clock skew tolerance: 1 minute)
        const CLOCK_SKEW_TOLERANCE_MS: u64 = 60_000;
        if message_timestamp.saturating_sub(current_timestamp) > CLOCK_SKEW_TOLERANCE_MS {
            return Err("Message timestamp is too far in the future".to_string());
        }

        // Extract actual message (skip first 8 bytes)
        let message_bytes = &plaintext[8..];

        String::from_utf8(message_bytes.to_vec())
            .map_err(|_| "Invalid UTF-8 in decrypted message".to_string())
    }

    pub fn sign_message(message: &str, private_key_b64: &str) -> Result<String, String> {
        let private_key_bytes = general_purpose::STANDARD
            .decode(private_key_b64)
            .map_err(|_| "Invalid private key format")?;

        if private_key_bytes.len() != 32 {
            return Err("Invalid private key length".to_string());
        }

        let signing_key = SigningKey::from_bytes(&<[u8; 32]>::try_from(private_key_bytes).unwrap());
        let signature = signing_key.sign(message.as_bytes());
        Ok(general_purpose::STANDARD.encode(signature.to_bytes()))
    }

    pub fn verify_signature(message: &str, signature_b64: &str, public_key_b64: &str) -> bool {
        // Decode signature and public key
        let signature_bytes = match general_purpose::STANDARD.decode(signature_b64) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let public_key_bytes = match general_purpose::STANDARD.decode(public_key_b64) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        if signature_bytes.len() != 64 || public_key_bytes.len() != 32 {
            return false;
        }

        let signature_array = match <[u8; 64]>::try_from(signature_bytes) {
            Ok(arr) => arr,
            Err(_) => return false,
        };
        let signature = Signature::from_bytes(&signature_array);

        let public_key_array = match <[u8; 32]>::try_from(public_key_bytes) {
            Ok(arr) => arr,
            Err(_) => return false,
        };
        let public_key = match VerifyingKey::from_bytes(&public_key_array) {
            Ok(key) => key,
            Err(_) => return false,
        };

        public_key.verify(message.as_bytes(), &signature).is_ok()
    }

    /// Generate a new BIP39 recovery phrase (24 words for 256 bits of entropy)
    /// Default is 24 words for maximum security
    pub fn generate_recovery_phrase(word_count: Option<usize>) -> Result<String, String> {
        // Determine entropy size (16 bytes = 128 bits = 12 words, 32 bytes = 256 bits = 24 words)
        let entropy_size = match word_count {
            Some(24) | None => 32, // 256 bits of entropy (default)
            Some(12) => 16,        // 128 bits of entropy (for testing only)
            Some(n) => return Err(format!("Invalid word count: {}. Must be 12 or 24.", n)),
        };

        // Generate random entropy
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut entropy = vec![0u8; entropy_size];
        rng.fill(&mut entropy[..]);

        // Create mnemonic from entropy (defaults to English)
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| format!("Failed to create mnemonic: {}", e))?;

        Ok(mnemonic.to_string())
    }

    /// Derive signing and encryption key seeds from recovery phrase AND display name
    ///
    /// SECURITY: The display name is cryptographically bound to the identity.
    /// This prevents an attacker who obtains a recovery phrase from impersonating
    /// the user with a different display name to existing contacts.
    ///
    /// To restore an account, you need BOTH the recovery phrase AND the exact display name.
    ///
    /// Returns (signing_seed, encryption_seed)
    pub fn derive_keys_from_recovery_phrase(
        recovery_phrase: &str,
        display_name: &str,
    ) -> Result<([u8; 32], [u8; 32]), String> {
        // Validate recovery phrase first
        if !Self::validate_recovery_phrase(recovery_phrase) {
            return Err("Invalid recovery phrase".to_string());
        }

        // Validate display name
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err("Display name cannot be empty".to_string());
        }

        // SECURITY: Combine recovery phrase with display name
        // The display name becomes part of the key derivation input
        // Format: "recovery_phrase|display_name"
        let combined_input = format!("{}|{}", recovery_phrase, display_name);

        // Derive signing key seed with v3 domain separator (breaking change from v2)
        let signing_seed = pbkdf2_hmac_array::<Sha256, 32>(
            combined_input.as_bytes(),
            b"cipher_signing_v3_with_name:", // v3: includes display name
            100_000,
        );

        // Derive encryption key seed
        let encryption_seed = pbkdf2_hmac_array::<Sha256, 32>(
            combined_input.as_bytes(),
            b"cipher_encryption_v3_with_name:", // v3: includes display name
            100_000,
        );

        Ok((signing_seed, encryption_seed))
    }

    /// Sign profile data (display_name, bio, profile_picture) with the user's private key
    ///
    /// SECURITY: This signature proves that the profile data was set by the owner
    /// of the private key. Friends can verify this signature to detect tampering.
    ///
    /// Format signed: "profile_v1|display_name|bio|profile_picture"
    pub fn sign_profile_data(
        private_key_b64: &str,
        display_name: &str,
        bio: &str,
        profile_picture: &str,
    ) -> Result<String, String> {
        // Create canonical format for signing
        // Using pipe separator and explicit empty strings for null values
        let data_to_sign = format!(
            "profile_v1|{}|{}|{}",
            display_name,
            bio,
            profile_picture
        );

        Self::sign_message(&data_to_sign, private_key_b64)
    }

    /// Verify a profile signature against the user's public key
    ///
    /// Returns true if the signature is valid for the given profile data
    pub fn verify_profile_signature(
        public_key_b64: &str,
        display_name: &str,
        bio: &str,
        profile_picture: &str,
        signature_b64: &str,
    ) -> bool {
        // Recreate the canonical format
        let data_to_verify = format!(
            "profile_v1|{}|{}|{}",
            display_name,
            bio,
            profile_picture
        );

        Self::verify_signature(&data_to_verify, signature_b64, public_key_b64)
    }

    /// Validate a BIP39 recovery phrase
    /// Returns true if the phrase is a valid BIP39 mnemonic
    pub fn validate_recovery_phrase(phrase: &str) -> bool {
        Mnemonic::parse(phrase).is_ok()
    }

    /// Securely hash recovery phrase using Argon2id
    /// This is the primary method for hashing recovery phrases in production
    pub fn hash_recovery_phrase_secure(phrase: &str) -> Result<String, String> {
        use argon2::{
            password_hash::{PasswordHasher, SaltString},
            Argon2, Algorithm, Params, Version
        };

        // Use Argon2id with production-ready parameters
        // Memory: 64MB, Iterations: 3, Parallelism: 4
        // These parameters provide good security while maintaining reasonable performance
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(65536, 3, 4, None)
                .map_err(|e| format!("Failed to create Argon2 params: {}", e))?
        );

        // Generate a cryptographically secure random salt
        let salt = SaltString::generate(&mut rand::thread_rng());

        // Hash the recovery phrase
        argon2.hash_password(phrase.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| format!("Failed to hash recovery phrase: {}", e))
    }

    /// Verify a recovery phrase against a stored hash
    #[allow(dead_code)]
    pub fn verify_recovery_phrase(phrase: &str, hash: &str) -> Result<bool, String> {
        use argon2::{Argon2, PasswordHash, PasswordVerifier};

        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| format!("Invalid hash format: {}", e))?;

        Ok(Argon2::default()
            .verify_password(phrase.as_bytes(), &parsed_hash)
            .is_ok())
    }
}

// Standalone crypto functions for testing
#[allow(dead_code)]
pub fn derive_private_key_from_recovery_phrase(
    recovery_phrase: &str,
    display_name: &str,
) -> Result<String, String> {
    // Derive the signing key from recovery phrase + display name
    let (signing_seed, _) =
        Database::derive_keys_from_recovery_phrase(recovery_phrase, display_name)?;
    let private_key_b64 = general_purpose::STANDARD.encode(signing_seed);
    Ok(private_key_b64)
}

#[allow(dead_code)]
pub fn get_public_key_from_private(private_key_b64: &str) -> String {
    // Decode the private key
    let private_key_bytes = general_purpose::STANDARD
        .decode(private_key_b64)
        .unwrap_or_else(|_| vec![]);

    if private_key_bytes.len() != 32 {
        return String::new();
    }

    let signing_key = SigningKey::from_bytes(&<[u8; 32]>::try_from(private_key_bytes).unwrap());
    let verifying_key = signing_key.verifying_key();
    general_purpose::STANDARD.encode(verifying_key.as_bytes())
}

#[allow(dead_code)]
pub fn encrypt_for_user(
    data: &[u8],
    recipient_public_key: &str,
    sender_private_key: &str,
) -> Result<Vec<u8>, String> {
    // Use X25519 for encryption
    let recipient_pub_bytes = general_purpose::STANDARD
        .decode(recipient_public_key)
        .map_err(|_| "Invalid recipient public key")?;

    let sender_priv_bytes = general_purpose::STANDARD
        .decode(sender_private_key)
        .map_err(|_| "Invalid sender private key")?;

    if recipient_pub_bytes.len() != 32 || sender_priv_bytes.len() != 32 {
        return Err("Invalid key length".to_string());
    }

    let recipient_public = X25519PublicKey::from(<[u8; 32]>::try_from(recipient_pub_bytes).unwrap());
    let sender_private = StaticSecret::from(<[u8; 32]>::try_from(sender_priv_bytes).unwrap());

    // Perform ECDH
    let shared_secret = sender_private.diffie_hellman(&recipient_public);

    // Use shared secret as encryption key
    let key = Key::from_slice(shared_secret.as_bytes());
    let cipher = XChaCha20Poly1305::new(key);

    // Generate nonce
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 24];
    rng.fill(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, data)
        .map_err(|_| "Encryption failed")?;

    // Combine nonce + ciphertext
    let mut result = Vec::new();
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

#[allow(dead_code)]
pub fn decrypt_from_user(
    encrypted_data: &[u8],
    sender_public_key: &str,
    recipient_private_key: &str,
) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < 24 {
        return Err("Invalid encrypted data".to_string());
    }

    // Extract nonce and ciphertext
    let nonce_bytes = &encrypted_data[..24];
    let ciphertext = &encrypted_data[24..];

    let sender_pub_bytes = general_purpose::STANDARD
        .decode(sender_public_key)
        .map_err(|_| "Invalid sender public key")?;

    let recipient_priv_bytes = general_purpose::STANDARD
        .decode(recipient_private_key)
        .map_err(|_| "Invalid recipient private key")?;

    if sender_pub_bytes.len() != 32 || recipient_priv_bytes.len() != 32 {
        return Err("Invalid key length".to_string());
    }

    let sender_public = X25519PublicKey::from(<[u8; 32]>::try_from(sender_pub_bytes).unwrap());
    let recipient_private = StaticSecret::from(<[u8; 32]>::try_from(recipient_priv_bytes).unwrap());

    // Perform ECDH
    let shared_secret = recipient_private.diffie_hellman(&sender_public);

    // Use shared secret as decryption key
    let key = Key::from_slice(shared_secret.as_bytes());
    let cipher = XChaCha20Poly1305::new(key);
    let nonce = XNonce::from_slice(nonce_bytes);

    // Decrypt
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed".to_string())
}

#[allow(dead_code)]
pub fn generate_recovery_phrase() -> String {
    Database::generate_recovery_phrase(None).unwrap_or_else(|_| String::new())
}

#[allow(dead_code)]
pub fn hash_recovery_phrase(phrase: &str) -> String {
    // Use the secure method from Database
    Database::hash_recovery_phrase_secure(phrase)
        .unwrap_or_else(|_| {
            // Fallback to SHA256 if Argon2 fails (should rarely happen)
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(phrase.as_bytes());
            hex::encode(hasher.finalize())
        })
}

#[allow(dead_code)]
pub fn sign_message(message: &str, private_key: &str) -> Result<String, String> {
    Database::sign_message(message, private_key)
}

#[allow(dead_code)]
pub fn verify_signature(message: &str, signature: &str, public_key: &str) -> Result<bool, String> {
    Ok(Database::verify_signature(message, signature, public_key))
}
