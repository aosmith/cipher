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
#[tauri::command]
pub async fn create_community_invite(
    community_id: SqliteUuid,
    creator_id: SqliteUuid,
    uses_remaining: Option<i32>,
    hours_valid: Option<i64>,
    db: State<'_, Database>,
) -> Result<CommunityInvite, String> {
    let uses = uses_remaining.unwrap_or(1);
    let hours = hours_valid.unwrap_or(24);

    println!(
        "[COMMUNITY] Creating invite for community {:?} with {} uses, valid for {} hours",
        community_id, uses, hours
    );

    db.create_community_invite(community_id, creator_id, uses, hours)
        .map_err(|e| format!("Failed to create invite: {}", e))
}

/// Join a community using an invite code
#[tauri::command]
pub async fn join_community_by_invite(
    user_id: SqliteUuid,
    invite_code: String,
    db: State<'_, Database>,
) -> Result<Option<Community>, String> {
    println!("[COMMUNITY] User {:?} joining with invite code: {}", user_id, invite_code);

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

    // Get all member public keys
    let member_keys = db
        .get_community_member_public_keys(community_id)
        .map_err(|e| format!("Failed to get member keys: {}", e))?;

    if member_keys.is_empty() {
        return Err("No members with encryption keys".to_string());
    }

    // Get current user info for encryption (with private keys)
    let user = db
        .find_current_user_by_id(post.user_id)
        .map_err(|e| format!("Failed to get user: {}", e))?
        .ok_or("User not found")?;

    let sender_pub_key = user
        .encryption_public_key
        .ok_or("User missing encryption public key")?;
    let sender_priv_key = user
        .encryption_private_key
        .ok_or("User missing encryption private key")?;

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

    // Create sealed envelope for all members
    let envelope = GossipEnvelope::new_community_post(
        &sender_pub_key,
        &community_id.to_string(),
        &community.name,
        &post.content,
        None, // TODO: handle attachments
        show_in_main_feed,
        &member_keys,
        &sender_priv_key,
    )?;

    // Serialize envelope to JSON
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    // Broadcast via Iroh - clone Arc to release lock before await
    let network_opt = IROH_NETWORK.lock().map_err(|_| "Failed to lock network")?.clone();

    if let Some(network) = network_opt {
        let message = P2PMessage::SealedEnvelope { envelope_json };
        network
            .publish_message(CONTENT_TOPIC, message)
            .await
            .map_err(|e| format!("Failed to broadcast: {}", e))?;
        println!("[COMMUNITY] Post published to {} members", member_keys.len());
    } else {
        return Err("P2P network not initialized".to_string());
    }

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

    let new_member_pub_key = new_member
        .encryption_public_key
        .clone()
        .ok_or("New member missing encryption public key")?;

    // Get all member public keys (including new member for announcement)
    let member_keys = db
        .get_community_member_public_keys(community_id)
        .map_err(|e| format!("Failed to get member keys: {}", e))?;

    if member_keys.is_empty() {
        return Ok(()); // No one to notify
    }

    // Use new member's keys for encryption (they're announcing themselves)
    let sender_priv_key = new_member
        .encryption_private_key
        .ok_or("New member missing encryption private key")?;

    // Create announcement envelope
    let envelope = GossipEnvelope::new_community_member_added(
        &new_member_pub_key,
        &community_id.to_string(),
        &community.name,
        &new_member_pub_key,
        &new_member.display_name,
        &member_keys,
        &sender_priv_key,
    )?;

    // Serialize envelope to JSON
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    // Broadcast via Iroh - clone Arc to release lock before await
    let network_opt = IROH_NETWORK.lock().map_err(|_| "Failed to lock network")?.clone();

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
