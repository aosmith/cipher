// Rotating signed pre-keys for forward secrecy.
//
// Each user maintains a rotating X25519 "pre-key". The public part is signed by
// their long-term Ed25519 identity key and published to friends (via a
// KeyRotation envelope and piggybacked on Presence). Senders seal to a friend's
// CURRENT pre-key instead of the static identity key. We keep only the current
// and immediately-previous private keys - one rotation of overlap to survive
// lossy gossip - and delete anything older, so a later identity-key compromise
// cannot decrypt recorded traffic that was sealed to a deleted pre-key.

use base64::{engine::general_purpose, Engine as _};
use rand::Rng;
use rusqlite::{params, OptionalExtension, Result as SqliteResult};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use super::Database;
use crate::app::crypto::sealed_box::prekey_signing_context;
use crate::app::types::SqliteUuid;

/// How long a pre-key stays current before we rotate to a fresh one.
pub const PREKEY_ROTATION_SECS: i64 = 7 * 24 * 3600;

/// How long a friend's advertised pre-key is trusted for sealing. If we haven't
/// heard a fresh pre-key in this window (missed rotations while offline), we
/// fall back to their identity key so delivery still succeeds. 2x the rotation
/// interval, so one missed rotation is always covered.
pub const FRIEND_PREKEY_FRESH_SECS: i64 = 2 * PREKEY_ROTATION_SECS;

/// A pre-key ready to advertise: its public key, the creation timestamp bound
/// into the signature, and the identity signature over both.
#[derive(Debug, Clone)]
pub struct PublishedPrekey {
    pub public_key: String,
    pub signature: String,
    /// Unix seconds the pre-key was created. Advertised alongside the key and
    /// covered by `signature`; receivers use it as a monotonic watermark so an
    /// old rotation cannot be replayed to roll the key backward.
    pub created_at: i64,
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

impl Database {
    /// Ensure the user has a current pre-key, generating (and signing) one if
    /// none exists. Returns the current pre-key to advertise. Idempotent.
    pub fn ensure_current_prekey(
        &self,
        user_id: SqliteUuid,
        identity_signing_private_key: &str,
    ) -> Result<PublishedPrekey, String> {
        if let Some(existing) = self.get_current_prekey(user_id) {
            return Ok(existing);
        }
        self.rotate_prekey(user_id, identity_signing_private_key)
    }

    /// Generate a fresh pre-key, sign it with the identity key, and make it
    /// current. The previously-current key is retained as "previous" (for the
    /// decryption overlap window); anything older than that is deleted.
    pub fn rotate_prekey(
        &self,
        user_id: SqliteUuid,
        identity_signing_private_key: &str,
    ) -> Result<PublishedPrekey, String> {
        // Generate a random X25519 keypair
        let mut secret = [0u8; 32];
        rand::thread_rng().fill(&mut secret);
        let static_secret = StaticSecret::from(secret);
        let public = X25519PublicKey::from(&static_secret);
        let public_b64 = general_purpose::STANDARD.encode(public.as_bytes());
        let private_b64 = general_purpose::STANDARD.encode(secret);

        let now = now_ts();

        // Sign the pre-key public (bound to its creation time) with our
        // identity key so friends can verify it really came from us and can
        // tell a fresh rotation from a replayed old one
        let signature = Self::sign_message(
            &prekey_signing_context(&public_b64, now),
            identity_signing_private_key,
        )
        .map_err(|e| format!("Failed to sign pre-key: {}", e))?;

        let conn = self.conn.lock().unwrap();

        // Delete the old "previous" (non-current) key(s) - we only keep one
        // rotation of overlap - then demote the current, then insert the new one
        conn.execute(
            "DELETE FROM signed_prekeys WHERE user_id = ?1 AND is_current = 0",
            params![user_id],
        )
        .map_err(|e| format!("Failed to prune old pre-keys: {}", e))?;
        conn.execute(
            "UPDATE signed_prekeys SET is_current = 0 WHERE user_id = ?1",
            params![user_id],
        )
        .map_err(|e| format!("Failed to demote current pre-key: {}", e))?;
        conn.execute(
            "INSERT INTO signed_prekeys (user_id, public_key, private_key, signature, created_at, is_current)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            params![user_id, public_b64, private_b64, signature, now],
        )
        .map_err(|e| format!("Failed to store pre-key: {}", e))?;

        Ok(PublishedPrekey {
            public_key: public_b64,
            signature,
            created_at: now,
        })
    }

