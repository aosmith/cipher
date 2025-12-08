// Sealed Box implementation for global mesh encryption
//
// Architecture:
// - GossipEnvelope contains multiple SealedBox instances (one per friend)
// - Each SealedBox can only be decrypted by its intended recipient
// - The `recipient_hint` (first 8 bytes of public key) allows quick filtering
// - Ephemeral keys provide forward secrecy for each message

use base64::{engine::general_purpose, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use crate::app::types::{MediaAttachmentWithData, SqliteUuid};

/// Content types that can be sealed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    Post,
    DirectMessage,
    FriendRequest,
    FriendAccepted,
    KeyRotation,
}

/// Envelope for gossiped content - contains multiple sealed boxes
/// All nodes receive this, but only intended recipients can decrypt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipEnvelope {
    /// Random ID for deduplication (32 bytes, hex encoded)
    pub message_id: String,
    /// Unix timestamp (for purging old content)
    pub timestamp: i64,
    /// Type hint for the content
    pub content_type: ContentType,
    /// Sender's public key (for verification after decryption)
    pub sender_public_key: String,
    /// One sealed box per intended recipient
    pub sealed_boxes: Vec<SealedBox>,
}

/// A sealed box that only one recipient can open
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBox {
    /// Ephemeral X25519 public key (for ECDH) - base64 encoded
    pub ephemeral_pubkey: String,
    /// First 8 bytes of recipient's public key (for quick filtering) - hex encoded
    pub recipient_hint: String,
    /// Nonce for XChaCha20-Poly1305 (24 bytes) - base64 encoded
    pub nonce: String,
    /// Encrypted payload - base64 encoded
    pub ciphertext: String,
}

/// The actual content inside a sealed box
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentPayload {
    Post {
        content: String,
        attachments: Option<Vec<MediaAttachmentWithData>>,
    },
    DirectMessage {
        content: String,
        thread_id: Option<SqliteUuid>,
    },
    FriendRequest {
        username: String,
        message: Option<String>,
    },
    FriendAccepted {
        username: String,
    },
    KeyRotation {
        /// New signed pre-key (base64)
        new_signed_prekey: String,
        /// Signature over the new key (base64)
        signature: String,
    },
}

impl GossipEnvelope {
    /// Create a new envelope for a post, encrypted for all friends
    pub fn new_post(
        sender_public_key: &str,
        content: &str,
        attachments: Option<Vec<MediaAttachmentWithData>>,
        friend_public_keys: &[String],
        sender_encryption_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let timestamp = chrono::Utc::now().timestamp();

        let payload = ContentPayload::Post {
            content: content.to_string(),
            attachments,
        };

        let sealed_boxes = create_sealed_boxes_for_recipients(
            &payload,
            friend_public_keys,
            sender_encryption_private_key,
        )?;

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            content_type: ContentType::Post,
            sender_public_key: sender_public_key.to_string(),
            sealed_boxes,
        })
    }

    /// Try to decrypt this envelope for a given recipient
    /// Returns the payload if we can decrypt any of the sealed boxes
    pub fn try_decrypt(
        &self,
        recipient_public_key: &str,
        recipient_private_key: &str,
    ) -> Option<ContentPayload> {
        // Calculate our recipient hint for quick filtering
        let our_hint = calculate_recipient_hint(recipient_public_key);

        for sealed_box in &self.sealed_boxes {
            // Quick filter: check if this box might be for us
            if sealed_box.recipient_hint != our_hint {
                continue;
            }

            // Try to decrypt
            if let Ok(payload) = sealed_box.decrypt(recipient_private_key) {
                return Some(payload);
            }
        }

        None
    }

    /// Check if this envelope might be for us (quick hint check)
    pub fn might_be_for_us(&self, recipient_public_key: &str) -> bool {
        let our_hint = calculate_recipient_hint(recipient_public_key);
        self.sealed_boxes.iter().any(|sb| sb.recipient_hint == our_hint)
    }
}

