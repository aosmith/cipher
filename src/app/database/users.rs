use bcrypt::{hash, DEFAULT_COST};
use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{SqliteUuid, User};
use crate::app::Database;

impl Database {
    /// Find user by username - ONLY for local authentication
    /// WARNING: Never use this for P2P user discovery! Use find_user_by_public_key instead.
    /// Usernames are not globally unique - public keys are the true identity in P2P.
    pub fn find_user_by_username(&self, username: &str) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE username = ?1"
        )?;

        let user_iter = stmt.query_map([username], |row| {
            Ok(User {
                id: row.get("id")?,
                username: row.get("username")?,
                public_key: row.get("public_key")?,
                private_key: row.get("private_key")?,
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: row.get("encryption_private_key")?,
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
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

    /// Search friends by username - ONLY returns users who are already friends
    /// This is safe because the friendship has already been established via public key
    pub fn find_friend_by_username(
        &self,
        current_user_id: SqliteUuid,
        username: &str,
    ) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT u.id, u.username, u.public_key, u.encryption_public_key, u.device_id,
                    u.bio, u.profile_picture, u.created_at, u.updated_at
             FROM users u
             INNER JOIN p2p_connections p ON (p.user_id = ?1 AND p.friend_user_id = u.id)
                                          OR (p.friend_user_id = ?1 AND p.user_id = u.id)
             WHERE p.status = 'accepted' AND u.username = ?2",
        )?;

        let user_iter = stmt.query_map(params![current_user_id, username], |row| {
            Ok(User {
                id: row.get("id")?,
                username: row.get("username")?,
                public_key: row.get("public_key")?,
                private_key: None, // Never expose other users' private keys
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None, // Never expose other users' private keys
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
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

        // Derive keys from recovery phrase only (no username/password needed)
        let (signing_seed, encryption_seed) =
            Self::derive_keys_from_recovery_phrase(&recovery_phrase)?;

        // Generate signing keypair
        let (signing_public, signing_private) =
            Self::generate_signing_keypair_from_seed(&signing_seed);

        // Generate encryption keypair
        let (encryption_public, encryption_private) =
            Self::generate_encryption_keypair_from_seed(&encryption_seed);

        // Generate deterministic user_id from public key for multi-device sync
        let user_id = SqliteUuid::from_public_key(&signing_public);

        // Hash recovery phrase for storage (allows validation without storing plaintext)
        let recovery_phrase_hash = hash(&recovery_phrase, DEFAULT_COST)
            .map_err(|e| format!("Failed to hash recovery phrase: {}", e))?;

        conn.execute(
            "INSERT INTO users (id, username, public_key, private_key, encryption_public_key, encryption_private_key, device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown, created_at, updated_at)
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
            username: display_name,
            public_key: Some(signing_public),
            private_key: Some(signing_private),
            encryption_public_key: Some(encryption_public),
            encryption_private_key: Some(encryption_private),
            device_id: Some(device_id),
            bio: None,
            profile_picture: None,
            recovery_phrase_hash: Some(recovery_phrase_hash),
            recovery_phrase_shown: false,
            created_at: now.clone(),
            updated_at: now,
        };

        Ok((user, recovery_phrase))
    }

    /// Restore user from recovery phrase on new device or after data loss
    /// Returns User with keys derived from recovery phrase
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

        // Derive keys from recovery phrase
        let (signing_seed, encryption_seed) =
            Self::derive_keys_from_recovery_phrase(&recovery_phrase)?;

        // Generate signing keypair
        let (signing_public, signing_private) =
            Self::generate_signing_keypair_from_seed(&signing_seed);

        // Generate encryption keypair
        let (encryption_public, encryption_private) =
            Self::generate_encryption_keypair_from_seed(&encryption_seed);

        // Generate deterministic user_id from public key
        let user_id = SqliteUuid::from_public_key(&signing_public);

        // Hash recovery phrase for storage
        let recovery_phrase_hash = hash(&recovery_phrase, DEFAULT_COST)
            .map_err(|e| format!("Failed to hash recovery phrase: {}", e))?;

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

        if existing_user.is_some() {
            // User exists - this is a restore on a new device
            // Update device_id for this device
            conn.execute(
                "UPDATE users SET device_id = ?1, username = ?2, updated_at = ?3 WHERE public_key = ?4",
                params![&device_id, &display_name, &now, &signing_public],
            ).map_err(|e| format!("Database error: {}", e))?;
        } else {
            // New user being restored (shouldn't happen normally, but handle gracefully)
            conn.execute(
                "INSERT INTO users (id, username, public_key, private_key, encryption_public_key, encryption_private_key, device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![user_id, &display_name, &signing_public, &signing_private, &encryption_public, &encryption_private, &device_id, "", "", &recovery_phrase_hash, true, &now, &now],
            ).map_err(|e| format!("Database error: {}", e))?;
        }

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
            username: display_name,
            public_key: Some(signing_public),
            private_key: Some(signing_private),
            encryption_public_key: Some(encryption_public),
            encryption_private_key: Some(encryption_private),
            device_id: Some(device_id),
            bio: None,
            profile_picture: None,
            recovery_phrase_hash: Some(recovery_phrase_hash),
            recovery_phrase_shown: true, // Already shown during restore
            created_at: now.clone(),
            updated_at: now,
        };

        Ok(user)
    }

    pub fn find_user_by_public_key(&self, public_key: &str) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE public_key = ?1"
        )?;

        let user_iter = stmt.query_map([public_key], |row| {
            Ok(User {
                id: row.get("id")?,
                username: row.get("username")?,
                public_key: row.get("public_key")?,
                private_key: None, // Don't expose other users' private keys
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None, // Don't expose other users' private keys
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
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
    /// Public keys are the ONLY way to identify users in P2P - usernames can duplicate!
    pub fn sync_peer_user(
        &self,
        username: &str,
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
            "INSERT INTO users (id, username, public_key, private_key, encryption_public_key, encryption_private_key, bio, profile_picture, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?5, ?6, ?7, ?8)",
            params![user_id, username, public_key, encryption_public_key, "", "", &now, &now],
        )?;

        Ok(User {
            id: user_id,
            username: username.to_string(),
            public_key: Some(public_key.to_string()),
            private_key: None, // We never have peer's private keys
            encryption_public_key: Some(encryption_public_key.to_string()),
            encryption_private_key: None, // We never have peer's private keys
            device_id: None,
            bio: None,
            profile_picture: None,
            recovery_phrase_hash: None,
            recovery_phrase_shown: false,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn find_user_by_id(&self, user_id: SqliteUuid) -> SqliteResult<Option<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE id = ?1"
        )?;

        let user_iter = stmt.query_map([user_id], |row| {
            Ok(User {
                id: row.get("id")?,
                username: row.get("username")?,
                public_key: row.get("public_key")?,
                private_key: None, // Don't expose other users' private keys
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None, // Don't expose other users' private keys
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
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
        bio: Option<String>,
        profile_picture: Option<String>,
    ) -> SqliteResult<User> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE users SET bio = ?1, profile_picture = ?2, updated_at = ?3 WHERE id = ?4",
            params![bio, profile_picture, now, user_id],
        )?;

        // Return updated user
        let mut stmt = conn.prepare(
            "SELECT id, username, public_key, private_key, encryption_public_key, encryption_private_key,
                    device_id, bio, profile_picture, recovery_phrase_hash, recovery_phrase_shown,
                    created_at, updated_at
             FROM users WHERE id = ?1"
        )?;

        stmt.query_row([user_id], |row| {
            Ok(User {
                id: row.get("id")?,
                username: row.get("username")?,
                public_key: row.get("public_key")?,
                private_key: row.get("private_key")?,
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: row.get("encryption_private_key")?,
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                recovery_phrase_hash: row.get("recovery_phrase_hash")?,
                recovery_phrase_shown: row.get("recovery_phrase_shown")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })
    }

    /// Set device ID for current user
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

    /// Generate a unique device ID
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

    /// Get or create persistent device ID for this physical device
    /// Device ID is stored in app_data_dir/device_id.txt and persists across logins
    pub fn get_or_create_device_id(app_data_dir: &std::path::Path) -> Result<String, String> {
        let device_id_path = app_data_dir.join("device_id.txt");

        // Try to read existing device ID
        if device_id_path.exists() {
            std::fs::read_to_string(&device_id_path)
                .map(|s| s.trim().to_string())
                .map_err(|e| format!("Failed to read device_id.txt: {}", e))
        } else {
            // Generate new device ID
            let device_id = Self::generate_device_id();

            // Save to file
            std::fs::write(&device_id_path, &device_id)
                .map_err(|e| format!("Failed to write device_id.txt: {}", e))?;

            Ok(device_id)
        }
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
}