    /// The current pre-key to advertise, if one exists.
    pub fn get_current_prekey(&self, user_id: SqliteUuid) -> Option<PublishedPrekey> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT public_key, signature, created_at FROM signed_prekeys WHERE user_id = ?1 AND is_current = 1",
            params![user_id],
            |row| {
                Ok(PublishedPrekey {
                    public_key: row.get(0)?,
                    signature: row.get(1)?,
                    created_at: row.get(2)?,
                })
            },
        )
        .ok()
    }

    /// Age in seconds of the current pre-key, or None if there isn't one.
    pub fn current_prekey_age_secs(&self, user_id: SqliteUuid) -> Option<i64> {
        let conn = self.conn.lock().unwrap();
        let created: Option<i64> = conn
            .query_row(
                "SELECT created_at FROM signed_prekeys WHERE user_id = ?1 AND is_current = 1",
                params![user_id],
                |row| row.get(0),
            )
            .ok();
        created.map(|c| now_ts() - c)
    }

    /// All private pre-keys (current + previous) we should try when decrypting.
    /// Callers also try the identity key separately.
    pub fn get_prekey_private_keys(&self, user_id: SqliteUuid) -> Vec<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT private_key FROM signed_prekeys WHERE user_id = ?1 ORDER BY is_current DESC, created_at DESC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![user_id], |row| row.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }

    /// Store a friend's advertised pre-key (already signature-verified by the
    /// caller against their authenticated identity key).
    ///
    /// `prekey_created_at` is the creation time the friend signed alongside the
    /// key (0 for pre-v2 senders, which have no signed timestamp - we then fall
    /// back to our local receipt time). It is stored in `prekey_updated_at` and
    /// acts as a MONOTONIC WATERMARK: an announcement that does not advance
    /// past the stored value is rejected, so a replayed old rotation cannot
    /// roll a friend's pre-key backward onto a key they may have deleted.
    ///
    /// Returns true if the stored pre-key was updated, false if the
    /// announcement was rejected as non-advancing (or the friend is unknown).
    pub fn set_friend_prekey(
        &self,
        friend_public_key: &str,
        prekey_public: &str,
        prekey_created_at: i64,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let effective_ts = if prekey_created_at > 0 {
            prekey_created_at
        } else {
            now_ts()
        };

        let existing: Option<(Option<String>, Option<i64>)> = conn
            .query_row(
                "SELECT prekey_public, prekey_updated_at FROM users WHERE public_key = ?1",
                params![friend_public_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let Some((stored_prekey, stored_ts)) = existing else {
            // Unknown user - nothing to attach the pre-key to
            return Ok(false);
        };

        let same_key = stored_prekey.as_deref() == Some(prekey_public);
        if let Some(stored_ts) = stored_ts {
            if effective_ts < stored_ts || (effective_ts == stored_ts && !same_key) {
                // Older (or a conflicting key claiming the same instant):
                // a rollback attempt or a late duplicate. Keep what we have.
                return Ok(false);
            }
        }

        conn.execute(
            "UPDATE users SET prekey_public = ?1, prekey_updated_at = ?2 WHERE public_key = ?3",
            params![prekey_public, effective_ts, friend_public_key],
        )?;
        Ok(true)
    }
}

/// Choose the best recipient key for a friend: their fresh pre-key if we have
/// one, otherwise their static identity encryption key. `prekey` and its
/// `updated_at` come from the users row; `identity_key` is encryption_public_key.
pub fn best_recipient_key(
    identity_key: Option<String>,
    prekey: Option<String>,
    prekey_updated_at: Option<i64>,
) -> Option<String> {
    if let (Some(pk), Some(updated)) = (prekey.as_ref(), prekey_updated_at) {
        if !pk.is_empty() && now_ts() - updated <= FRIEND_PREKEY_FRESH_SECS {
            return Some(pk.clone());
        }
    }
    identity_key.filter(|k| !k.is_empty())
}
