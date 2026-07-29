// Sealed Box implementation for global mesh encryption
//
// Architecture (v3):
// - The payload is serialized, signed, padded to a size bucket, and encrypted
//   ONCE under a random 32-byte content key (XChaCha20-Poly1305)
// - Each recipient gets a small SealedBox that wraps just the content key
//   (ephemeral X25519 ECDH), so envelope size is O(1) payload + O(n) tiny boxes
// - Recipients TRIAL-DECRYPT the key boxes: there are no recipient hints on
//   the wire, so observers cannot tell who a message is addressed to
// - Dummy key boxes pad the box count to a power of two, so the exact
//   recipient count (friend count) is not visible either
// - The sender's identity travels INSIDE the ciphertext (SignedPlaintext),
//   authenticated by an Ed25519 signature binding the envelope metadata -
//   a wire observer sees only: size bucket, box count bucket, coarse hour
//
// What a passive mesh observer learns per envelope: nothing about sender,
// recipients, or content type - only approximate size and the hour it was sent.

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

/// Envelope for gossiped content.
/// All nodes receive this; only intended recipients can decrypt. The wire
/// format deliberately carries NO sender, recipient, or content-type metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipEnvelope {
    /// Random ID for deduplication / replay protection (32 bytes, hex encoded)
    pub message_id: String,
    /// Unix timestamp, rounded down to the hour (only used for purging)
    pub timestamp: i64,
    /// The payload (a padded SignedPlaintext), encrypted once under a random
    /// content key: base64(nonce_24 || xchacha20poly1305_ciphertext)
    pub encrypted_payload: String,
    /// One box per recipient wrapping the content key, padded with dummy
    /// boxes to a power of two so recipient count is not observable
    pub sealed_boxes: Vec<SealedBox>,
}

/// A sealed box wrapping the envelope's content key for one recipient.
/// Recipients trial-decrypt every box - there is deliberately no recipient
/// identifier, so a box is indistinguishable from a dummy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBox {
    /// Ephemeral X25519 public key (for ECDH) - base64 encoded
    pub ephemeral_pubkey: String,
    /// Nonce for XChaCha20-Poly1305 (24 bytes) - base64 encoded
    pub nonce: String,
    /// Encrypted content key - base64 encoded
    pub ciphertext: String,
}

/// The decrypted, authenticated result of opening an envelope
#[derive(Debug)]
pub struct DecryptedEnvelope {
    /// The sender's Ed25519 public key. AUTHENTICATED: the payload signature
    /// was verified against this key during decryption.
    pub sender_public_key: String,
    pub payload: ContentPayload,
}

