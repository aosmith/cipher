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

use crate::app::types::{BlobReference, MediaAttachmentWithData, SqliteUuid};

/// Content types that can be sealed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    Post,
    DirectMessage,
    FriendRequest,
    FriendAccepted,
    KeyRotation,
    CommunityPost,
    CommunityMemberAdded,
    PostComment,
    PostReaction,
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
///
/// Variants carry a `sent_at` timestamp INSIDE the ciphertext: the envelope's
/// outer `timestamp` is coarsened to the hour (it only exists for purging), so
/// precise send times are not visible to mesh observers. `sent_at` defaults to
/// 0 when decoding messages from older clients - fall back to the envelope
/// timestamp in that case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentPayload {
    Post {
        post_id: String,
        content: String,
        node_id: String,               // Sender's NodeId for blob fetching
        blob_refs: Vec<BlobReference>, // Attachments stored as blobs
        #[serde(default)]
        sent_at: i64,
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
    /// A post in a community
    CommunityPost {
        community_id: String,
        community_name: String,
        content: String,
        attachments: Option<Vec<MediaAttachmentWithData>>,
        show_in_main_feed: bool,
        #[serde(default)]
        sent_at: i64,
    },
    /// Notification that a new member joined a community
    CommunityMemberAdded {
        community_id: String,
        community_name: String,
        new_member_public_key: String,
        new_member_display_name: String,
    },
    /// A comment on a post
    PostComment {
        comment_id: String,
        post_id: String,
        content: String,
        parent_comment_id: Option<String>,
        #[serde(default)]
        sent_at: i64,
    },
    /// A reaction on a post
    PostReaction {
        post_id: String,
        emoji: String,
        action: String, // "add" or "remove"
        #[serde(default)]
        sent_at: i64,
    },
}

/// Envelope timestamps are only used for purging old content, so round them
/// down to the hour: a precise plaintext timestamp hands long-term observers
/// exact per-user activity times. Precise time travels encrypted in `sent_at`.
fn coarse_timestamp(now: i64) -> i64 {
    const GRANULARITY: i64 = 3600;
    now - now.rem_euclid(GRANULARITY)
}

/// Bucket size for plaintext padding: ciphertext length would otherwise reveal
/// content length (a reaction vs. a long post) to every mesh observer.
/// Small payloads land in one 256-byte bucket, mid-size round up to the next
/// power of two, large (attachment-bearing) payloads to a 4 KiB multiple.
fn padded_len(len: usize) -> usize {
    if len <= 256 {
        256
    } else if len <= 4096 {
        len.next_power_of_two()
    } else {
        len.div_ceil(4096) * 4096
    }
}

impl GossipEnvelope {
    /// Create a new envelope for a post, encrypted for all friends
    pub fn new_post(
        sender_public_key: &str,
        post_id: &str,
        content: &str,
        node_id: &str,
        blob_refs: &[BlobReference],
        friend_public_keys: &[String],
        sender_encryption_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let now = chrono::Utc::now().timestamp();
        let timestamp = coarse_timestamp(now);

        let payload = ContentPayload::Post {
            post_id: post_id.to_string(),
            content: content.to_string(),
            node_id: node_id.to_string(),
            blob_refs: blob_refs.to_vec(),
            sent_at: now,
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

    /// Create a new envelope for a community post, encrypted for all community members
    pub fn new_community_post(
        sender_public_key: &str,
        community_id: &str,
        community_name: &str,
        content: &str,
        attachments: Option<Vec<MediaAttachmentWithData>>,
        show_in_main_feed: bool,
        member_public_keys: &[String],
        sender_encryption_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let now = chrono::Utc::now().timestamp();
        let timestamp = coarse_timestamp(now);

        let payload = ContentPayload::CommunityPost {
            community_id: community_id.to_string(),
            community_name: community_name.to_string(),
            content: content.to_string(),
            attachments,
            show_in_main_feed,
            sent_at: now,
        };

        let sealed_boxes = create_sealed_boxes_for_recipients(
            &payload,
            member_public_keys,
            sender_encryption_private_key,
        )?;

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            content_type: ContentType::CommunityPost,
            sender_public_key: sender_public_key.to_string(),
            sealed_boxes,
        })
    }