impl SealedBox {
    /// Create a new sealed box for a specific recipient
    pub fn new(
        payload: &ContentPayload,
        recipient_public_key: &str,
        _sender_private_key: &str,
    ) -> Result<Self, String> {
        // Serialize the payload
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;

        // Decode recipient's public key
        let recipient_pub_bytes = general_purpose::STANDARD
            .decode(recipient_public_key)
            .map_err(|_| "Invalid recipient public key")?;

        if recipient_pub_bytes.len() != 32 {
            return Err("Invalid recipient key length".to_string());
        }

        let recipient_public = X25519PublicKey::from(
            <[u8; 32]>::try_from(recipient_pub_bytes.as_slice()).unwrap()
        );

        // Generate ephemeral keypair for forward secrecy
        let mut rng = rand::thread_rng();
        let mut ephemeral_secret_bytes = [0u8; 32];
        rng.fill(&mut ephemeral_secret_bytes);
        let ephemeral_private = StaticSecret::from(ephemeral_secret_bytes);
        let ephemeral_public = X25519PublicKey::from(&ephemeral_private);

        // Perform ECDH
        let shared_secret = ephemeral_private.diffie_hellman(&recipient_public);

        // Encrypt with XChaCha20-Poly1305
        let key = Key::from_slice(shared_secret.as_bytes());
        let cipher = XChaCha20Poly1305::new(key);

        let mut nonce_bytes = [0u8; 24];
        rng.fill(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, payload_json.as_bytes())
            .map_err(|_| "Encryption failed")?;

        // Calculate recipient hint (first 8 bytes of public key, hex encoded)
        let recipient_hint = calculate_recipient_hint(recipient_public_key);

        Ok(SealedBox {
            ephemeral_pubkey: general_purpose::STANDARD.encode(ephemeral_public.as_bytes()),
            recipient_hint,
            nonce: general_purpose::STANDARD.encode(nonce_bytes),
            ciphertext: general_purpose::STANDARD.encode(ciphertext),
        })
    }

    /// Decrypt this sealed box
    pub fn decrypt(&self, recipient_private_key: &str) -> Result<ContentPayload, String> {
        // Decode ephemeral public key
        let ephemeral_pub_bytes = general_purpose::STANDARD
            .decode(&self.ephemeral_pubkey)
            .map_err(|_| "Invalid ephemeral public key")?;

        if ephemeral_pub_bytes.len() != 32 {
            return Err("Invalid ephemeral key length".to_string());
        }

        let ephemeral_public = X25519PublicKey::from(
            <[u8; 32]>::try_from(ephemeral_pub_bytes.as_slice()).unwrap()
        );

        // Decode recipient private key
        let recipient_priv_bytes = general_purpose::STANDARD
            .decode(recipient_private_key)
            .map_err(|_| "Invalid recipient private key")?;

        if recipient_priv_bytes.len() != 32 {
            return Err("Invalid private key length".to_string());
        }

        let recipient_private = StaticSecret::from(
            <[u8; 32]>::try_from(recipient_priv_bytes.as_slice()).unwrap()
        );

        // Perform ECDH
        let shared_secret = recipient_private.diffie_hellman(&ephemeral_public);

        // Decrypt
        let key = Key::from_slice(shared_secret.as_bytes());
        let cipher = XChaCha20Poly1305::new(key);

        let nonce_bytes = general_purpose::STANDARD
            .decode(&self.nonce)
            .map_err(|_| "Invalid nonce")?;
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = general_purpose::STANDARD
            .decode(&self.ciphertext)
            .map_err(|_| "Invalid ciphertext")?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_slice())
            .map_err(|_| "Decryption failed")?;

        // Deserialize payload
        serde_json::from_slice(&plaintext)
            .map_err(|e| format!("Failed to deserialize payload: {}", e))
    }
}

/// Generate a random message ID (32 bytes, hex encoded)
fn generate_message_id() -> String {
    let mut rng = rand::thread_rng();
    let mut id = [0u8; 32];
    rng.fill(&mut id);
    hex::encode(id)
}