/// The actual content inside a sealed envelope
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
        /// True when this is a catch-up re-send (a friend came back online and
        /// asked for posts they missed). Backfilled posts render into the feed
        /// silently - no toast - so reconnecting doesn't spam notifications.
        #[serde(default)]
        is_backfill: bool,
    },
    DirectMessage {
        #[serde(default)]
        message_id: String,
        content: String,
        thread_id: Option<SqliteUuid>,
        #[serde(default)]
        sent_at: i64,
    },
    /// Friend request, sealed to the target's encryption key (from their
    /// invite QR). Sender identity/authenticity comes from the envelope
    /// signature; carries everything the target needs to connect back.
    FriendRequest {
        display_name: String,
        encryption_public_key: String,
        node_id: String,
        relay_url: String,
        #[serde(default)]
        sent_at: i64,
    },
    /// Friend request acceptance, sealed to the requester's encryption key
    FriendAccepted {
        display_name: String,
        encryption_public_key: String,
        node_id: String,
        relay_url: String,
        #[serde(default)]
        sent_at: i64,
    },
    /// Announcement of the sender's new rotating pre-key. Sealed to friends so
    /// they seal future content to this key instead of the static identity key
    /// (forward secrecy). The signature is verified against the envelope's
    /// authenticated sender identity key.
    KeyRotation {
        /// New X25519 pre-key public (base64)
        prekey_public: String,
        /// Ed25519 signature over
        /// prekey_signing_context(prekey_public, prekey_created_at) by the
        /// sender's identity key (base64)
        signature: String,
        /// Unix seconds the pre-key was created. Signed alongside the key so a
        /// replayed older rotation can be detected and rejected (monotonic
        /// pre-keys). 0 / absent means a pre-v2 sender.
        #[serde(default)]
        prekey_created_at: i64,
        #[serde(default)]
        sent_at: i64,
    },
    /// A post in a community
    CommunityPost {
        community_id: String,
        community_name: String,
        content: String,
        /// Legacy embedded attachments from older senders. Kept so old
        /// envelopes still deserialize - new senders always send None and
        /// use `blob_refs` instead (embedded bytes exceeded the gossip cap).
        attachments: Option<Vec<MediaAttachmentWithData>>,
        /// Sender's NodeId for blob fetching (empty from older senders)
        #[serde(default)]
        node_id: String,
        /// Attachments stored as encrypted blobs, same as Post
        #[serde(default)]
        blob_refs: Vec<BlobReference>,
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
    /// Catch-up request sent to a friend when they come back online. Gossip is
    /// fire-and-forget, so posts made while we were offline are otherwise lost;
    /// this asks the friend to re-send their posts authored since `since`
    /// (unix seconds). Sealed to the friend and answered only for accepted
    /// friendships. The response is ordinary sealed Post envelopes flagged
    /// is_backfill, deduplicated by post id on receipt.
    FriendSyncRequest {
        since: i64,
        #[serde(default)]
        sent_at: i64,
    },
    /// Presence announcement, sealed to friends + the user's own devices.
    /// Carries profile data and network addresses (including IPs), which is
    /// why it must never travel in plaintext. The sender's user_id is derived
    /// from the authenticated sender public key, not carried here.
    Presence {
        device_id: String,
        /// serde-serialized iroh::EndpointAddr (kept as JSON so this module
        /// stays transport-agnostic)
        node_addr_json: String,
        /// X25519 encryption public key for sealed envelopes
        encryption_public_key: Option<String>,
        display_name: String,
        bio: String,
        profile_picture: String,
        /// Profile signature (display_name|bio|profile_picture), kept for the
        /// profile-tamper checks in the presence handler
        profile_signature: Option<String>,
        /// Current rotating pre-key (base64 X25519 public), piggybacked so
        /// friends catch up on rotations even if a KeyRotation envelope was
        /// lost. Signature is over
        /// prekey_signing_context(prekey_public, prekey_created_at).
        #[serde(default)]
        prekey_public: Option<String>,
        #[serde(default)]
        prekey_signature: Option<String>,
        /// Creation time of the advertised pre-key (see KeyRotation). 0 /
        /// absent means a pre-v2 sender.
        #[serde(default)]
        prekey_created_at: i64,
        #[serde(default)]
        sent_at: i64,
    },
}

/// Canonical string an identity key signs to authenticate a rotating pre-key.
/// Verified against the sender's authenticated identity key on receipt.
///
/// v2 binds the pre-key's creation time into the signature. Without it, a
/// signature over the bare key bytes stays valid forever, so a recorded old
/// rotation announcement could be replayed to roll a friend's pre-key BACKWARD
/// onto a key whose private half they may already have deleted (or that the
/// attacker recovered). Receivers refuse any advertised pre-key whose
/// created_at does not advance past the one they already hold.
pub fn prekey_signing_context(prekey_public: &str, created_at: i64) -> String {
    format!("prekey_v2|{}|{}", created_at, prekey_public)
}

/// Pre-v2 signing context: signature over the key bytes alone, with no
/// monotonic binding.
///
/// TRANSITION ONLY. Peers running older builds sign and advertise pre-keys
/// without a creation timestamp; refusing them outright would break sealing to
/// those peers once their advertised key ages out. We therefore still verify a
/// v1 signature when the announcement carries no created_at, and mark such
/// pre-keys with our local receipt time (which always advances, so v1 peers get
/// no rollback protection). Envelope-level defences still apply to those
/// announcements: message_ids are persistently deduped and envelopes older than
/// 7 days are dropped, so a v1 replay must be both fresh and unseen.
/// Remove this path once all peers are on v2.
pub fn prekey_signing_context_v1(prekey_public: &str) -> String {
    format!("prekey_v1|{}", prekey_public)
}