    /// Create a new envelope to notify members about a new community member
    pub fn new_community_member_added(
        sender_public_key: &str,
        community_id: &str,
        community_name: &str,
        new_member_public_key: &str,
        new_member_display_name: &str,
        member_public_keys: &[String],
        sender_encryption_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let timestamp = coarse_timestamp(chrono::Utc::now().timestamp());

        let payload = ContentPayload::CommunityMemberAdded {
            community_id: community_id.to_string(),
            community_name: community_name.to_string(),
            new_member_public_key: new_member_public_key.to_string(),
            new_member_display_name: new_member_display_name.to_string(),
        };

        let sealed_boxes = create_sealed_boxes_for_recipients(
            &payload,
            member_public_keys,
            sender_encryption_private_key,
        )?;

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            content_type: ContentType::CommunityMemberAdded,
            sender_public_key: sender_public_key.to_string(),
            sealed_boxes,
        })
    }

    /// Create a new envelope for a post comment, encrypted for all friends
    pub fn new_post_comment(
        sender_public_key: &str,
        comment_id: &str,
        post_id: &str,
        content: &str,
        parent_comment_id: Option<&str>,
        friend_public_keys: &[String],
        sender_encryption_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let now = chrono::Utc::now().timestamp();
        let timestamp = coarse_timestamp(now);

        let payload = ContentPayload::PostComment {
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            content: content.to_string(),
            parent_comment_id: parent_comment_id.map(|s| s.to_string()),
            sent_at: now,
        };

        let sealed_boxes = create_sealed_boxes_for_recipients(
            &payload,
            friend_public_keys,
            sender_encryption_private_key,
        )?;

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            content_type: ContentType::PostComment,
            sender_public_key: sender_public_key.to_string(),
            sealed_boxes,
        })
    }

    /// Create a new envelope for a post reaction, encrypted for all friends
    pub fn new_post_reaction(
        sender_public_key: &str,
        post_id: &str,
        emoji: &str,
        action: &str,
        friend_public_keys: &[String],
        sender_encryption_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let now = chrono::Utc::now().timestamp();
        let timestamp = coarse_timestamp(now);

        let payload = ContentPayload::PostReaction {
            post_id: post_id.to_string(),
            emoji: emoji.to_string(),
            action: action.to_string(),
            sent_at: now,
        };

        let sealed_boxes = create_sealed_boxes_for_recipients(
            &payload,
            friend_public_keys,
            sender_encryption_private_key,
        )?;

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            content_type: ContentType::PostReaction,
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
        self.sealed_boxes
            .iter()
            .any(|sb| sb.recipient_hint == our_hint)
    }
}

impl SealedBox {
    /// Create a new sealed box for a specific recipient
    pub fn new(
        payload: &ContentPayload,
        recipient_public_key: &str,
        _sender_private_key: &str,
    ) -> Result<Self, String> {
        // Serialize the payload, then pad with trailing spaces to a size bucket
        // so ciphertext length doesn't reveal content length. JSON parsers skip
        // trailing whitespace, so decryption needs no changes and stays
        // compatible with envelopes from older clients.
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;
        let mut plaintext = payload_json.into_bytes();
        plaintext.resize(padded_len(plaintext.len()), b' ');

        // Decode recipient's public key
        let recipient_pub_bytes = general_purpose::STANDARD
            .decode(recipient_public_key)
            .map_err(|_| "Invalid recipient public key")?;

        if recipient_pub_bytes.len() != 32 {
            return Err("Invalid recipient key length".to_string());
        }

        let recipient_public =
            X25519PublicKey::from(<[u8; 32]>::try_from(recipient_pub_bytes.as_slice()).unwrap());

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
            .encrypt(nonce, plaintext.as_slice())
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

        let ephemeral_public =
            X25519PublicKey::from(<[u8; 32]>::try_from(ephemeral_pub_bytes.as_slice()).unwrap());

        // Decode recipient private key
        let recipient_priv_bytes = general_purpose::STANDARD
            .decode(recipient_private_key)
            .map_err(|_| "Invalid recipient private key")?;

        if recipient_priv_bytes.len() != 32 {
            return Err("Invalid private key length".to_string());
        }

        let recipient_private =
            StaticSecret::from(<[u8; 32]>::try_from(recipient_priv_bytes.as_slice()).unwrap());

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
    use sha2::{Digest, Sha256};
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
            post_id: "test-post-123".to_string(),
            content: "Hello, encrypted world!".to_string(),
            node_id: "test-node-id".to_string(),
            blob_refs: vec![],
            sent_at: 1_700_000_123,
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
            "test-post-456",
            "Secret message for friends only",
            "test-node-id",
            &[],
            &recipient_pub_keys,
            &sender_priv_b64,
        )
        .unwrap();

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

