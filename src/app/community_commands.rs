// Tauri commands for Communities feature
// Communities are encrypted group spaces where users can share posts

use tauri::State;

use super::crypto::sealed_box::GossipEnvelope;
use super::iroh_commands::IROH_NETWORK;
use super::iroh_network::{P2PMessage, CONTENT_TOPIC};
use super::types::{
    Community, CommunityInvite, CommunityMember, CommunityWithMembers, Post, SqliteUuid,
};
use super::Database;

/// Create a new community
#[tauri::command]
pub async fn create_community(
    user_id: SqliteUuid,
    name: String,
    description: Option<String>,
    db: State<'_, Database>,
) -> Result<Community, String> {
    println!("[COMMUNITY] Creating community: {}", name);

    db.create_community(user_id, &name, description.as_deref())
        .map_err(|e| format!("Failed to create community: {}", e))
}

/// Get all communities the current user is a member of
#[tauri::command]
pub async fn get_my_communities(
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Community>, String> {
    db.get_user_communities(user_id)
        .map_err(|e| format!("Failed to get communities: {}", e))
}

/// Get a community with all its members
#[tauri::command]
pub async fn get_community(
    community_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Option<CommunityWithMembers>, String> {
    db.get_community_with_members(community_id)
        .map_err(|e| format!("Failed to get community: {}", e))
}

/// Get community members
#[tauri::command]
pub async fn get_community_members(
    community_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<CommunityMember>, String> {
    db.get_community_members(community_id)
        .map_err(|e| format!("Failed to get community members: {}", e))
}

/// Leave a community (remove self as member)
#[tauri::command]
pub async fn leave_community(
    community_id: SqliteUuid,
    user_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<bool, String> {
    db.remove_community_member(community_id, user_id)
        .map_err(|e| format!("Failed to leave community: {}", e))
}

/// Create an invite code for a community
/// Returns a self-contained invite URI: cipher://c/{base64url_data}
/// Format: [16 community_id][1 name_len][name][32 creator_pubkey][32 node_id]
#[tauri::command]
pub async fn create_community_invite(
    community_id: SqliteUuid,
    creator_id: SqliteUuid,
    uses_remaining: Option<i32>,
    hours_valid: Option<i64>,
    db: State<'_, Database>,
) -> Result<CommunityInvite, String> {
    let _uses = uses_remaining.unwrap_or(1);
    let _hours = hours_valid.unwrap_or(24);

    println!(
        "[COMMUNITY] Creating self-contained invite for community {:?}",
        community_id
    );

    // Get community info
    let community = db
        .get_community(community_id)
        .map_err(|e| format!("Failed to get community: {}", e))?
        .ok_or("Community not found")?;

    // Get creator's public key
    let creator = db
        .find_current_user_by_id(creator_id)
        .map_err(|e| format!("Failed to get creator: {}", e))?
        .ok_or("Creator not found")?;

    let creator_pubkey = creator
        .encryption_public_key
        .ok_or("Creator missing public key")?;

    // Get node ID from Iroh network
    let network = IROH_NETWORK
        .lock()
        .map_err(|_| "Failed to lock network")?
        .as_ref()
        .ok_or("P2P network not initialized")?
        .clone();

    // Clone endpoint to avoid holding lock during await - blocking_lock()
    // panics on a runtime thread (same pattern as iroh_generate_invite)
    let endpoint_clone = network.endpoint.lock().await.clone();
    let node_id = if let Some(endpoint) = endpoint_clone.as_ref() {
        endpoint.id().to_string()
    } else {
        return Err("Endpoint not initialized".to_string());
    };

    // Build binary payload
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use base64::Engine;

    let mut payload = Vec::new();

    // Community ID (16 bytes)
    payload.extend_from_slice(community_id.as_bytes());

    // Community name (length-prefixed, max 255)
    let name_bytes = community.name.as_bytes();
    if name_bytes.len() > 255 {
        return Err("Community name too long".to_string());
    }
    payload.push(name_bytes.len() as u8);
    payload.extend_from_slice(name_bytes);

    // Creator's public key (32 bytes)
    let pubkey_bytes = STANDARD
        .decode(&creator_pubkey)
        .map_err(|e| format!("Invalid public key: {}", e))?;
    payload.extend_from_slice(&pubkey_bytes);

    // Node ID (32 bytes)
    let node_bytes = hex::decode(&node_id).map_err(|e| format!("Invalid node id: {}", e))?;
    payload.extend_from_slice(&node_bytes);

    // Encode as base64url
    let encoded = URL_SAFE_NO_PAD.encode(&payload);
    let invite_code = format!("cipher://c/{}", encoded);

    println!(
        "[COMMUNITY] Generated invite URI: {} chars, payload: {} bytes",
        invite_code.len(),
        payload.len()
    );

    // Return a CommunityInvite struct with the new code
    // (We're not storing it in the database anymore since it's self-contained)
    Ok(CommunityInvite {
        id: SqliteUuid::new(),
        community_id,
        community_name: community.name,
        creator_id,
        invite_code,
        uses_remaining: -1,         // Unlimited for self-contained invites
        expires_at: "".to_string(), // No expiration
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Parsed community invite data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedCommunityInvite {
    pub community_id: SqliteUuid,
    pub community_name: String,
    pub creator_public_key: String,
    pub node_id: String,
}

/// Parse a community invite URI
fn parse_community_invite(invite_code: &str) -> Result<ParsedCommunityInvite, String> {
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use base64::Engine;

    if !invite_code.starts_with("cipher://c/") {
        return Err("Invalid community invite format. Must start with cipher://c/".to_string());
    }

    let encoded = &invite_code[11..]; // Skip "cipher://c/"
    let payload = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| format!("Invalid invite encoding: {}", e))?;

    // Minimum size: 16 (UUID) + 1 (name_len) + 32 (pubkey) + 32 (node_id) = 81
    if payload.len() < 81 {
        return Err("Invite data too short".to_string());
    }

    let mut offset = 0;

    // Community ID (16 bytes)
    let community_id = SqliteUuid::from_bytes(
        payload[offset..offset + 16]
            .try_into()
            .map_err(|_| "Invalid community ID")?,
    );
    offset += 16;

    // Community name (length-prefixed)
    let name_len = payload[offset] as usize;
    offset += 1;
    if offset + name_len > payload.len() {
        return Err("Invalid name length".to_string());
    }
    let community_name = String::from_utf8(payload[offset..offset + name_len].to_vec())
        .map_err(|_| "Invalid community name encoding")?;
    offset += name_len;

    // Creator's public key (32 bytes)
    if offset + 32 > payload.len() {
        return Err("Missing public key".to_string());
    }
    let creator_public_key = STANDARD.encode(&payload[offset..offset + 32]);
    offset += 32;

    // Node ID (32 bytes)
    if offset + 32 > payload.len() {
        return Err("Missing node ID".to_string());
    }
    let node_id = hex::encode(&payload[offset..offset + 32]);

    Ok(ParsedCommunityInvite {
        community_id,
        community_name,
        creator_public_key,
        node_id,
    })
}

/// Join a community using an invite code
/// Supports self-contained URI format: cipher://c/{base64url_data}
#[tauri::command]
pub async fn join_community_by_invite(
    user_id: SqliteUuid,
    invite_code: String,
    db: State<'_, Database>,
) -> Result<Option<Community>, String> {
    println!(
        "[COMMUNITY] User {:?} joining with invite code: {}...",
        user_id,
        &invite_code[..invite_code.len().min(50)]
    );

    // Try to parse as self-contained invite first
    if invite_code.starts_with("cipher://c/") {
        let parsed = parse_community_invite(&invite_code)?;
        println!(
            "[COMMUNITY] Parsed invite: community='{}', id={:?}",
            parsed.community_name, parsed.community_id
        );

        // Check if community already exists locally
        let existing = db
            .get_community(parsed.community_id)
            .map_err(|e| format!("Database error: {}", e))?;

        let community = if let Some(c) = existing {
            println!("[COMMUNITY] Community already exists locally");
            c
        } else {
            // Create the community locally
            println!("[COMMUNITY] Creating community locally from invite");

            // First ensure the creator user exists
            let creator_user_id = SqliteUuid::from_public_key(&parsed.creator_public_key);
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO users (id, display_name, public_key, encryption_public_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    creator_user_id,
                    "Community Creator",
                    &parsed.creator_public_key,
                    &parsed.creator_public_key,
                    chrono::Utc::now().to_rfc3339(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .map_err(|e| format!("Failed to ensure creator exists: {}", e))?;
            drop(conn);

            // Create the community
            db.create_community_with_id(
                parsed.community_id,
                creator_user_id,
                &parsed.community_name,
                None,
            )
            .map_err(|e| format!("Failed to create community: {}", e))?
        };

        // Check if user is already a member
        let is_member = db
            .is_community_member(parsed.community_id, user_id)
            .map_err(|e| format!("Failed to check membership: {}", e))?;

        if !is_member {
            // Get user's public key
            let user = db
                .find_current_user_by_id(user_id)
                .map_err(|e| format!("Failed to get user: {}", e))?
                .ok_or("User not found")?;

            let user_pubkey = user
                .encryption_public_key
                .ok_or("User missing public key")?;

            // Add user as member
            db.add_community_member(
                parsed.community_id,
                user_id,
                &user_pubkey,
                Some(&user.display_name),
                None,
            )
            .map_err(|e| format!("Failed to add member: {}", e))?;

            println!("[COMMUNITY] User added as member");
        } else {
            println!("[COMMUNITY] User is already a member");
        }

        return Ok(Some(community));
    }

    // Fall back to legacy database lookup (for backwards compatibility)
    println!("[COMMUNITY] Trying legacy database lookup");
    db.use_community_invite(user_id, &invite_code)
        .map_err(|e| format!("Failed to join community: {}", e))
}

/// Create a post in a community
#[tauri::command]
pub async fn create_community_post(
    community_id: SqliteUuid,
    user_id: SqliteUuid,
    content: String,
    show_in_main_feed: bool,
    db: State<'_, Database>,
) -> Result<Post, String> {
    println!(
        "[COMMUNITY] Creating post in community {:?}, show_in_main_feed: {}",
        community_id, show_in_main_feed
    );

    // First check if user is a member
    let is_member = db
        .is_community_member(community_id, user_id)
        .map_err(|e| format!("Failed to check membership: {}", e))?;

    if !is_member {
        return Err("You are not a member of this community".to_string());
    }

    // Create the post
    let post = db
        .create_post(user_id, &content, false)
        .map_err(|e| format!("Failed to create post: {}", e))?;

    // Link it to the community
    db.create_community_post(community_id, post.id, show_in_main_feed)
        .map_err(|e| format!("Failed to link post to community: {}", e))?;

    Ok(post)
}

/// Get all posts for a community
#[tauri::command]
pub async fn get_community_feed(
    community_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<Vec<Post>, String> {
    db.get_community_posts(community_id)
        .map_err(|e| format!("Failed to get community posts: {}", e))
}

/// Publish a community post to all members via P2P
/// This encrypts the post for each member and broadcasts via gossip
#[tauri::command]
pub async fn publish_community_post(
    community_id: SqliteUuid,
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    println!(
        "[COMMUNITY] Publishing post {:?} to community {:?}",
        post_id, community_id
    );

    // Get community info
    let community = db
        .get_community(community_id)
        .map_err(|e| format!("Failed to get community: {}", e))?
        .ok_or("Community not found")?;

    // Get post content (get_shared_post works for any post by ID)
    let post = db
        .get_shared_post(post_id)
        .map_err(|e| format!("Failed to get post: {}", e))?
        .ok_or("Post not found")?;

    // Get member X25519 encryption keys (recipients for the sealed post)
    let member_keys = db
        .get_community_member_encryption_keys(community_id)
        .map_err(|e| format!("Failed to get member keys: {}", e))?;

    if member_keys.is_empty() {
        return Err("No members with encryption keys".to_string());
    }

    // Get current user info for encryption (with private keys)
    let user = db
        .find_current_user_by_id(post.user_id)
        .map_err(|e| format!("Failed to get user: {}", e))?
        .ok_or("User not found")?;

    // CRITICAL: Use the Ed25519 signing keypair for sender identification AND
    // payload signing - the envelope's sealed boxes verify against it
    let sender_pub_key = user.public_key.ok_or("User missing signing public key")?;
    let sender_priv_key = user.private_key.ok_or("User missing signing private key")?;

    // Check community_posts for show_in_main_feed setting
    let show_in_main_feed = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT show_in_main_feed FROM community_posts WHERE community_id = ?1 AND post_id = ?2",
            rusqlite::params![community_id, post_id],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
    };

    // Get network - clone Arc to release lock before await
    let network = IROH_NETWORK
        .lock()
        .map_err(|_| "Failed to lock network")?
        .as_ref()
        .ok_or("P2P network not initialized")?
        .clone();

    // Attachments travel as encrypted blob refs (same mechanism as regular
    // posts in iroh_publish_post) - embedding base64 bytes in the envelope
    // blew past the gossip message size cap for any real image
    let (blob_refs, dropped_attachments) = network.build_blob_refs_for_post(post_id, true).await;
    if blob_refs.is_empty() && !dropped_attachments.is_empty() {
        return Err(format!(
            "All {} attachment(s) failed to store: {}",
            dropped_attachments.len(),
            dropped_attachments.join("; ")
        ));
    }
    if !dropped_attachments.is_empty() {
        eprintln!(
            "[COMMUNITY] WARNING: publishing post {} without {} failed attachment(s): {}",
            post_id,
            dropped_attachments.len(),
            dropped_attachments.join("; ")
        );
    }

    // Our NodeId so members can fetch the blobs from us
    let node_id = network.get_node_id().await;

    // Create sealed envelope for all members
    let envelope = GossipEnvelope::new_community_post(
        &sender_pub_key,
        &community_id.to_string(),
        &community.name,
        &post.content,
        &node_id,
        &blob_refs,
        show_in_main_feed,
        &member_keys,
        &sender_priv_key,
    )?;

    // Serialize envelope to JSON
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    let message = P2PMessage::SealedEnvelope { envelope_json };
    network
        .publish_message(CONTENT_TOPIC, message)
        .await
        .map_err(|e| format!("Failed to broadcast: {}", e))?;
    println!(
        "[COMMUNITY] Post published to {} members ({} blob refs)",
        member_keys.len(),
        blob_refs.len()
    );

    Ok(())
}

/// Notify community members about a new member joining
/// This is called after a user joins via invite
#[tauri::command]
pub async fn announce_community_member(
    community_id: SqliteUuid,
    new_member_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<(), String> {
    println!(
        "[COMMUNITY] Announcing new member {:?} to community {:?}",
        new_member_id, community_id
    );

    // Get community info
    let community = db
        .get_community(community_id)
        .map_err(|e| format!("Failed to get community: {}", e))?
        .ok_or("Community not found")?;

    // Get new member info (with private keys for encryption)
    let new_member = db
        .find_current_user_by_id(new_member_id)
        .map_err(|e| format!("Failed to get new member: {}", e))?
        .ok_or("New member not found")?;

    // CRITICAL: Use signing public key (Ed25519) for identification, NOT encryption key (X25519)
    let new_member_signing_key = new_member
        .public_key
        .ok_or("New member missing signing public key")?;

    // Get member X25519 encryption keys (recipients for the sealed announcement)
    let member_keys = db
        .get_community_member_encryption_keys(community_id)
        .map_err(|e| format!("Failed to get member keys: {}", e))?;

    if member_keys.is_empty() {
        return Ok(()); // No one to notify
    }

    // Use the new member's signing key (they're announcing themselves, so the
    // envelope payload is signed with their key)
    let sender_priv_key = new_member
        .private_key
        .ok_or("New member missing signing private key")?;

    // Create announcement envelope
    // sender_public_key = who sent this envelope (the new member)
    // new_member_public_key = who is being announced (also the new member)
    let envelope = GossipEnvelope::new_community_member_added(
        &new_member_signing_key,
        &community_id.to_string(),
        &community.name,
        &new_member_signing_key,
        &new_member.display_name,
        &member_keys,
        &sender_priv_key,
    )?;

    // Serialize envelope to JSON
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    // Broadcast via Iroh - clone Arc to release lock before await
    let network_opt = IROH_NETWORK
        .lock()
        .map_err(|_| "Failed to lock network")?
        .clone();

    if let Some(network) = network_opt {
        let message = P2PMessage::SealedEnvelope { envelope_json };
        network
            .publish_message(CONTENT_TOPIC, message)
            .await
            .map_err(|e| format!("Failed to broadcast: {}", e))?;
        println!(
            "[COMMUNITY] Member announcement sent to {} members",
            member_keys.len()
        );
    }

    Ok(())
}