/// What actually gets encrypted into the payload: the payload JSON, the
/// sender's identity, and an Ed25519 signature binding both to the envelope's
/// message_id and timestamp. Keeping the sender INSIDE the ciphertext means a
/// wire observer cannot attribute envelopes to users, and the signature means
/// a recipient cannot be fooled about who wrote the payload.
#[derive(Debug, Serialize, Deserialize)]
struct SignedPlaintext {
    payload_json: String,
    /// Sender's Ed25519 signing public key - base64 encoded
    sender_public_key: String,
    /// base64 Ed25519 signature over `signing_context(...)` by the private key
    /// matching `sender_public_key`
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

/// Number of key boxes to put on an envelope for `real` recipients: the next
/// power of two, minimum 2, so the exact recipient count (friend count) is
/// not observable. The extras are dummy boxes indistinguishable from real ones.
fn padded_box_count(real: usize) -> usize {
    real.max(1).next_power_of_two().max(2)
}

/// Derive the AEAD key for a key box from the ECDH result with a KDF instead
/// of using the raw shared secret, binding it to both public keys so a box
/// cannot be re-targeted to a different recipient or ephemeral key.
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
    /// Seal a payload for a set of recipients. This is the single path all
    /// content takes onto the wire: sign, pad, encrypt once under a random
    /// content key, then wrap the key for each recipient (plus dummies).
    pub fn seal(
        payload: &ContentPayload,
        recipient_public_keys: &[String],
        sender_public_key: &str,
        sender_signing_private_key: &str,
    ) -> Result<Self, String> {
        let message_id = generate_message_id();
        let timestamp = coarse_timestamp(chrono::Utc::now().timestamp());

        // Serialize and sign the payload, binding the envelope metadata
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| format!("Failed to serialize payload: {}", e))?;
        let context = signing_context(&message_id, timestamp, sender_public_key, &payload_json);
        let signature = Database::sign_message(&context, sender_signing_private_key)
            .map_err(|e| format!("Failed to sign payload: {}", e))?;
        let signed = SignedPlaintext {
            payload_json,
            sender_public_key: sender_public_key.to_string(),
            signature,
        };
        let mut plaintext = serde_json::to_vec(&signed)
            .map_err(|e| format!("Failed to serialize signed payload: {}", e))?;
        plaintext.resize(padded_len(plaintext.len()), b' ');

