use chrono::Utc;
use rusqlite::{params, OptionalExtension, Result as SqliteResult};

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

        // Get creator's public key and display_name
        let (public_key, display_name): (String, String) = conn.query_row(
            "SELECT public_key, display_name FROM users WHERE id = ?1",
            [creator_id],
            |row| {
                let pk: Option<String> = row.get("public_key")?;
                let dn: String = row.get("display_name")?;
                Ok((pk.unwrap_or_default(), dn))
            },
        )?;

        conn.execute(
            "INSERT INTO friend_invites (id, creator_id, invite_code, public_key, display_name, uses_remaining, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![invite_id, creator_id, &invite_code, &public_key, &display_name, uses_remaining, expires_at.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(FriendInvite {
            id: invite_id,
            creator_id,
            invite_code,
            public_key,
            display_name,
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
            "SELECT id, creator_id, invite_code, public_key, display_name, uses_remaining, expires_at, created_at
             FROM friend_invites WHERE invite_code = ?1",
            [invite_code],
            |row| {
                Ok(FriendInvite {
                    id: row.get("id")?,
                    creator_id: row.get("creator_id")?,
                    invite_code: row.get("invite_code")?,
                    public_key: row.get("public_key")?,
                    display_name: row.get("display_name")?,
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
    pub fn generate_invite_code() -> String {
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

        // Check if there's already a pending request FROM the other user TO us
        // If so, auto-accept it instead of creating a duplicate
        let existing_request: Option<SqliteUuid> = conn.query_row(
            "SELECT id FROM p2p_connections
             WHERE user_id = ?1 AND friend_user_id = ?2 AND status = 'pending'",
            params![friend_user_id, user_id],
            |row| row.get(0),
        ).optional()?;

        if let Some(existing_id) = existing_request {
            // Other user already sent us a request - auto-accept it
            conn.execute(
                "UPDATE p2p_connections SET status = 'accepted', updated_at = ?1 WHERE id = ?2",
                params![&now, existing_id],
            )?;

            return Ok(P2pConnection {
                id: existing_id,
                user_id: friend_user_id,  // The original requester
                friend_user_id: user_id,  // Us
                status: "accepted".to_string(),
                initiated_by: friend_user_id,
                created_at: now.clone(),  // Not accurate but we don't have original
                updated_at: now,
            });
        }

        // Check if we already have a connection with this user (any direction)
        let existing_any: i64 = conn.query_row(
            "SELECT COUNT(*) FROM p2p_connections
             WHERE (user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1)",
            params![user_id, friend_user_id],
            |row| row.get(0),
        )?;

        if existing_any > 0 {
            // Already have some connection - return error to prevent duplicates
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Connection already exists"),
            )));
        }

        // Create new pending request
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
        println!("[DB] get_friends called for user_id: {}", user_id);
        let conn = self.conn.lock().unwrap();

        // Debug: count connections
        let conn_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM p2p_connections WHERE user_id = ?1 OR friend_user_id = ?1",
            [user_id],
            |row| row.get(0)
        ).unwrap_or(0);
        println!("[DB] Total p2p_connections for user: {}", conn_count);

        let mut stmt = conn.prepare(
            "SELECT DISTINCT u.id, u.display_name, u.public_key, u.encryption_public_key,
                    u.device_id, u.bio, u.profile_picture, u.created_at, u.updated_at
             FROM users u
             INNER JOIN p2p_connections p ON ((p.user_id = ?1 AND p.friend_user_id = u.id)
                                          OR (p.friend_user_id = ?1 AND p.user_id = u.id))
             WHERE p.status = 'accepted'",
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
                profile_signature: None, // Friend signature tracked in p2p_connections
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

    /// Get pending incoming friend requests (requests TO this user that they haven't accepted yet)
    pub fn get_pending_friend_requests(&self, user_id: SqliteUuid) -> SqliteResult<Vec<User>> {
        println!("[DB] get_pending_friend_requests called for user_id: {}", user_id);
        let conn = self.conn.lock().unwrap();

        // NEW CONVENTION: user_id is ALWAYS the local user, friend_user_id is ALWAYS the friend
        // Find pending requests where:
        // - user_id = current user (local user owns this row)
        // - friend_user_id = the requester (the friend who sent the request)
        // - initiated_by = friend_user_id (they initiated it, not us)
        // - status = 'pending'
        let mut stmt = conn.prepare(
            "SELECT DISTINCT u.id, u.display_name, u.public_key, u.encryption_public_key,
                    u.device_id, u.bio, u.profile_picture, u.created_at, u.updated_at
             FROM users u
             INNER JOIN p2p_connections p ON (p.user_id = ?1 AND p.friend_user_id = u.id AND p.initiated_by = p.friend_user_id)
             WHERE p.status = 'pending'",
        )?;

        let user_iter = stmt.query_map([user_id], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: None,
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None,
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: None,
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
        println!("[DB] Found {} pending friend requests", users.len());
        Ok(users)
    }

    /// Accept a friend request - updates status to 'accepted'
    pub fn accept_friend_request(&self, user_id: SqliteUuid, friend_user_id: SqliteUuid) -> SqliteResult<()> {
        println!("[DB] accept_friend_request: user {} accepting friend {}", user_id, friend_user_id);
        let conn = self.conn.lock().unwrap();

        // Debug: Show all pending connections for this user
        {
            let mut stmt = conn.prepare(
                "SELECT id, user_id, friend_user_id, status, initiated_by FROM p2p_connections WHERE user_id = ?1"
            )?;
            let rows = stmt.query_map([user_id], |row| {
                Ok((
                    row.get::<_, SqliteUuid>(0)?,
                    row.get::<_, SqliteUuid>(1)?,
                    row.get::<_, SqliteUuid>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, SqliteUuid>(4)?,
                ))
            })?;
            println!("[DB] All connections for user {}:", user_id);
            for row in rows {
                if let Ok((id, uid, fuid, status, initiated)) = row {
                    println!("[DB]   - id={}, user_id={}, friend_user_id={}, status={}, initiated_by={}",
                        id, uid, fuid, status, initiated);
                }
            }
        }

        let now = chrono::Utc::now().to_rfc3339();

        // NEW CONVENTION: user_id is ALWAYS the local user, friend_user_id is ALWAYS the friend
        // Update the pending request to accepted where:
        //   user_id = current user (me - the accepter)
        //   friend_user_id = the friend (who sent the request)
        let rows_updated = conn.execute(
            "UPDATE p2p_connections SET status = 'accepted', updated_at = ?1
             WHERE user_id = ?2 AND friend_user_id = ?3 AND status = 'pending'",
            rusqlite::params![&now, user_id, friend_user_id],
        )?;

        println!("[DB] Tried to update where user_id={} AND friend_user_id={} AND status='pending'", user_id, friend_user_id);
        println!("[DB] Updated {} rows to accepted", rows_updated);
        if rows_updated == 0 {
            println!("[DB] WARNING: No rows updated! The pending request may not exist or IDs don't match.");
        }
        Ok(())
    }

    /// Reject/delete a friend request
    pub fn reject_friend_request(&self, user_id: SqliteUuid, friend_user_id: SqliteUuid) -> SqliteResult<()> {
        println!("[DB] reject_friend_request: user {} rejecting friend {}", user_id, friend_user_id);
        let conn = self.conn.lock().unwrap();

        // NEW CONVENTION: user_id is ALWAYS the local user, friend_user_id is ALWAYS the friend
        // Delete the pending request where:
        //   user_id = current user (me - the rejecter)
        //   friend_user_id = the friend (who sent the request)
        conn.execute(
            "DELETE FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2 AND status = 'pending'",
            rusqlite::params![user_id, friend_user_id],
        )?;

        Ok(())
    }

    /// Get outgoing pending friend requests (requests FROM this user that haven't been accepted yet)
    pub fn get_outgoing_friend_requests(&self, user_id: SqliteUuid) -> SqliteResult<Vec<User>> {
        println!("[DB] get_outgoing_friend_requests called for user_id: {}", user_id);
        let conn = self.conn.lock().unwrap();

        // Find pending requests where:
        // - user_id = current user (we made the connection record)
        // - initiated_by = current user (we initiated it)
        // - status = 'pending'
        let mut stmt = conn.prepare(
            "SELECT DISTINCT u.id, u.display_name, u.public_key, u.encryption_public_key,
                    u.device_id, u.bio, u.profile_picture, u.created_at, u.updated_at
             FROM users u
             INNER JOIN p2p_connections p ON (p.user_id = ?1 AND p.friend_user_id = u.id AND p.initiated_by = ?1)
             WHERE p.status = 'pending'",
        )?;

        let user_iter = stmt.query_map([user_id], |row| {
            Ok(User {
                id: row.get("id")?,
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: None,
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None,
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: None,
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
        println!("[DB] Found {} outgoing friend requests", users.len());
        Ok(users)
    }

    /// Cancel an outgoing friend request (delete a pending request that we initiated)
    pub fn cancel_friend_request(&self, user_id: SqliteUuid, friend_user_id: SqliteUuid) -> SqliteResult<()> {
        println!("[DB] cancel_friend_request: user {} canceling request to {}", user_id, friend_user_id);
        let conn = self.conn.lock().unwrap();

        // Only delete if we initiated it (initiated_by = user_id)
        conn.execute(
            "DELETE FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2 AND initiated_by = ?1 AND status = 'pending'",
            rusqlite::params![user_id, friend_user_id],
        )?;

        Ok(())
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
            "SELECT DISTINCT u.id, u.display_name, u.public_key, u.encryption_public_key,
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
                display_name: row.get("display_name")?,
                public_key: row.get("public_key")?,
                private_key: None, // Don't expose other users' private keys
                encryption_public_key: row.get("encryption_public_key")?,
                encryption_private_key: None, // Don't expose other users' private keys
                device_id: row.get("device_id")?,
                bio: row.get("bio")?,
                profile_picture: row.get("profile_picture")?,
                profile_signature: None,
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
    #[allow(dead_code)]
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

    /// Update only the NodeId for a friend connection, preserving existing relay_url
    /// Used when we receive presence from a friend but don't have their relay URL
    pub fn update_friend_node_id(
        &self,
        user_id: SqliteUuid,
        friend_user_id: SqliteUuid,
        iroh_node_id: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE p2p_connections
             SET iroh_node_id = :node_id
             WHERE ((user_id = :user_id AND friend_user_id = :friend_user_id)
                    OR (user_id = :friend_user_id AND friend_user_id = :user_id))",
            rusqlite::named_params! {
                ":node_id": iroh_node_id,
                ":user_id": user_id,
                ":friend_user_id": friend_user_id,
            },
        )?;
        Ok(())
    }

    /// Get all friend peer addresses (NodeId + relay URL) for a user
    /// Used to pre-populate endpoint with known peer addresses on app startup
    #[allow(dead_code)]
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

    /// Get a friend's node ID and relay URL by their public key
    /// Returns (node_id, relay_url) if available
    pub fn get_friend_peer_info_by_public_key(
        &self,
        user_id: SqliteUuid,
        friend_public_key: &str,
    ) -> SqliteResult<Option<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.iroh_node_id, p.friend_relay_url
             FROM p2p_connections p
             INNER JOIN users u ON (p.friend_user_id = u.id OR p.user_id = u.id)
             WHERE ((p.user_id = ?1 OR p.friend_user_id = ?1)
                    AND u.public_key = ?2
                    AND p.status = 'accepted'
                    AND p.iroh_node_id IS NOT NULL
                    AND p.friend_relay_url IS NOT NULL)",
        )?;

        let result = stmt
            .query_row(rusqlite::params![user_id, friend_public_key], |row| {
                let node_id: String = row.get("iroh_node_id")?;
                let relay_url: String = row.get("friend_relay_url")?;
                Ok((node_id, relay_url))
            })
            .optional()?;

        Ok(result)
    }
}