/// Calculate recipient hint from public key (first 8 bytes, hex encoded)
fn calculate_recipient_hint(public_key: &str) -> String {
    // Decode the base64 public key
    if let Ok(bytes) = general_purpose::STANDARD.decode(public_key) {
        if bytes.len() >= 8 {
            return hex::encode(&bytes[0..8]);
        }
    }
    // Fallback: hash the key and use first 8 bytes
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(public_key.as_bytes());
    let hash = hasher.finalize();
    hex::encode(&hash[0..8])
}

/// Create sealed boxes for multiple recipients
fn create_sealed_boxes_for_recipients(
    payload: &ContentPayload,
    recipient_public_keys: &[String],
    sender_private_key: &str,
) -> Result<Vec<SealedBox>, String> {
    let mut boxes = Vec::new();

    for recipient_key in recipient_public_keys {
        let sealed_box = SealedBox::new(payload, recipient_key, sender_private_key)?;
        boxes.push(sealed_box);
    }

    Ok(boxes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sealed_box_encrypt_decrypt() {
        // Generate test keypair
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut secret_bytes = [0u8; 32];
        rng.fill(&mut secret_bytes);

        let private_key = StaticSecret::from(secret_bytes);
        let public_key = X25519PublicKey::from(&private_key);

        let public_key_b64 = general_purpose::STANDARD.encode(public_key.as_bytes());
        let private_key_b64 = general_purpose::STANDARD.encode(secret_bytes);

        // Create a test payload
        let payload = ContentPayload::Post {
            content: "Hello, encrypted world!".to_string(),
            attachments: None,
        };

        // Create sealed box
        let sealed_box = SealedBox::new(&payload, &public_key_b64, &private_key_b64).unwrap();

        // Decrypt
        let decrypted = sealed_box.decrypt(&private_key_b64).unwrap();

        match decrypted {
            ContentPayload::Post { content, .. } => {
                assert_eq!(content, "Hello, encrypted world!");
            }
            _ => panic!("Wrong payload type"),
        }
    }

    #[test]
    fn test_envelope_with_multiple_recipients() {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Generate sender keypair
        let mut sender_secret = [0u8; 32];
        rng.fill(&mut sender_secret);
        let sender_private = StaticSecret::from(sender_secret);
        let sender_public = X25519PublicKey::from(&sender_private);
        let sender_pub_b64 = general_purpose::STANDARD.encode(sender_public.as_bytes());
        let sender_priv_b64 = general_purpose::STANDARD.encode(sender_secret);

        // Generate 3 recipient keypairs
        let mut recipients = Vec::new();
        for _ in 0..3 {
            let mut secret = [0u8; 32];
            rng.fill(&mut secret);
            let private = StaticSecret::from(secret);
            let public = X25519PublicKey::from(&private);
            recipients.push((
                general_purpose::STANDARD.encode(public.as_bytes()),
                general_purpose::STANDARD.encode(secret),
            ));
        }

        let recipient_pub_keys: Vec<String> = recipients.iter().map(|(p, _)| p.clone()).collect();

        // Create envelope
        let envelope = GossipEnvelope::new_post(
            &sender_pub_b64,
            "Secret message for friends only",
            None,
            &recipient_pub_keys,
            &sender_priv_b64,
        ).unwrap();

        // Each recipient should be able to decrypt
        for (pub_key, priv_key) in &recipients {
            let decrypted = envelope.try_decrypt(pub_key, priv_key);
            assert!(decrypted.is_some());

            match decrypted.unwrap() {
                ContentPayload::Post { content, .. } => {
                    assert_eq!(content, "Secret message for friends only");
                }
                _ => panic!("Wrong payload type"),
            }
        }

        // A non-recipient should NOT be able to decrypt
        let mut outsider_secret = [0u8; 32];
        rng.fill(&mut outsider_secret);
        let outsider_private = StaticSecret::from(outsider_secret);
        let outsider_public = X25519PublicKey::from(&outsider_private);
        let outsider_pub_b64 = general_purpose::STANDARD.encode(outsider_public.as_bytes());
        let outsider_priv_b64 = general_purpose::STANDARD.encode(outsider_secret);

        let decrypted = envelope.try_decrypt(&outsider_pub_b64, &outsider_priv_b64);
        assert!(decrypted.is_none());
    }
}