    fn test_keypair() -> (String, String) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut secret = [0u8; 32];
        rng.fill(&mut secret);
        let private = StaticSecret::from(secret);
        let public = X25519PublicKey::from(&private);
        (
            general_purpose::STANDARD.encode(public.as_bytes()),
            general_purpose::STANDARD.encode(secret),
        )
    }

    #[test]
    fn test_padded_len_buckets() {
        assert_eq!(padded_len(0), 256);
        assert_eq!(padded_len(1), 256);
        assert_eq!(padded_len(256), 256);
        assert_eq!(padded_len(257), 512);
        assert_eq!(padded_len(1000), 1024);
        assert_eq!(padded_len(4096), 4096);
        assert_eq!(padded_len(4097), 8192);
        assert_eq!(padded_len(10000), 12288);
    }

    #[test]
    fn test_ciphertext_length_hides_content_length() {
        let (pub_key, priv_key) = test_keypair();

        // Two payloads with very different content lengths that land in the
        // same bucket must produce identically sized ciphertexts.
        let short = ContentPayload::PostReaction {
            post_id: "p".to_string(),
            emoji: "x".to_string(),
            action: "add".to_string(),
            sent_at: 1,
        };
        let long = ContentPayload::PostReaction {
            post_id: "p".repeat(60),
            emoji: "x".repeat(30),
            action: "remove".to_string(),
            sent_at: 1_700_000_000,
        };

        let short_box = SealedBox::new(&short, &pub_key, &priv_key).unwrap();
        let long_box = SealedBox::new(&long, &pub_key, &priv_key).unwrap();
        assert_eq!(
            short_box.ciphertext.len(),
            long_box.ciphertext.len(),
            "same-bucket payloads must be indistinguishable by size"
        );

        // Padded payloads must still decrypt cleanly (trailing whitespace is
        // ignored by the JSON parser).
        match short_box.decrypt(&priv_key).unwrap() {
            ContentPayload::PostReaction { action, .. } => assert_eq!(action, "add"),
            _ => panic!("Wrong payload type"),
        }
    }

    #[test]
    fn test_envelope_timestamp_is_coarse_and_sent_at_precise() {
        let (pub_key, priv_key) = test_keypair();
        let (sender_pub, sender_priv) = test_keypair();

        let before = chrono::Utc::now().timestamp();
        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "post-1",
            "content",
            "node-1",
            &[],
            &[pub_key.clone()],
            &sender_priv,
        )
        .unwrap();
        let after = chrono::Utc::now().timestamp();

        // Outer timestamp is rounded down to the hour...
        assert_eq!(envelope.timestamp % 3600, 0);
        assert!(envelope.timestamp <= after && envelope.timestamp > before - 3600);

        // ...while the precise time is inside the ciphertext.
        match envelope.try_decrypt(&pub_key, &priv_key).unwrap() {
            ContentPayload::Post { sent_at, .. } => {
                assert!((before..=after).contains(&sent_at));
            }
            _ => panic!("Wrong payload type"),
        }
    }

    #[test]
    fn test_decode_payload_without_sent_at_defaults_to_zero() {
        // Envelopes from older clients have no sent_at field.
        let legacy_json = r#"{"Post":{"post_id":"p","content":"c","node_id":"n","blob_refs":[]}}"#;
        match serde_json::from_str::<ContentPayload>(legacy_json).unwrap() {
            ContentPayload::Post { sent_at, .. } => assert_eq!(sent_at, 0),
            _ => panic!("Wrong payload type"),
        }
    }
}
