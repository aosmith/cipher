// Sealed Box implementation for global mesh encryption
//
// Architecture:
// - GossipEnvelope contains multiple SealedBox instances (one per friend)
// - Each SealedBox can only be decrypted by its intended recipient
// - The `recipient_hint` (first 8 bytes of public key) allows quick filtering
// - Ephemeral keys provide forward secrecy for each message
// - The plaintext inside each box carries an Ed25519 signature binding it to
//   the envelope's claimed sender, so envelopes cannot be forged or re-targeted

use base64::{engine::general_purpose, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use crate::app::database::Database;
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
    /// Full device-to-device sync payload, sealed to the user's own key
    DeviceSync,
}

/// Envelope for gossiped content - contains multiple sealed boxes
/// All nodes receive this, but only intended recipients can decrypt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipEnvelope {
    /// Random ID for deduplication (32 bytes, hex encoded)
    pub message_id: String,
    /// Unix timestamp, rounded down to the hour (only used for purging)
    pub timestamp: i64,
    /// Type hint for the content
    pub content_type: ContentType,
    /// Sender's Ed25519 signing public key. AUTHENTICATED: the payload inside
    /// each sealed box is signed with the matching private key and verified in
    /// try_decrypt(), so this field cannot be spoofed on a decryptable envelope.
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
    /// Device sync data (posts, messages, friends), sealed to the user's OWN
    /// encryption key - both devices derive the same keypair from the recovery
    /// phrase, so only the user's own devices can read it.
    DeviceSync {
        device_id: String,
        data_json: String,
        #[serde(default)]
        sent_at: i64,
    },
}

/// What actually gets encrypted into each SealedBox: the payload plus an
/// Ed25519 signature binding it to the envelope's sender, message_id, and
/// timestamp. Without this, anyone could put any `sender_public_key` on an
/// envelope and impersonate other users - the boxes alone prove nothing about
/// who created them.
#[derive(Debug, Serialize, Deserialize)]
struct SignedPlaintext {
    payload_json: String,
    /// base64 Ed25519 signature over `signing_context(...)` by the private key
    /// matching the envelope's `sender_public_key`
    signature: String,
}

