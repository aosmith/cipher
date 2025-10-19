use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{FriendInvite, P2pConnection, SqliteUuid, User};
use crate::app::Database;

impl Database {
    /// Create a friend invite code
    /// Uses remaining: how many times this code can be used
    /// Hours valid: how many hours until the code expires
    pub fn create_friend_invite(
        &self,
        creator_id: SqliteUuid,
        uses_remaining: i32,
        hours_valid: i64,
    ) -> SqliteResult<FriendInvite> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(hours_valid);

        // Generate unique 8-character invite code
        let invite_code = Self::generate_invite_code();
        let invite_id = SqliteUuid::new();

        // Get creator's public key and username
        let (public_key, username): (String, String) = conn.query_row(
            "SELECT public_key, username FROM users WHERE id = ?1",
            [creator_id],
            |row| {
                let pk: Option<String> = row.get("public_key")?;
                let un: String = row.get("username")?;
                Ok((pk.unwrap_or_default(), un))
            },
        )?;

        conn.execute(
            "INSERT INTO friend_invites (id, creator_id, invite_code, public_key, username, uses_remaining, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![invite_id, creator_id, &invite_code, &public_key, &username, uses_remaining, expires_at.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(FriendInvite {
            id: invite_id,
            creator_id,
            invite_code,
            public_key,
            username,
            uses_remaining,
            expires_at: expires_at.to_rfc3339(),
            created_at: now.to_rfc3339(),
        })
    }

    /// Get a friend invite by invite code
    #[allow(dead_code)]
    pub fn get_friend_invite(&self, invite_code: &str) -> SqliteResult<FriendInvite> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, creator_id, invite_code, public_key, username, uses_remaining, expires_at, created_at
             FROM friend_invites WHERE invite_code = ?1",
            [invite_code],
            |row| {
                Ok(FriendInvite {
                    id: row.get("id")?,
                    creator_id: row.get("creator_id")?,
                    invite_code: row.get("invite_code")?,
                    public_key: row.get("public_key")?,
                    username: row.get("username")?,
                    uses_remaining: row.get("uses_remaining")?,
                    expires_at: row.get("expires_at")?,
                    created_at: row.get("created_at")?,
                })
            }
        )
    }

    /// Use a friend invite code to add a friend
    /// Returns the user who created the invite
    pub fn use_friend_invite(
        &self,
        user_id: SqliteUuid,
        invite_code: String,
    ) -> SqliteResult<User> {
        let conn = self.conn.lock().unwrap();

        // Get invite details
        let invite = conn.query_row(
            "SELECT id, creator_id, uses_remaining, expires_at FROM friend_invites WHERE invite_code = ?1",
            [&invite_code],
            |row| {
                Ok((
                    row.get::<_, SqliteUuid>("id")?,
                    row.get::<_, SqliteUuid>("creator_id")?,
                    row.get::<_, i32>("uses_remaining")?,
                    row.get::<_, String>("expires_at")?,
                ))
            }
        )?;

        let (invite_id, creator_id, uses_remaining, expires_at) = invite;

        // Check if user is trying to use their own invite
        if user_id == creator_id {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Cannot use your own friend invite",
                ),
            )));
        }

        // Check if invite has expired
        let now = Utc::now();
        let expiry = chrono::DateTime::parse_from_rfc3339(&expires_at).map_err(|_| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid expiration date",
            )))
        })?;
        if now >= expiry {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invite has expired"),
            )));
        }

        // Check if uses remaining
        if uses_remaining <= 0 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invite has no uses remaining",
                ),
            )));
        }

        // Check if already friends
        let existing_friendship: i64 = conn.query_row(
            "SELECT COUNT(*) FROM p2p_connections
             WHERE ((user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1))
             AND status = 'accepted'",
            params![user_id, creator_id],
            |row| row.get("COUNT(*)")
        )?;

        if existing_friendship > 0 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Already friends"),
            )));
        }

        // Decrement uses
        conn.execute(
            "UPDATE friend_invites SET uses_remaining = uses_remaining - 1 WHERE id = ?1",
            [invite_id],
        )?;

        // Create friendship
        let connection_id = SqliteUuid::new();
        let now_str = now.to_rfc3339();

        conn.execute(
            "INSERT INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'accepted', ?2, ?4, ?5)",
            params![connection_id, user_id, creator_id, &now_str, &now_str],
        )?;

        // Return the creator's user info
        drop(conn); // Release lock before calling find_user_by_id
        self.find_user_by_id(creator_id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    /// Generate a random 8-character invite code (uppercase letters and digits)
    fn generate_invite_code() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut code = String::with_capacity(8);
        let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

        for _ in 0..8 {
            let idx = rng.gen_range(0..charset.len());
            code.push(charset[idx] as char);
        }

        code
    }

    pub fn add_friend(
        &self,
        user_id: SqliteUuid,
        friend_user_id: SqliteUuid,
    ) -> SqliteResult<P2pConnection> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let connection_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'pending', ?2, ?4, ?5)",
            params![connection_id, user_id, friend_user_id, &now, &now],
        )?;

        Ok(P2pConnection {
            id: connection_id,
            user_id,
            friend_user_id,
            status: "pending".to_string(),
            initiated_by: user_id,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn get_friends(&self, user_id: SqliteUuid) -> SqliteResult<Vec<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT u.id, u.username, u.public_key, u.encryption_public_key,
                    u.device_id, u.bio, u.profile_picture, u.created_at, u.updated_at
             FROM users u
             INNER JOIN p2p_connections p ON ((p.user_id = ?1 AND p.friend_user_id = u.id)
                                          OR (p.friend_user_id = ?1 AND p.user_id = u.id))
             WHERE p.status = 'accepted'",
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

        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }
        Ok(users)
    }

    /// Check if two users are friends (bidirectional check)
    #[allow(dead_code)]
    pub fn are_friends(&self, user_id1: SqliteUuid, user_id2: SqliteUuid) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM p2p_connections
             WHERE ((user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1))
             AND status = 'accepted'",
            params![user_id1, user_id2],
            |row| row.get("COUNT(*)")
        )?;

        Ok(count > 0)
    }

    /// Get friends of friends (2-degree connections)
    /// Returns users who are friends with your friends but not direct friends with you
    pub fn get_friends_of_friends(&self, user_id: SqliteUuid) -> SqliteResult<Vec<User>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT u.id, u.username, u.public_key, u.encryption_public_key,
                    u.device_id, u.bio, u.profile_picture, u.created_at, u.updated_at
             FROM users u
             INNER JOIN p2p_connections p2 ON ((p2.user_id = u.id OR p2.friend_user_id = u.id) AND p2.status = 'accepted')
             INNER JOIN p2p_connections p1 ON (
                 (p1.user_id = ?1 OR p1.friend_user_id = ?1) AND p1.status = 'accepted' AND
                 ((p1.user_id = p2.user_id AND p1.user_id != ?1) OR
                  (p1.user_id = p2.friend_user_id AND p1.user_id != ?1) OR
                  (p1.friend_user_id = p2.user_id AND p1.friend_user_id != ?1) OR
                  (p1.friend_user_id = p2.friend_user_id AND p1.friend_user_id != ?1))
             )
             WHERE u.id != ?1
             AND NOT EXISTS (
                 SELECT 1 FROM p2p_connections p3
                 WHERE ((p3.user_id = ?1 AND p3.friend_user_id = u.id)
                     OR (p3.friend_user_id = ?1 AND p3.user_id = u.id))
                 AND p3.status = 'accepted'
             )"
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

        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }
        Ok(users)
    }

    /// Get public keys of all accepted friends
    /// Used for subscribing to friend topics on app startup
    pub fn get_friend_public_keys(&self, user_id: SqliteUuid) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT u.public_key
             FROM users u
             INNER JOIN p2p_connections p ON ((p.user_id = ?1 AND p.friend_user_id = u.id)
                                          OR (p.friend_user_id = ?1 AND p.user_id = u.id))
             WHERE p.status = 'accepted' AND u.public_key IS NOT NULL",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let public_keys: Vec<String> = stmt
            .query_map([user_id], |row| row.get(0))
            .map_err(|e| format!("Failed to execute query: {}", e))?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| format!("Failed to collect results: {}", e))?;

        Ok(public_keys)
    }

    /// Save peer address info (NodeId + relay URL) for a friend
    /// Used to persist discovered peer addresses for reconnection on app restart
    pub fn save_friend_peer_address(
        &self,
        user_id: SqliteUuid,
        friend_user_id: SqliteUuid,
        iroh_node_id: &str,
        relay_url: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE p2p_connections
             SET iroh_node_id = :node_id, friend_relay_url = :relay_url
             WHERE ((user_id = :user_id AND friend_user_id = :friend_user_id)
                    OR (user_id = :friend_user_id AND friend_user_id = :user_id))",
            rusqlite::named_params! {
                ":node_id": iroh_node_id,
                ":relay_url": relay_url,
                ":user_id": user_id,
                ":friend_user_id": friend_user_id,
            },
        )?;
        Ok(())
    }

    /// Get all friend peer addresses (NodeId + relay URL) for a user
    /// Used to pre-populate endpoint with known peer addresses on app startup
    pub fn get_all_friend_peer_addresses(
        &self,
        user_id: SqliteUuid,
    ) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT iroh_node_id, friend_relay_url FROM p2p_connections
             WHERE ((user_id = ?1 OR friend_user_id = ?1) AND status = 'accepted'
                    AND iroh_node_id IS NOT NULL AND friend_relay_url IS NOT NULL)",
        )?;

        let peer_addrs = stmt
            .query_map([user_id], |row| {
                let node_id: String = row.get("iroh_node_id")?;
                let relay_url: String = row.get("friend_relay_url")?;
                Ok((node_id, relay_url))
            })?
            .collect::<Result<Vec<(String, String)>, _>>()?;

        Ok(peer_addrs)
    }
}
