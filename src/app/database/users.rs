use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{SqliteUuid, User};
use crate::app::Database;

impl Database {
    /// Find user by display name - ONLY for local display purposes
    /// WARNING: Display names are NOT unique! Multiple users can have the same display name.
    /// Use find_user_by_public_key for reliable identification.
    #[allow(dead_code)]
    pub fn find_user_by_display_name(&self, display_name: &str) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, display_name, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE display_name = ?1"
        )?;

        let user_iter = stmt.query_map([display_name], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: row.get("private_key")?,
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: row.get("encryption_private_key")?,
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: row.get("profile_signature").ok().flatten(),
                recovery_phrase_hash: row.get("recovery_phrase_hash")?,
                recovery_phrase_shown: row.get("recovery_phrase_shown")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        #[allow(clippy::never_loop)]
        for user in user_iter {
            return Ok(Some(user?));
        }
        Ok(None)
    }

    /// Search friends by display name - ONLY returns users who are already friends
    /// This is safe because the friendship has already been established via public key
    pub fn find_friend_by_display_name(
        &self,
        current_user_id: SqliteUuid,
        display_name: &str,
    ) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT u.id, u.display_name, u.public_key, u.encryption_public_key, u.device_id,
                    u.bio, u.profile_picture, u.created_at, u.updated_at
             FROM users u
             INNER JOIN p2p_connections p ON (p.user_id = ?1 AND p.friend_user_id = u.id)
                                          OR (p.friend_user_id = ?1 AND p.user_id = u.id)
             WHERE p.status = 'accepted' AND u.display_name = ?2",
        )?;

        let user_iter = stmt.query_map(params![current_user_id, display_name], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: None, // Never expose other users' private keys
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None, // Never expose other users' private keys
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: None, // Friend's signature stored separately
                recovery_phrase_hash: None,
                recovery_phrase_shown: false,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        #[allow(clippy::never_loop)]
        for user in user_iter {
            return Ok(Some(user?));
        }
        Ok(None)
    }

    /// Create a new user on first launch - generates 24-word recovery phrase
    /// Returns (User, recovery_phrase)
    /// SECURITY: Display recovery phrase to user ONCE and ensure they save it
    pub fn create_user_first_launch(
        &self,
        display_name: String,
        device_id: String,
    ) -> Result<(User, String), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;
        let now = Utc::now().to_rfc3339();

        // Generate 24-word recovery phrase
        let recovery_phrase = Self::generate_recovery_phrase(None)?;

        // SECURITY: Derive keys from recovery phrase AND display name
        // This cryptographically binds the display name to the identity
        // User must remember both to restore their account
        let (signing_seed, encryption_seed) =
            Self::derive_keys_from_recovery_phrase(&recovery_phrase, &display_name)?;

        // Generate signing keypair
        let (signing_public, signing_private) =
            Self::generate_signing_keypair_from_seed(&signing_seed);

        // Generate encryption keypair
        let (encryption_public, encryption_private) =
            Self::generate_encryption_keypair_from_seed(&encryption_seed);

        // Generate deterministic user_id from public key for multi-device sync
        let user_id = SqliteUuid::from_public_key(&signing_public);

        // Hash recovery phrase for storage (allows validation without storing plaintext)
        let recovery_phrase_hash = Self::hash_recovery_phrase_secure(&recovery_phrase)?;

        conn.execute(
            "INSERT INTO users (id, display_name, public_key, private_key, encryption_public_key, encryption_private_key, device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![user_id, &display_name, &signing_public, &signing_private, &encryption_public, &encryption_private, &device_id, "", "", &recovery_phrase_hash, false, &now, &now],
        ).map_err(|e| format!("Database error: {}", e))?;

        // Register device in devices table
        conn.execute(
            "INSERT INTO devices (id, user_public_key, device_name, last_sync, created_at)
             VALUES (?1, ?2, NULL, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET
                user_public_key = excluded.user_public_key,
                last_sync = excluded.last_sync",
            params![&device_id, &signing_public, &now],
        )
        .map_err(|e| format!("Failed to register device: {}", e))?;

        let user = User {
            id: user_id,
            display_name,
            public_key: Some(signing_public),
            private_key: Some(signing_private),
            encryption_public_key: Some(encryption_public),
            encryption_private_key: Some(encryption_private),
            device_id: Some(device_id),
            bio: None,
            profile_picture: None,
            profile_signature: None, // Will be signed when profile is set up
            recovery_phrase_hash: Some(recovery_phrase_hash),
            recovery_phrase_shown: false,
            created_at: now.clone(),
            updated_at: now,
        };

        Ok((user, recovery_phrase))
    }

    /// Restore a user from their recovery phrase AND display name on new device or after data loss.
    /// Returns User with keys derived from recovery phrase + display name.
    ///
    /// SECURITY: The display_name is cryptographically bound to the identity.
    /// Keys are derived from BOTH the recovery phrase AND the display name.
    /// If the wrong display name is provided, different keys will be generated,
    /// and the restore will effectively create a new identity (which won't match
    /// the user's existing friends/data).
    ///
    /// This prevents impersonation: even with the recovery phrase, an attacker
    /// cannot restore as a different display name because they would get different keys.
    pub fn restore_user_from_recovery_phrase(
        &self,
        display_name: String,
        recovery_phrase: String,
        device_id: String,
    ) -> Result<User, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))?;
        let now = Utc::now().to_rfc3339();

        // SECURITY: Derive keys from recovery phrase AND display name
        // Wrong display name = wrong keys = restore fails to match existing identity
        let (signing_seed, encryption_seed) =
            Self::derive_keys_from_recovery_phrase(&recovery_phrase, &display_name)?;

        // Generate signing keypair
        let (signing_public, signing_private) =
            Self::generate_signing_keypair_from_seed(&signing_seed);

        // Generate encryption keypair
        let (encryption_public, encryption_private) =
            Self::generate_encryption_keypair_from_seed(&encryption_seed);

        // Generate deterministic user_id from public key
        let user_id = SqliteUuid::from_public_key(&signing_public);

        // Hash recovery phrase for storage
        let recovery_phrase_hash = Self::hash_recovery_phrase_secure(&recovery_phrase)?;

        // Check if user already exists in database (multi-device or restore)
        let existing_user: Option<User> = {
            let mut stmt = conn
                .prepare("SELECT id FROM users WHERE public_key = ?1")
                .map_err(|e| format!("Database error: {}", e))?;

            let result = stmt.query_row([&signing_public], |_| Ok(true));
            if result.is_ok() {
                Some(())
            } else {
                None
            }
        }
        .and_then(|_| self.find_user_by_public_key(&signing_public).ok().flatten());

        if let Some(existing) = existing_user {
            // User exists - this is a restore on the same device
            // Only update device_id, preserve existing display_name
            conn.execute(
                "UPDATE users SET device_id = ?1, updated_at = ?2 WHERE public_key = ?3",
                params![&device_id, &now, &signing_public],
            ).map_err(|e| format!("Database error: {}", e))?;

            // Register device in devices table
            conn.execute(
                "INSERT INTO devices (id, user_public_key, device_name, last_sync, created_at)
                 VALUES (?1, ?2, NULL, ?3, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                    user_public_key = excluded.user_public_key,
                    last_sync = excluded.last_sync",
                params![&device_id, &signing_public, &now],
            )
            .map_err(|e| format!("Failed to register device: {}", e))?;

            // Return the existing user with preserved display_name
            return Ok(User {
                id: existing.id,
                display_name: existing.display_name, // Preserve existing
                public_key: Some(signing_public),
                private_key: Some(signing_private),
                encryption_public_key: Some(encryption_public),
                encryption_private_key: Some(encryption_private),
                device_id: Some(device_id),
                bio: existing.bio,
                profile_picture: existing.profile_picture,
                profile_signature: existing.profile_signature,
                recovery_phrase_hash: existing.recovery_phrase_hash,
                recovery_phrase_shown: true,
                created_at: existing.created_at,
                updated_at: now,
            });
        } else {
            // New device restore - user doesn't exist locally
            // SECURITY: The display_name is cryptographically bound to the keys
            // If wrong display_name was provided, we have wrong keys, but that's fine -
            // it just means this restore won't connect to the original identity
            conn.execute(
                "INSERT INTO users (id, display_name, public_key, private_key, encryption_public_key, encryption_private_key, device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![user_id, &display_name, &signing_public, &signing_private, &encryption_public, &encryption_private, &device_id, "", "", &recovery_phrase_hash, true, &now, &now],
            ).map_err(|e| format!("Database error: {}", e))?;

            // Register device in devices table
            conn.execute(
                "INSERT INTO devices (id, user_public_key, device_name, last_sync, created_at)
                 VALUES (?1, ?2, NULL, ?3, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                    user_public_key = excluded.user_public_key,
                    last_sync = excluded.last_sync",
                params![&device_id, &signing_public, &now],
            )
            .map_err(|e| format!("Failed to register device: {}", e))?;

            Ok(User {
                id: user_id,
                display_name,
                public_key: Some(signing_public),
                private_key: Some(signing_private),
                encryption_public_key: Some(encryption_public),
                encryption_private_key: Some(encryption_private),
                device_id: Some(device_id),
                bio: None,
                profile_picture: None,
                profile_signature: None, // New account, no signature yet
                recovery_phrase_hash: Some(recovery_phrase_hash),
                recovery_phrase_shown: true, // Already shown during restore
                created_at: now.clone(),
                updated_at: now,
            })
        }
    }

    pub fn find_user_by_public_key(&self, public_key: &str) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, display_name, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE public_key = ?1"
        )?;

        let user_iter = stmt.query_map([public_key], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: None, // Don't expose other users' private keys
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None, // Don't expose other users' private keys
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: row.get("profile_signature").ok().flatten(),
                recovery_phrase_hash: None,
                recovery_phrase_shown: false,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        #[allow(clippy::never_loop)]
        for user in user_iter {
            return Ok(Some(user?));
        }
        Ok(None)
    }

    /// Sync a peer's user data via P2P - creates user record from their public keys
    /// This is used when discovering a new peer via their public key exchange
    /// Public keys are the ONLY way to identify users in P2P - display names can duplicate!
    #[allow(dead_code)]
    pub fn sync_peer_user(
        &self,
        display_name: &str,
        public_key: &str,
        encryption_public_key: &str,
    ) -> SqliteResult<User> {
        let now = Utc::now().to_rfc3339();
        // Generate deterministic user_id from public key for multi-device sync
        let user_id = SqliteUuid::from_public_key(public_key);

        // Check if user already exists by public key (without holding lock)
        if let Some(existing_user) = self.find_user_by_public_key(public_key)? {
            return Ok(existing_user);
        }

        // Now acquire lock to create user
        let conn = self.conn.lock().unwrap();

        // Double-check user doesn't exist (race condition safety)
        let count: i64 = {
            let mut check_stmt =
                conn.prepare("SELECT COUNT(*) FROM users WHERE public_key = ?1")?;
            check_stmt.query_row([public_key], |row| row.get("COUNT(*)"))?
        };

        if count > 0 {
            drop(conn); // Release lock
            return self
                .find_user_by_public_key(public_key)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows);
        }

        // Create user record without private keys (we don't have them for peers)
        conn.execute(
            "INSERT INTO users (id, display_name, public_key, private_key, encryption_public_key, encryption_private_key, bio, profile_picture, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?5, ?6, ?7, ?8)",
            params![user_id, display_name, public_key, encryption_public_key, "", "", &now, &now],
        )?;

        Ok(User {
            id: user_id,
            display_name: display_name.to_string(),
            public_key: Some(public_key.to_string()),
            private_key: None, // We never have peer's private keys
            encryption_public_key: Some(encryption_public_key.to_string()),
            encryption_private_key: None, // We never have peer's private keys
            device_id: None,
            bio: None,
            profile_picture: None,
            profile_signature: None, // Peer signature stored in friends table
            recovery_phrase_hash: None,
            recovery_phrase_shown: false,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn find_user_by_id(&self, user_id: SqliteUuid) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, display_name, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, profile_signature, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE id = ?1"
        )?;

        let user_iter = stmt.query_map([user_id], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: None, // Don't expose other users' private keys
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None, // Don't expose other users' private keys
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: row.get("profile_signature").ok().flatten(),
                recovery_phrase_hash: None,
                recovery_phrase_shown: false,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        #[allow(clippy::never_loop)]
        for user in user_iter {
            return Ok(Some(user?));
        }
        Ok(None)
    }

    /// Get the current user with their private keys for encryption operations.
    /// ONLY use this for the currently logged-in user, never for other users.
    pub fn find_current_user_by_id(&self, user_id: SqliteUuid) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, display_name, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, profile_signature, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE id = ?1"
        )?;

        let user_iter = stmt.query_map([user_id], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: row.get("private_key")?, // Include private key for current user
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: row.get("encryption_private_key")?, // Include encryption private key
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: row.get("profile_signature").ok().flatten(),
                recovery_phrase_hash: None,
                recovery_phrase_shown: false,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;

        #[allow(clippy::never_loop)]
        for user in user_iter {
            return Ok(Some(user?));
        }
        Ok(None)
    }

    pub fn update_user_profile(
        &self,
        user_id: SqliteUuid,
        display_name: Option<String>,
        bio: Option<String>,
        profile_picture: Option<String>,
    ) -> SqliteResult<User> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // SECURITY: Changing display_name after account creation is NOT allowed
        // because the display_name is cryptographically bound to the keys.
        // If we allowed changes, the user's public key would no longer match
        // what friends expect for that display_name.
        if display_name.is_some() {
            // Log warning but don't fail - just ignore the display_name change
            eprintln!("WARNING: Attempted to change display_name after account creation. This is not allowed as display_name is cryptographically bound to identity.");
        }

        // Use COALESCE to keep existing values when None is passed
        // Note: display_name is intentionally NOT updated
        conn.execute(
            "UPDATE users SET bio = COALESCE(?1, bio), profile_picture = COALESCE(?2, profile_picture), updated_at = ?3 WHERE id = ?4",
            params![bio, profile_picture, now, user_id],
        )?;

        // Return updated user
        let mut stmt = conn.prepare(
            "SELECT id, display_name, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, profile_signature, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE id = ?1"
        )?;

        stmt.query_row([user_id], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: row.get("private_key")?,
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: row.get("encryption_private_key")?,
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: row.get("profile_signature").ok().flatten(),
                recovery_phrase_hash: row.get("recovery_phrase_hash")?,
                recovery_phrase_shown: row.get("recovery_phrase_shown")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })
    }

    /// Set device ID for current user
    #[allow(dead_code)]
    pub fn set_device_id(&self, user_id: SqliteUuid, device_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE users SET device_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![device_id, now, user_id],
        )?;

        // Register device in devices table
        conn.execute(
            "INSERT INTO devices (id, user_public_key, device_name, last_sync, created_at)
             VALUES (?1, (SELECT public_key FROM users WHERE id = ?2), NULL, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET last_sync = ?3",
            params![device_id, user_id, now],
        )?;

        Ok(())
    }

    /// Generate a unique device ID (used by seed_fixture binary)
    #[allow(dead_code)]
    pub fn generate_device_id() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut device_bytes = [0u8; 16];
        rng.fill(&mut device_bytes);

        // Format as hex string
        device_bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    /// Get device ID from database (created during schema init)
    pub fn get_device_id(&self) -> Result<String, String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM app_settings WHERE key = 'device_id'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to get device_id from database: {}", e))
    }

    /// Get user's private keys (signing and encryption)
    /// Returns (private_key, encryption_private_key)
    /// SECURITY: This should ONLY be called for the current user, never for other users
    #[allow(dead_code)]
    pub fn get_user_keys(&self, user_id: SqliteUuid) -> SqliteResult<(String, String)> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT private_key, encryption_private_key FROM users WHERE id = ?1",
            [user_id],
            |row| {
                let private_key: Option<String> = row.get("private_key")?;
                let encryption_private_key: Option<String> = row.get("encryption_private_key")?;

                Ok((
                    private_key.unwrap_or_default(),
                    encryption_private_key.unwrap_or_default(),
                ))
            },
        )
    }

    /// Get user's encryption public key
    /// Used for sealed box encryption in Phase 2
    pub fn get_user_encryption_public_key(
        &self,
        user_id: SqliteUuid,
    ) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT encryption_public_key FROM users WHERE id = ?1",
            [user_id],
            |row| row.get(0),
        )
    }

    /// Get user's encryption private key
    /// SECURITY: This should ONLY be called for the current user, never for other users
    /// Used for sealed box decryption in Phase 2
    pub fn get_user_encryption_private_key(
        &self,
        user_id: SqliteUuid,
    ) -> SqliteResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT encryption_private_key FROM users WHERE id = ?1",
            [user_id],
            |row| row.get(0),
        )
    }
}