/// Canonical string the payload signature covers. Binds the envelope metadata
/// so message_id/timestamp/sender cannot be swapped on a relayed envelope
/// without invalidating the signature.
fn signing_context(
    message_id: &str,
    timestamp: i64,
    sender_public_key: &str,
    payload_json: &str,
) -> String {
    format!(
        "sealed_v2|{}|{}|{}|{}",
        message_id, timestamp, sender_public_key, payload_json
    )
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

/// Derive the AEAD key from the ECDH result with a KDF instead of using the
/// raw shared secret, binding it to both public keys so a box cannot be
/// re-targeted to a different recipient or ephemeral key.
fn derive_aead_key(
    shared_secret: &x25519_dalek::SharedSecret,
    ephemeral_public: &[u8],
    recipient_public: &[u8],
) -> [u8; 32] {
    let mut ikm = Vec::with_capacity(96);
    ikm.extend_from_slice(shared_secret.as_bytes());
    ikm.extend_from_slice(ephemeral_public);
    ikm.extend_from_slice(recipient_public);
    blake3::derive_key("cipher-social 2026-07 sealed-box v2 aead key", &ikm)
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
        sender_signing_private_key: &str,
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
            &message_id,
            timestamp,
            sender_public_key,
            sender_signing_private_key,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new_community_post(
        sender_public_key: &str,
        community_id: &str,
        community_name: &str,
        content: &str,
        attachments: Option<Vec<MediaAttachmentWithData>>,
        show_in_main_feed: bool,
        member_public_keys: &[String],
        sender_signing_private_key: &str,
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
            &message_id,
            timestamp,
            sender_public_key,
            sender_signing_private_key,
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
        sender_signing_private_key: &str,
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
            &message_id,
            timestamp,
            sender_public_key,
            sender_signing_private_key,
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
        sender_signing_private_key: &str,
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
            &message_id,
            timestamp,
            sender_public_key,
            sender_signing_private_key,
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
        sender_signing_private_key: &str,
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
            &message_id,
            timestamp,
            sender_public_key,
            sender_signing_private_key,
        )?;

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            content_type: ContentType::PostReaction,
            sender_public_key: sender_public_key.to_string(),
            sealed_boxes,
        })
    }

    /// Create a device-sync envelope, sealed to the user's OWN encryption key.
    /// All of a user's devices derive the same keypair from the recovery
    /// phrase, so only the user's own devices can decrypt it - unlike the old
    /// plaintext DeviceSyncResponse, which broadcast the entire database to
    /// the whole mesh.
    pub fn new_device_sync(
        sender_public_key: &str,
        device_id: &str,
        data_json: &str,
        own_encryption_public_key: &str,
        sender_signing_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let now = chrono::Utc::now().timestamp();
        let timestamp = coarse_timestamp(now);

        let payload = ContentPayload::DeviceSync {
            device_id: device_id.to_string(),
            data_json: data_json.to_string(),
            sent_at: now,
        };

        let sealed_boxes = create_sealed_boxes_for_recipients(
            &payload,
            &[own_encryption_public_key.to_string()],
            &message_id,
            timestamp,
            sender_public_key,
            sender_signing_private_key,
        )?;

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            content_type: ContentType::DeviceSync,
            sender_public_key: sender_public_key.to_string(),
            sealed_boxes,
        })
    }

    /// Try to decrypt this envelope for a given recipient.
    ///
    /// Returns the payload only if BOTH hold:
    /// 1. one of the sealed boxes decrypts with our key, AND
    /// 2. the payload signature verifies against the envelope's
    ///    `sender_public_key` over the envelope metadata.
    ///
    /// A decryptable envelope whose signature does not verify is a forgery
    /// attempt (someone stamped another user's key on their own envelope) and
    /// is rejected.
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

            let plaintext = match sealed_box.decrypt_bytes(recipient_private_key) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let signed: SignedPlaintext = match serde_json::from_slice(&plaintext) {
                Ok(s) => s,
                Err(e) => {
                    println!("[SEALED-BOX] Rejecting envelope: not a signed payload: {e}");
                    continue;
                }
            };

            // AUTHENTICITY: the payload must be signed by the key the envelope
            // claims as sender - otherwise anyone could impersonate any user.
            let context = signing_context(
                &self.message_id,
                self.timestamp,
                &self.sender_public_key,
                &signed.payload_json,
            );
            if !Database::verify_signature(&context, &signed.signature, &self.sender_public_key) {
                println!(
                    "[SEALED-BOX] Rejecting envelope: payload signature invalid for claimed sender {}",
                    self.sender_public_key
                );
                continue;
            }

            match serde_json::from_str(&signed.payload_json) {
                Ok(payload) => return Some(payload),
                Err(e) => {
                    println!("[SEALED-BOX] Failed to deserialize verified payload: {e}");
                    continue;
                }
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
    /// Encrypt plaintext bytes for a specific recipient.
    ///
    /// The plaintext is padded with trailing spaces to a size bucket so
    /// ciphertext length doesn't reveal content length - callers must pass
    /// whitespace-tolerant plaintext (JSON).
    pub fn seal_bytes(plaintext: &[u8], recipient_public_key: &str) -> Result<Self, String> {
        let mut padded = plaintext.to_vec();
        padded.resize(padded_len(padded.len()), b' ');

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

        // Perform ECDH; reject low-order/identity points that would produce a
        // non-contributory (attacker-known) shared secret
        let shared_secret = ephemeral_private.diffie_hellman(&recipient_public);
        if !shared_secret.was_contributory() {
            return Err("Recipient public key is a low-order point".to_string());
        }

        // Encrypt with XChaCha20-Poly1305 under a derived key
        let key_bytes = derive_aead_key(
            &shared_secret,
            ephemeral_public.as_bytes(),
            &recipient_pub_bytes,
        );
        let key = Key::from_slice(&key_bytes);
        let cipher = XChaCha20Poly1305::new(key);

        let mut nonce_bytes = [0u8; 24];
        rng.fill(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, padded.as_slice())
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

    /// Decrypt this sealed box to raw plaintext bytes (padded; callers parse
    /// JSON, which ignores the trailing whitespace padding)
    pub fn decrypt_bytes(&self, recipient_private_key: &str) -> Result<Vec<u8>, String> {
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
        let recipient_public = X25519PublicKey::from(&recipient_private);

        // Perform ECDH with the same low-order-point rejection as seal_bytes
        let shared_secret = recipient_private.diffie_hellman(&ephemeral_public);
        if !shared_secret.was_contributory() {
            return Err("Ephemeral public key is a low-order point".to_string());
        }

        // Decrypt under the derived key
        let key_bytes = derive_aead_key(
            &shared_secret,
            &ephemeral_pub_bytes,
            recipient_public.as_bytes(),
        );
        let key = Key::from_slice(&key_bytes);
        let cipher = XChaCha20Poly1305::new(key);

        let nonce_bytes = general_purpose::STANDARD
            .decode(&self.nonce)
            .map_err(|_| "Invalid nonce")?;
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = general_purpose::STANDARD
            .decode(&self.ciphertext)
            .map_err(|_| "Invalid ciphertext")?;

        cipher
            .decrypt(nonce, ciphertext.as_slice())
            .map_err(|_| "Decryption failed".to_string())
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

/// Create sealed boxes for multiple recipients.
/// The payload is serialized and signed ONCE (the signature binds the envelope
/// metadata), then the same signed plaintext is sealed per recipient.
fn create_sealed_boxes_for_recipients(
    payload: &ContentPayload,
    recipient_public_keys: &[String],
    message_id: &str,
    timestamp: i64,
    sender_public_key: &str,
    sender_signing_private_key: &str,
) -> Result<Vec<SealedBox>, String> {
    let payload_json = serde_json::to_string(payload)
        .map_err(|e| format!("Failed to serialize payload: {}", e))?;

    let context = signing_context(message_id, timestamp, sender_public_key, &payload_json);
    let signature = Database::sign_message(&context, sender_signing_private_key)
        .map_err(|e| format!("Failed to sign payload: {}", e))?;

    let signed = SignedPlaintext {
        payload_json,
        signature,
    };
    let plaintext = serde_json::to_string(&signed)
        .map_err(|e| format!("Failed to serialize signed payload: {}", e))?;

    let mut boxes = Vec::new();
    for recipient_key in recipient_public_keys {
        boxes.push(SealedBox::seal_bytes(plaintext.as_bytes(), recipient_key)?);
    }

    Ok(boxes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// X25519 keypair (encryption) as (public_b64, private_b64)
    fn x25519_keypair() -> (String, String) {
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

    /// Ed25519 keypair (signing) as (public_b64, private_b64)
    fn ed25519_keypair() -> (String, String) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut secret = [0u8; 32];
        rng.fill(&mut secret);
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret);
        (
            general_purpose::STANDARD.encode(signing_key.verifying_key().to_bytes()),
            general_purpose::STANDARD.encode(secret),
        )
    }

    #[test]
    fn test_seal_bytes_roundtrip() {
        let (pub_key, priv_key) = x25519_keypair();

        let plaintext = br#"{"hello":"encrypted world"}"#;
        let sealed = SealedBox::seal_bytes(plaintext, &pub_key).unwrap();
        let decrypted = sealed.decrypt_bytes(&priv_key).unwrap();

        // Decrypted output is the plaintext plus whitespace padding
        assert!(decrypted.starts_with(plaintext));
        assert!(decrypted[plaintext.len()..].iter().all(|&b| b == b' '));
        assert_eq!(decrypted.len(), 256); // smallest bucket
    }

    #[test]
    fn test_envelope_with_multiple_recipients() {
        let (sender_pub, sender_priv) = ed25519_keypair();

        // Generate 3 recipient keypairs
        let recipients: Vec<(String, String)> = (0..3).map(|_| x25519_keypair()).collect();
        let recipient_pub_keys: Vec<String> = recipients.iter().map(|(p, _)| p.clone()).collect();

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "test-post-456",
            "Secret message for friends only",
            "test-node-id",
            &[],
            &recipient_pub_keys,
            &sender_priv,
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
        let (outsider_pub, outsider_priv) = x25519_keypair();
        let decrypted = envelope.try_decrypt(&outsider_pub, &outsider_priv);
        assert!(decrypted.is_none());
    }

    #[test]
    fn test_forged_sender_is_rejected() {
        // Attacker signs with their own key but stamps the victim's public key
        // on the envelope. Recipients must reject it.
        let (_attacker_pub, attacker_priv) = ed25519_keypair();
        let (victim_pub, _) = ed25519_keypair();
        let (recipient_pub, recipient_priv) = x25519_keypair();

        let forged = GossipEnvelope::new_post(
            &victim_pub, // claimed sender: the victim
            "forged-post",
            "I definitely wrote this - victim",
            "node",
            &[],
            &[recipient_pub.clone()],
            &attacker_priv, // actually signed by the attacker
        )
        .unwrap();

        assert!(
            forged
                .try_decrypt(&recipient_pub, &recipient_priv)
                .is_none(),
            "envelope claiming another user's key must be rejected"
        );
    }

    #[test]
    fn test_tampered_envelope_metadata_is_rejected() {
        let (sender_pub, sender_priv) = ed25519_keypair();
        let (recipient_pub, recipient_priv) = x25519_keypair();

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "post-1",
            "content",
            "node-1",
            &[],
            &[recipient_pub.clone()],
            &sender_priv,
        )
        .unwrap();

        // Sanity: unmodified envelope decrypts
        assert!(envelope
            .try_decrypt(&recipient_pub, &recipient_priv)
            .is_some());

        // Tampered message_id must be rejected (signature binds it)
        let mut tampered = envelope.clone();
        tampered.message_id = generate_message_id();
        assert!(tampered
            .try_decrypt(&recipient_pub, &recipient_priv)
            .is_none());

        // Tampered timestamp must be rejected
        let mut tampered = envelope.clone();
        tampered.timestamp += 3600;
        assert!(tampered
            .try_decrypt(&recipient_pub, &recipient_priv)
            .is_none());

        // Swapped sender key must be rejected
        let (other_pub, _) = ed25519_keypair();
        let mut tampered = envelope.clone();
        tampered.sender_public_key = other_pub;
        assert!(tampered
            .try_decrypt(&recipient_pub, &recipient_priv)
            .is_none());
    }

    #[test]
    fn test_device_sync_envelope_roundtrip() {
        let (sender_pub, sender_priv) = ed25519_keypair();
        // Device sync seals to the user's OWN encryption key
        let (own_enc_pub, own_enc_priv) = x25519_keypair();

        let data = r#"{"posts":[],"messages":[],"friends":[]}"#;
        let envelope = GossipEnvelope::new_device_sync(
            &sender_pub,
            "device-abc",
            data,
            &own_enc_pub,
            &sender_priv,
        )
        .unwrap();

        match envelope.try_decrypt(&own_enc_pub, &own_enc_priv) {
            Some(ContentPayload::DeviceSync {
                device_id,
                data_json,
                ..
            }) => {
                assert_eq!(device_id, "device-abc");
                assert_eq!(data_json, data);
            }
            _ => panic!("Expected DeviceSync payload"),
        }

        // Another user cannot read it
        let (other_pub, other_priv) = x25519_keypair();
        assert!(envelope.try_decrypt(&other_pub, &other_priv).is_none());
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
        let (sender_pub, sender_priv) = ed25519_keypair();
        let (recipient_pub, recipient_priv) = x25519_keypair();

        // Two posts with different content lengths that land in the same
        // padding bucket must produce identically sized ciphertexts.
        let make = |content: String| {
            GossipEnvelope::new_post(
                &sender_pub,
                "post",
                &content,
                "node",
                &[],
                &[recipient_pub.clone()],
                &sender_priv,
            )
            .unwrap()
        };
        let short = make("a".repeat(500));
        let long = make("b".repeat(600));

        assert_eq!(
            short.sealed_boxes[0].ciphertext.len(),
            long.sealed_boxes[0].ciphertext.len(),
            "same-bucket payloads must be indistinguishable by size"
        );

        // Padded payloads must still decrypt cleanly
        match short.try_decrypt(&recipient_pub, &recipient_priv) {
            Some(ContentPayload::Post { content, .. }) => {
                assert_eq!(content, "a".repeat(500));
            }
            _ => panic!("Wrong payload type"),
        }
    }

    #[test]
    fn test_envelope_timestamp_is_coarse_and_sent_at_precise() {
        let (sender_pub, sender_priv) = ed25519_keypair();
        let (pub_key, priv_key) = x25519_keypair();

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
