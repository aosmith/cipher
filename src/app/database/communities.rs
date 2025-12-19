use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::types::{
    Community, CommunityInvite, CommunityMember, CommunityPost, CommunityWithMembers, Post,
    SqliteUuid,
};
use crate::app::Database;

impl Database {
    // ============================================
    // Community CRUD Operations
    // ============================================

    /// Create a new community
    pub fn create_community(
        &self,
        creator_id: SqliteUuid,
        name: &str,
        description: Option<&str>,
    ) -> SqliteResult<Community> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let community_id = SqliteUuid::new();

        // Get creator's public key for membership
        let (creator_public_key, creator_display_name): (String, String) = conn.query_row(
            "SELECT COALESCE(encryption_public_key, ''), display_name FROM users WHERE id = ?1",
            [creator_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        // Create the community
        conn.execute(
            "INSERT INTO communities (id, name, description, avatar, creator_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?5)",
            params![community_id, name, description, creator_id, &now],
        )?;

        // Add creator as first member with 'creator' role
        let member_id = SqliteUuid::new();
        conn.execute(
            "INSERT INTO community_members (id, community_id, user_id, public_key, display_name, role, invited_by, joined_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'creator', NULL, ?6)",
            params![
                member_id,
                community_id,
                creator_id,
                &creator_public_key,
                &creator_display_name,
                &now
            ],
        )?;

        Ok(Community {
            id: community_id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            avatar: None,
            creator_id,
            member_count: 1,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Get a community by ID
    pub fn get_community(&self, community_id: SqliteUuid) -> SqliteResult<Option<Community>> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT c.id, c.name, c.description, c.avatar, c.creator_id, c.created_at, c.updated_at,
                    (SELECT COUNT(*) FROM community_members WHERE community_id = c.id) as member_count
             FROM communities c WHERE c.id = ?1",
            [community_id],
            |row| {
                Ok(Community {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    avatar: row.get("avatar")?,
                    creator_id: row.get("creator_id")?,
                    member_count: row.get("member_count")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            },
        );

        match result {
            Ok(community) => Ok(Some(community)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a community with all its members
    pub fn get_community_with_members(
        &self,
        community_id: SqliteUuid,
    ) -> SqliteResult<Option<CommunityWithMembers>> {
        let community = self.get_community(community_id)?;
        if community.is_none() {
            return Ok(None);
        }

        let members = self.get_community_members(community_id)?;

        Ok(Some(CommunityWithMembers {
            community: community.unwrap(),
            members,
        }))
    }

    /// Get all communities a user is a member of
    pub fn get_user_communities(&self, user_id: SqliteUuid) -> SqliteResult<Vec<Community>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT c.id, c.name, c.description, c.avatar, c.creator_id, c.created_at, c.updated_at,
                    (SELECT COUNT(*) FROM community_members WHERE community_id = c.id) as member_count
             FROM communities c
             INNER JOIN community_members cm ON c.id = cm.community_id
             WHERE cm.user_id = ?1
             ORDER BY c.name ASC",
        )?;

        let communities = stmt
            .query_map([user_id], |row| {
                Ok(Community {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    description: row.get("description")?,
                    avatar: row.get("avatar")?,
                    creator_id: row.get("creator_id")?,
                    member_count: row.get("member_count")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(communities)
    }

    /// Update a community's name and description
    pub fn update_community(
        &self,
        community_id: SqliteUuid,
        name: &str,
        description: Option<&str>,
    ) -> SqliteResult<Community> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE communities SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, description, &now, community_id],
        )?;

        drop(conn);
        self.get_community(community_id)
            .map(|c| c.expect("Community should exist after update"))
    }

    /// Delete a community (only creator can delete)
    pub fn delete_community(
        &self,
        community_id: SqliteUuid,
        requester_id: SqliteUuid,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();

        // Check if requester is the creator
        let is_creator: bool = conn
            .query_row(
                "SELECT 1 FROM communities WHERE id = ?1 AND creator_id = ?2",
                params![community_id, requester_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !is_creator {
            return Ok(false);
        }

        // Delete community (cascade will delete members, posts, invites)
        conn.execute("DELETE FROM communities WHERE id = ?1", [community_id])?;

        Ok(true)
    }

    // ============================================
    // Member Operations
    // ============================================

    /// Add a member to a community
    pub fn add_community_member(
        &self,
        community_id: SqliteUuid,
        user_id: SqliteUuid,
        public_key: &str,
        display_name: Option<&str>,
        invited_by: Option<SqliteUuid>,
    ) -> SqliteResult<CommunityMember> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let member_id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO community_members (id, community_id, user_id, public_key, display_name, role, invited_by, joined_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'member', ?6, ?7)",
            params![member_id, community_id, user_id, public_key, display_name, invited_by, &now],
        )?;

        Ok(CommunityMember {
            id: member_id,
            community_id,
            user_id,
            public_key: public_key.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            role: "member".to_string(),
            invited_by,
            joined_at: now,
        })
    }

    /// Remove a member from a community
    pub fn remove_community_member(
        &self,
        community_id: SqliteUuid,
        user_id: SqliteUuid,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();

        // Don't allow removing the creator
        let is_creator: bool = conn
            .query_row(
                "SELECT 1 FROM community_members WHERE community_id = ?1 AND user_id = ?2 AND role = 'creator'",
                params![community_id, user_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if is_creator {
            return Ok(false);
        }

        let deleted = conn.execute(
            "DELETE FROM community_members WHERE community_id = ?1 AND user_id = ?2",
            params![community_id, user_id],
        )?;

        Ok(deleted > 0)
    }

    /// Get all members of a community
    pub fn get_community_members(
        &self,
        community_id: SqliteUuid,
    ) -> SqliteResult<Vec<CommunityMember>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, community_id, user_id, public_key, display_name, role, invited_by, joined_at
             FROM community_members
             WHERE community_id = ?1
             ORDER BY role DESC, joined_at ASC",
        )?;

        let members = stmt
            .query_map([community_id], |row| {
                Ok(CommunityMember {
                    id: row.get("id")?,
                    community_id: row.get("community_id")?,
                    user_id: row.get("user_id")?,
                    public_key: row.get("public_key")?,
                    display_name: row.get("display_name")?,
                    role: row.get("role")?,
                    invited_by: row.get("invited_by")?,
                    joined_at: row.get("joined_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(members)
    }

    /// Get all member encryption public keys for a community (for sealed box encryption)
    pub fn get_community_member_public_keys(
        &self,
        community_id: SqliteUuid,
    ) -> SqliteResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT public_key FROM community_members WHERE community_id = ?1 AND public_key != ''",
        )?;

        let keys = stmt
            .query_map([community_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(keys)
    }

    /// Check if a user is a member of a community
    pub fn is_community_member(
        &self,
        community_id: SqliteUuid,
        user_id: SqliteUuid,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();

        let result = conn.query_row(
            "SELECT 1 FROM community_members WHERE community_id = ?1 AND user_id = ?2",
            params![community_id, user_id],
            |_| Ok(true),
        );

        match result {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(e),
        }
    }

    // ============================================
    // Invite Operations
    // ============================================

    /// Create a community invite code
    pub fn create_community_invite(
        &self,
        community_id: SqliteUuid,
        creator_id: SqliteUuid,
        uses_remaining: i32,
        hours_valid: i64,
    ) -> SqliteResult<CommunityInvite> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(hours_valid);
        let invite_id = SqliteUuid::new();
        let invite_code = Self::generate_invite_code();

        // Get community name for the invite
        let community_name: String = conn.query_row(
            "SELECT name FROM communities WHERE id = ?1",
            [community_id],
            |row| row.get(0),
        )?;

        conn.execute(
            "INSERT INTO community_invites (id, community_id, creator_id, invite_code, uses_remaining, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                invite_id,
                community_id,
                creator_id,
                &invite_code,
                uses_remaining,
                expires_at.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(CommunityInvite {
            id: invite_id,
            community_id,
            community_name,
            creator_id,
            invite_code,
            uses_remaining,
            expires_at: expires_at.to_rfc3339(),
            created_at: now.to_rfc3339(),
        })
    }

    /// Use a community invite code to join a community
    pub fn use_community_invite(
        &self,
        user_id: SqliteUuid,
        invite_code: &str,
    ) -> SqliteResult<Option<Community>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now();

        // Get invite details
        let invite_result = conn.query_row(
            "SELECT id, community_id, uses_remaining, expires_at FROM community_invites WHERE invite_code = ?1",
            [invite_code],
            |row| {
                Ok((
                    row.get::<_, SqliteUuid>("id")?,
                    row.get::<_, SqliteUuid>("community_id")?,
                    row.get::<_, i32>("uses_remaining")?,
                    row.get::<_, String>("expires_at")?,
                ))
            }
        );

        let (invite_id, community_id, uses_remaining, expires_at) = match invite_result {
            Ok(data) => data,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Check if expired
        if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&expires_at) {
            if exp < now {
                return Ok(None); // Expired
            }
        }

        // Check if uses remaining
        if uses_remaining <= 0 {
            return Ok(None);
        }

        // Check if user is already a member
        let already_member: bool = conn
            .query_row(
                "SELECT 1 FROM community_members WHERE community_id = ?1 AND user_id = ?2",
                params![community_id, user_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if already_member {
            // Already a member, just return the community
            drop(conn);
            return self.get_community(community_id);
        }

        // Get user's public key and display name
        let (public_key, display_name): (String, Option<String>) = conn.query_row(
            "SELECT COALESCE(encryption_public_key, ''), display_name FROM users WHERE id = ?1",
            [user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        // Add user as member
        let member_id = SqliteUuid::new();
        conn.execute(
            "INSERT INTO community_members (id, community_id, user_id, public_key, display_name, role, invited_by, joined_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'member', NULL, ?6)",
            params![member_id, community_id, user_id, &public_key, &display_name, now.to_rfc3339()],
        )?;

        // Decrement uses remaining
        conn.execute(
            "UPDATE community_invites SET uses_remaining = uses_remaining - 1 WHERE id = ?1",
            [invite_id],
        )?;

        drop(conn);
        self.get_community(community_id)
    }

    /// Get all invites for a community
    #[allow(dead_code)]
    pub fn get_community_invites(
        &self,
        community_id: SqliteUuid,
    ) -> SqliteResult<Vec<CommunityInvite>> {
        let conn = self.conn.lock().unwrap();

        // Get community name
        let community_name: String = conn.query_row(
            "SELECT name FROM communities WHERE id = ?1",
            [community_id],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, community_id, creator_id, invite_code, uses_remaining, expires_at, created_at
             FROM community_invites
             WHERE community_id = ?1
             ORDER BY created_at DESC",
        )?;

        let invites = stmt
            .query_map([community_id], |row| {
                Ok(CommunityInvite {
                    id: row.get("id")?,
                    community_id: row.get("community_id")?,
                    community_name: community_name.clone(),
                    creator_id: row.get("creator_id")?,
                    invite_code: row.get("invite_code")?,
                    uses_remaining: row.get("uses_remaining")?,
                    expires_at: row.get("expires_at")?,
                    created_at: row.get("created_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(invites)
    }

    // ============================================
    // Post Operations
    // ============================================

    /// Link a post to a community
    pub fn create_community_post(
        &self,
        community_id: SqliteUuid,
        post_id: SqliteUuid,
        show_in_main_feed: bool,
    ) -> SqliteResult<CommunityPost> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let id = SqliteUuid::new();

        conn.execute(
            "INSERT INTO community_posts (id, community_id, post_id, show_in_main_feed, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, community_id, post_id, show_in_main_feed, &now],
        )?;

        Ok(CommunityPost {
            id,
            community_id,
            post_id,
            show_in_main_feed,
            created_at: now,
        })
    }

    /// Get all posts for a community
    pub fn get_community_posts(&self, community_id: SqliteUuid) -> SqliteResult<Vec<Post>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT p.id, p.user_id, p.content, p.encrypted, p.pinned, p.shared_post_id, p.share_comment, p.created_at, p.updated_at,
                    u.display_name
             FROM posts p
             INNER JOIN community_posts cp ON p.id = cp.post_id
             LEFT JOIN users u ON p.user_id = u.id
             WHERE cp.community_id = ?1
             ORDER BY p.created_at DESC",
        )?;

        let posts = stmt
            .query_map([community_id], |row| {
                Ok(Post {
                    id: row.get("id")?,
                    user_id: row.get("user_id")?,
                    display_name: row.get("display_name")?,
                    content: row.get("content")?,
                    encrypted: row.get("encrypted")?,
                    pinned: row.get("pinned")?,
                    shared_post_id: row.get("shared_post_id")?,
                    share_comment: row.get("share_comment")?,
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(posts)
    }
}