        // Encrypt the payload ONCE under a random content key
        let mut rng = rand::thread_rng();
        let mut content_key = [0u8; 32];
        rng.fill(&mut content_key);
        let mut nonce_bytes = [0u8; 24];
        rng.fill(&mut nonce_bytes);
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&content_key));
        let ciphertext = cipher
            .encrypt(XNonce::from_slice(&nonce_bytes), plaintext.as_slice())
            .map_err(|_| "Payload encryption failed")?;
        let mut blob = nonce_bytes.to_vec();
        blob.extend_from_slice(&ciphertext);

        // Wrap the content key for each recipient...
        let mut sealed_boxes = Vec::new();
        for recipient_key in recipient_public_keys {
            sealed_boxes.push(SealedBox::seal_bytes(&content_key, recipient_key)?);
        }
        // ...plus dummy boxes (a random key wrapped for a throwaway recipient,
        // indistinguishable from a real box) so box count doesn't reveal the
        // exact recipient count
        while sealed_boxes.len() < padded_box_count(recipient_public_keys.len()) {
            let mut dummy_key = [0u8; 32];
            rng.fill(&mut dummy_key);
            let mut throwaway_secret = [0u8; 32];
            rng.fill(&mut throwaway_secret);
            let throwaway_public = X25519PublicKey::from(&StaticSecret::from(throwaway_secret));
            let throwaway_b64 = general_purpose::STANDARD.encode(throwaway_public.as_bytes());
            sealed_boxes.push(SealedBox::seal_bytes(&dummy_key, &throwaway_b64)?);
        }

        Ok(GossipEnvelope {
            message_id,
            timestamp,
            encrypted_payload: general_purpose::STANDARD.encode(blob),
            sealed_boxes,
        })
    }

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
        let payload = ContentPayload::Post {
            post_id: post_id.to_string(),
            content: content.to_string(),
            node_id: node_id.to_string(),
            blob_refs: blob_refs.to_vec(),
            sent_at: chrono::Utc::now().timestamp(),
            is_backfill: false,
        };
        Self::seal(
            &payload,
            friend_public_keys,
            sender_public_key,
            sender_signing_private_key,
        )
    }

    /// Create a new envelope for a community post, encrypted for all community members
    #[allow(clippy::too_many_arguments)]
    pub fn new_community_post(
        sender_public_key: &str,
        community_id: &str,
        community_name: &str,
        content: &str,
        node_id: &str,
        blob_refs: &[BlobReference],
        show_in_main_feed: bool,
        member_public_keys: &[String],
        sender_signing_private_key: &str,
    ) -> Result<Self, String> {
        let payload = ContentPayload::CommunityPost {
            community_id: community_id.to_string(),
            community_name: community_name.to_string(),
            content: content.to_string(),
            attachments: None,
            node_id: node_id.to_string(),
            blob_refs: blob_refs.to_vec(),
            show_in_main_feed,
            sent_at: chrono::Utc::now().timestamp(),
        };
        Self::seal(
            &payload,
            member_public_keys,
            sender_public_key,
            sender_signing_private_key,
        )
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
        let payload = ContentPayload::CommunityMemberAdded {
            community_id: community_id.to_string(),
            community_name: community_name.to_string(),
            new_member_public_key: new_member_public_key.to_string(),
            new_member_display_name: new_member_display_name.to_string(),
        };
        Self::seal(
            &payload,
            member_public_keys,
            sender_public_key,
            sender_signing_private_key,
        )
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
        let payload = ContentPayload::PostComment {
            comment_id: comment_id.to_string(),
            post_id: post_id.to_string(),
            content: content.to_string(),
            parent_comment_id: parent_comment_id.map(|s| s.to_string()),
            sent_at: chrono::Utc::now().timestamp(),
        };
        Self::seal(
            &payload,
            friend_public_keys,
            sender_public_key,
            sender_signing_private_key,
        )
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
        let payload = ContentPayload::PostReaction {
            post_id: post_id.to_string(),
            emoji: emoji.to_string(),
            action: action.to_string(),
            sent_at: chrono::Utc::now().timestamp(),
        };
        Self::seal(
            &payload,
            friend_public_keys,
            sender_public_key,
            sender_signing_private_key,
        )
    }

    /// Create a device-sync envelope, sealed to the user's OWN encryption key.
    /// All of a user's devices derive the same keypair from the recovery
    /// phrase, so only the user's own devices can decrypt it.
    pub fn new_device_sync(
        sender_public_key: &str,
        device_id: &str,
        data_json: &str,
        own_encryption_public_key: &str,
        sender_signing_private_key: &str,
    ) -> Result<Self, String> {
        let payload = ContentPayload::DeviceSync {
            device_id: device_id.to_string(),
            data_json: data_json.to_string(),
            sent_at: chrono::Utc::now().timestamp(),
        };
        Self::seal(
            &payload,
            &[own_encryption_public_key.to_string()],
            sender_public_key,
            sender_signing_private_key,
        )
    }

    /// Try to decrypt this envelope with our encryption private key.
    ///
    /// Trial-decrypts every key box (there are no recipient hints on the wire),
    /// then opens the payload and verifies the sender signature. Returns the
    /// payload with its AUTHENTICATED sender only if both decryption and
    /// signature verification succeed.
    pub fn try_decrypt(&self, recipient_private_key: &str) -> Option<DecryptedEnvelope> {
        let payload_blob = general_purpose::STANDARD
            .decode(&self.encrypted_payload)
            .ok()?;
        if payload_blob.len() < 25 {
            return None;
        }
        let (nonce_bytes, payload_ct) = payload_blob.split_at(24);

        for sealed_box in &self.sealed_boxes {
            // Trial decryption: most boxes are for other recipients (or
            // dummies) and fail the AEAD tag check - that's the design.
            let Ok(key_bytes) = sealed_box.decrypt_bytes(recipient_private_key) else {
                continue;
            };
            if key_bytes.len() != 32 {
                continue;
            }

            let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
            let Ok(plaintext) = cipher.decrypt(XNonce::from_slice(nonce_bytes), payload_ct) else {
                continue;
            };

            let signed: SignedPlaintext = match serde_json::from_slice(&plaintext) {
                Ok(s) => s,
                Err(e) => {
                    println!("[SEALED-BOX] Rejecting envelope: not a signed payload: {e}");
                    continue;
                }
            };

            // AUTHENTICITY: the payload must be signed by the key it claims
            // as sender - otherwise anyone could impersonate any user.
            let context = signing_context(
                &self.message_id,
                self.timestamp,
                &signed.sender_public_key,
                &signed.payload_json,
            );
            if !Database::verify_signature(&context, &signed.signature, &signed.sender_public_key) {
                println!(
                    "[SEALED-BOX] Rejecting envelope: payload signature invalid for claimed sender {}",
                    signed.sender_public_key
                );
                continue;
            }

            match serde_json::from_str(&signed.payload_json) {
                Ok(payload) => {
                    return Some(DecryptedEnvelope {
                        sender_public_key: signed.sender_public_key,
                        payload,
                    })
                }
                Err(e) => {
                    println!("[SEALED-BOX] Failed to deserialize verified payload: {e}");
                    continue;
                }
            }
        }

        None
    }
}

impl SealedBox {
    /// Encrypt plaintext bytes for a specific recipient (used to wrap the
    /// 32-byte content key, so all boxes are the same size by construction -
    /// no padding needed).
    pub fn seal_bytes(plaintext: &[u8], recipient_public_key: &str) -> Result<Self, String> {
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
            .encrypt(nonce, plaintext)
            .map_err(|_| "Encryption failed")?;

        Ok(SealedBox {
            ephemeral_pubkey: general_purpose::STANDARD.encode(ephemeral_public.as_bytes()),
            nonce: general_purpose::STANDARD.encode(nonce_bytes),
            ciphertext: general_purpose::STANDARD.encode(ciphertext),
        })
    }

    /// Decrypt this sealed box to raw plaintext bytes
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

        let plaintext = [42u8; 32];
        let sealed = SealedBox::seal_bytes(&plaintext, &pub_key).unwrap();
        let decrypted = sealed.decrypt_bytes(&priv_key).unwrap();
        assert_eq!(decrypted, plaintext);

        // Wrong key fails the AEAD tag check
        let (_, other_priv) = x25519_keypair();
        assert!(sealed.decrypt_bytes(&other_priv).is_err());
    }

    #[test]
    fn test_envelope_with_multiple_recipients() {
        let (sender_pub, sender_priv) = ed25519_keypair();

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

        // Each recipient should be able to decrypt, and the authenticated
        // sender comes back with the payload
        for (_, priv_key) in &recipients {
            let decrypted = envelope.try_decrypt(priv_key).expect("should decrypt");
            assert_eq!(decrypted.sender_public_key, sender_pub);

            match decrypted.payload {
                ContentPayload::Post { content, .. } => {
                    assert_eq!(content, "Secret message for friends only");
                }
                _ => panic!("Wrong payload type"),
            }
        }

        // A non-recipient should NOT be able to decrypt
        let (_, outsider_priv) = x25519_keypair();
        assert!(envelope.try_decrypt(&outsider_priv).is_none());
    }

    #[test]
    fn test_no_recipient_metadata_on_wire() {
        let (sender_pub, sender_priv) = ed25519_keypair();
        let recipients: Vec<String> = (0..3).map(|_| x25519_keypair().0).collect();

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "p",
            "content",
            "n",
            &[],
            &recipients,
            &sender_priv,
        )
        .unwrap();

        // The serialized wire format must not contain the sender key or any
        // recipient key material
        let wire = serde_json::to_string(&envelope).unwrap();
        assert!(!wire.contains(&sender_pub[..16]));
        for r in &recipients {
            assert!(!wire.contains(&r[..16]));
        }

        // Box count is padded to a power of two: 3 recipients -> 4 boxes,
        // indistinguishable from an envelope with 4 real recipients
        assert_eq!(envelope.sealed_boxes.len(), 4);
    }

    #[test]
    fn test_box_count_hides_recipient_count() {
        assert_eq!(padded_box_count(1), 2);
        assert_eq!(padded_box_count(2), 2);
        assert_eq!(padded_box_count(3), 4);
        assert_eq!(padded_box_count(4), 4);
        assert_eq!(padded_box_count(5), 8);
        assert_eq!(padded_box_count(9), 16);
    }

    #[test]
    fn test_forged_sender_is_rejected() {
        // Attacker signs with their own key but claims the victim's public
        // key as sender. Recipients must reject it.
        let (_attacker_pub, attacker_priv) = ed25519_keypair();
        let (victim_pub, _) = ed25519_keypair();
        let (recipient_pub, recipient_priv) = x25519_keypair();

        let forged = GossipEnvelope::new_post(
            &victim_pub, // claimed sender: the victim
            "forged-post",
            "I definitely wrote this - victim",
            "node",
            &[],
            &[recipient_pub],
            &attacker_priv, // actually signed by the attacker
        )
        .unwrap();

        assert!(
            forged.try_decrypt(&recipient_priv).is_none(),
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
            &[recipient_pub],
            &sender_priv,
        )
        .unwrap();

        // Sanity: unmodified envelope decrypts
        assert!(envelope.try_decrypt(&recipient_priv).is_some());

        // Tampered message_id must be rejected (signature binds it)
        let mut tampered = envelope.clone();
        tampered.message_id = generate_message_id();
        assert!(tampered.try_decrypt(&recipient_priv).is_none());

        // Tampered timestamp must be rejected
        let mut tampered = envelope.clone();
        tampered.timestamp += 3600;
        assert!(tampered.try_decrypt(&recipient_priv).is_none());

        // Tampered payload ciphertext must be rejected (AEAD tag check)
        let mut tampered = envelope.clone();
        let mut blob = general_purpose::STANDARD
            .decode(&tampered.encrypted_payload)
            .unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        tampered.encrypted_payload = general_purpose::STANDARD.encode(blob);
        assert!(tampered.try_decrypt(&recipient_priv).is_none());
    }

    #[test]
    fn test_prekey_signature_roundtrip() {
        use crate::app::database::Database;
        let (id_pub, id_priv) = ed25519_keypair();
        let (prekey_pub, _) = x25519_keypair();

        let created_at = 1_700_000_000i64;
        let ctx = prekey_signing_context(&prekey_pub, created_at);
        let sig = Database::sign_message(&ctx, &id_priv).unwrap();

        // Verifies against the signer's identity key
        assert!(Database::verify_signature(&ctx, &sig, &id_pub));
        // Fails against a different identity key
        let (other_pub, _) = ed25519_keypair();
        assert!(!Database::verify_signature(&ctx, &sig, &other_pub));
        // Fails if the pre-key is swapped
        let (other_prekey, _) = x25519_keypair();
        assert!(!Database::verify_signature(
            &prekey_signing_context(&other_prekey, created_at),
            &sig,
            &id_pub
        ));
        // Fails if the creation timestamp is swapped: this is what stops a
        // recorded rotation from being replayed as a newer one
        assert!(!Database::verify_signature(
            &prekey_signing_context(&prekey_pub, created_at + 1),
            &sig,
            &id_pub
        ));
    }

    #[test]
    fn test_seal_to_prekey_then_decrypt() {
        // A message sealed to a recipient's pre-key is readable with the
        // pre-key's private key but not the recipient's identity key.
        let (sender_pub, sender_priv) = ed25519_keypair();
        let (_identity_pub, identity_priv) = x25519_keypair();
        let (prekey_pub, prekey_priv) = x25519_keypair();

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "p",
            "sealed to pre-key",
            "n",
            &[],
            &[prekey_pub], // sender seals to the pre-key
            &sender_priv,
        )
        .unwrap();

        // Decrypts with the pre-key private key
        let decrypted = envelope.try_decrypt(&prekey_priv).expect("prekey decrypts");
        match decrypted.payload {
            ContentPayload::Post { content, .. } => assert_eq!(content, "sealed to pre-key"),
            _ => panic!("wrong payload"),
        }
        // The identity key can't read it (forward secrecy: rotating the pre-key
        // away and deleting it makes this ciphertext unrecoverable)
        assert!(envelope.try_decrypt(&identity_priv).is_none());
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

        let decrypted = envelope.try_decrypt(&own_enc_priv).expect("should decrypt");
        assert_eq!(decrypted.sender_public_key, sender_pub);
        match decrypted.payload {
            ContentPayload::DeviceSync {
                device_id,
                data_json,
                ..
            } => {
                assert_eq!(device_id, "device-abc");
                assert_eq!(data_json, data);
            }
            _ => panic!("Expected DeviceSync payload"),
        }

        // Another user cannot read it
        let (_, other_priv) = x25519_keypair();
        assert!(envelope.try_decrypt(&other_priv).is_none());
    }

    #[test]
    fn test_presence_envelope_roundtrip() {
        let (sender_pub, sender_priv) = ed25519_keypair();
        let (friend_pub, friend_priv) = x25519_keypair();
        let (own_pub, own_priv) = x25519_keypair();

        let payload = ContentPayload::Presence {
            device_id: "device-1".to_string(),
            node_addr_json: r#"{"id":"fake"}"#.to_string(),
            encryption_public_key: Some(own_pub.clone()),
            display_name: "Alice".to_string(),
            bio: "hi".to_string(),
            profile_picture: String::new(),
            profile_signature: None,
            prekey_public: None,
            prekey_signature: None,
            prekey_created_at: 0,
            sent_at: chrono::Utc::now().timestamp(),
        };
        let envelope =
            GossipEnvelope::seal(&payload, &[friend_pub, own_pub], &sender_pub, &sender_priv)
                .unwrap();

        // Both the friend and our own device can read it
        for priv_key in [&friend_priv, &own_priv] {
            let decrypted = envelope.try_decrypt(priv_key).expect("should decrypt");
            assert_eq!(decrypted.sender_public_key, sender_pub);
            match decrypted.payload {
                ContentPayload::Presence { display_name, .. } => {
                    assert_eq!(display_name, "Alice");
                }
                _ => panic!("Expected Presence payload"),
            }
        }

        // A stranger cannot
        let (_, stranger_priv) = x25519_keypair();
        assert!(envelope.try_decrypt(&stranger_priv).is_none());
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
        // padding bucket must produce identically sized wire payloads.
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
            short.encrypted_payload.len(),
            long.encrypted_payload.len(),
            "same-bucket payloads must be indistinguishable by size"
        );

        // Padded payloads must still decrypt cleanly
        match short.try_decrypt(&recipient_priv).unwrap().payload {
            ContentPayload::Post { content, .. } => {
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
            &[pub_key],
            &sender_priv,
        )
        .unwrap();
        let after = chrono::Utc::now().timestamp();

        // Outer timestamp is rounded down to the hour...
        assert_eq!(envelope.timestamp % 3600, 0);
        assert!(envelope.timestamp <= after && envelope.timestamp > before - 3600);

        // ...while the precise time is inside the ciphertext.
        match envelope.try_decrypt(&priv_key).unwrap().payload {
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
