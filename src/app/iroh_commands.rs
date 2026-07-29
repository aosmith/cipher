// Tauri commands for Iroh P2P networking
// Global mesh architecture: all nodes on cipher/content/v1

use base64::{engine::general_purpose, Engine as _};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tauri::{Manager, State};

use super::iroh_network::{IrohNetwork, P2PMessage, CONTENT_TOPIC};
use super::types::SqliteUuid;
use super::Database;

lazy_static! {
    /// Global Iroh network instance
    pub static ref IROH_NETWORK: StdMutex<Option<Arc<IrohNetwork>>> = StdMutex::new(None);
}

/// Global flag to signal app is shutting down
/// All async commands should check this before doing work or responding
static APP_SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Flag to track if app is in foreground (prevents duplicate calls)
static APP_IN_FOREGROUND: AtomicBool = AtomicBool::new(true);

/// Default timeout for P2P operations (5 seconds)
const P2P_OPERATION_TIMEOUT_SECS: u64 = 5;

/// Check if app is shutting down
pub fn is_app_shutting_down() -> bool {
    APP_SHUTTING_DOWN.load(Ordering::Relaxed)
}

/// Signal that app is shutting down - call this early in termination
pub fn signal_app_shutdown() {
    println!("[IROH] App shutdown signaled - cancelling pending operations");
    APP_SHUTTING_DOWN.store(true, Ordering::Relaxed);

    // Also signal the network's shutdown flag if it exists
    if let Ok(guard) = IROH_NETWORK.lock() {
        if let Some(network) = guard.as_ref() {
            network.shutdown_flag.store(true, Ordering::Relaxed);
        }
    }
}

/// Reset shutdown flag (for app restart scenarios)
pub fn reset_app_shutdown() {
    APP_SHUTTING_DOWN.store(false, Ordering::Relaxed);
}

/// Helper macro to wrap async operations with timeout and shutdown check
/// Returns early with error if app is shutting down or operation times out
macro_rules! with_timeout {
    ($timeout_secs:expr, $operation:expr) => {{
        if is_app_shutting_down() {
            return Err("App is shutting down".to_string());
        }

        match tokio::time::timeout(tokio::time::Duration::from_secs($timeout_secs), $operation)
            .await
        {
            Ok(result) => {
                if is_app_shutting_down() {
                    return Err("App shutdown during operation".to_string());
                }
                result
            }
            Err(_) => Err("Operation timed out".to_string()),
        }
    }};
}

/// Initialize Iroh network
#[tauri::command]
pub async fn iroh_initialize(
    app_handle: tauri::AppHandle,
    user_id: SqliteUuid,
    display_name: String,
    public_key: String,
    device_id: Option<String>,
    db: tauri::State<'_, Database>,
) -> Result<String, String> {
    println!("========================================");
    println!("iroh_initialize called");
    println!("   User ID: {}", user_id);
    println!("   Display Name: {}", display_name);
    println!("   Public Key: {}", public_key);
    println!("   Device ID: {:?}", device_id);
    println!("========================================");

    // Clear the global shutdown flag. After logout (iroh_shutdown sets it) nothing else
    // resets it - iroh_enter_foreground's debounce skips the reset while the app stays
    // foregrounded - so every P2P command after re-login failed with "App is shutting down".
    reset_app_shutdown();

    // Tear down any previous network instance (logout/login, forced re-init). Replacing
    // it without shutdown leaks the old endpoint and its background tasks, leaving two
    // live nodes with the SAME NodeId (shared device keypair) fighting over discovery.
    let previous = IROH_NETWORK.lock().unwrap().take();
    if let Some(old_network) = previous {
        println!("[IROH-INIT] Shutting down previous network instance before re-init");
        match tokio::time::timeout(tokio::time::Duration::from_secs(2), old_network.shutdown())
            .await
        {
            Ok(Ok(())) => println!("[IROH-INIT] Previous network shut down cleanly"),
            Ok(Err(e)) => println!("[IROH-INIT] Previous network shutdown error: {}", e),
            Err(_) => println!("[IROH-INIT] Previous network shutdown timed out, continuing"),
        }
        // shutdown() sets the old instance's flag; clear the global one again so the
        // new instance starts clean
        reset_app_shutdown();
    }

    // Device-specific Ed25519 keypair - persistent across sessions
    // Independent of user identity - any user can log in on this device
    use std::fs;

    println!("[IROH-INIT] Creating keypair storage directory...");
    let mut keypair_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    keypair_dir.push("p2p");
    fs::create_dir_all(&keypair_dir)
        .map_err(|e| format!("Failed to create p2p directory: {}", e))?;
    println!("[IROH-INIT] Keypair directory: {:?}", keypair_dir);

    // Use a single keypair file for this device
    let keypair_path = keypair_dir.join("device-keypair.bin");

    println!("[IROH-INIT] Loading or generating keypair...");
    let seed: [u8; 32] = if keypair_path.exists() {
        println!(
            "[IROH-INIT] Loading existing keypair from {:?}",
            keypair_path
        );
        fs::read(&keypair_path)
            .map_err(|e| format!("Failed to read keypair file: {}", e))?
            .try_into()
            .map_err(|_| "Invalid keypair file size".to_string())?
    } else {
        println!("[IROH-INIT] Generating new random device keypair...");
        use rand::RngCore;
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);

        println!("[IROH-INIT] Saving device keypair to {:?}", keypair_path);
        fs::write(&keypair_path, seed)
            .map_err(|e| format!("Failed to write keypair file: {}", e))?;

        seed
    };
    println!("[IROH-INIT] Keypair ready");

    println!("[IROH-INIT] Creating IrohNetwork...");
    let network = IrohNetwork::new(
        user_id,
        display_name,
        public_key.clone(),
        device_id,
        &seed,
        app_handle,
        (*db.inner()).clone(),
    )
    .await?;
    println!("[IROH-INIT] IrohNetwork created successfully");

    println!("[IROH-INIT] Initializing endpoint and gossip...");
    network.initialize().await?;
    println!("[IROH-INIT] Endpoint and gossip initialized");

    // GLOBAL MESH: Node is now subscribed to cipher/content/v1
    // All discovery, presence, and content flows through the single global topic
    println!(
        "[IROH-INIT] Global mesh initialized - node joined {}",
        CONTENT_TOPIC
    );

    println!("[IROH-INIT] Storing network globally...");
    let network_arc = Arc::new(network);
    *IROH_NETWORK.lock().unwrap() = Some(network_arc);

    println!("========================================");
    println!("Iroh initialization COMPLETE");
    println!("========================================");

    Ok(format!("Iroh network initialized for user {}", user_id))
}

/// Subscribe to a friend's topic to receive their updates
/// GLOBAL MESH: This is now a no-op as all nodes are on cipher/content/v1
/// Kept for backwards compatibility with frontend code
#[tauri::command]
pub async fn iroh_subscribe_friend(
    friend_public_key: String,
    db: tauri::State<'_, crate::app::database::Database>,
) -> Result<String, String> {
    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // GLOBAL MESH: No per-friend topics needed
    // All content is broadcast to cipher/content/v1
    println!(
        "[IROH] iroh_subscribe_friend called for {} - using global mesh",
        friend_public_key
    );

    // Still try to add friend's node address for better connectivity
    let peer_info = db
        .get_friend_peer_info_by_public_key(network.user_id, &friend_public_key)
        .ok()
        .flatten();
    if let Some((node_id, relay_url)) = peer_info {
        println!(
            "[IROH] Adding friend peer info: NodeId={}, Relay={}",
            node_id, relay_url
        );
        if let Ok(peer_node_id) = node_id.parse::<iroh::EndpointId>() {
            if let Ok(relay_url_parsed) = relay_url.parse::<url::Url>() {
                let node_addr = iroh::EndpointAddr::new(peer_node_id)
                    .with_relay_url(iroh::RelayUrl::from(relay_url_parsed));
                network.register_peer_address(node_addr);
                println!("[IROH] Added friend address to the address book");
            }
        }
    }

    Ok("Friend added to global mesh".to_string())
}

/// Send a direct message to a user
/// GLOBAL MESH: Message broadcast to all, only recipient can decrypt
#[tauri::command]
pub async fn iroh_send_message(
    to_user_id: SqliteUuid,
    _to_public_key: String, // Kept for API compat; recipient resolved by user id
    encrypted_content: String,
    db: State<'_, Database>,
) -> Result<String, String> {
    if is_app_shutting_down() {
        return Err("App is shutting down".to_string());
    }

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    let message_id = uuid::Uuid::new_v4().to_string();

    // DMs are sealed to the recipient's encryption key. The old plaintext
    // DirectMessage broadcast the sender/recipient pair and timing to the
    // whole mesh even though the content was encrypted.
    // Seal to the recipient's CURRENT rotating pre-key when we have a fresh
    // one (falling back to their identity key inside best_recipient_key_for_user
    // when we don't) - the same selection posts/comments/reactions use. Sealing
    // DMs straight to the static identity key meant a later identity-key
    // compromise decrypted every recorded DM; the pre-key path gives DMs the
    // same forward secrecy as the rest of the content types.
    let recipient_encryption_key = db
        .best_recipient_key_for_user(to_user_id)
        .map_err(|e| format!("Failed to look up recipient key: {}", e))?
        .filter(|k| !k.is_empty())
        .ok_or("Recipient's encryption key is unknown - wait for their presence or re-add them")?;

    let payload = crate::app::crypto::ContentPayload::DirectMessage {
        message_id: message_id.clone(),
        content: encrypted_content,
        thread_id: None,
        sent_at: chrono::Utc::now().timestamp(),
    };

    with_timeout!(
        P2P_OPERATION_TIMEOUT_SECS,
        network.publish_sealed(&payload, std::slice::from_ref(&recipient_encryption_key))
    )?;

    Ok(message_id)
}

/// Publish a post to the global mesh
/// PHASE 2: Encrypts post with sealed boxes for each friend
#[tauri::command]
pub async fn iroh_publish_post(
    content: String,
    post_id: SqliteUuid,
    db: State<'_, Database>,
) -> Result<String, String> {
    println!(
        "[PUBLISH-POST] === START iroh_publish_post for post_id: {} ===",
        post_id
    );

    if is_app_shutting_down() {
        println!("[PUBLISH-POST] App is shutting down, skipping");
        return Err("App is shutting down".to_string());
    }

    println!("[PUBLISH-POST] Step 1: Acquiring IROH_NETWORK lock...");
    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();
    println!("[PUBLISH-POST] Step 1: DONE - Got network");

    // Get our encryption keys (public key fetched for future use with multi-device sync)
    println!("[PUBLISH-POST] Step 3: Getting encryption public key from DB...");
    let _our_encryption_public_key = db
        .get_user_encryption_public_key(network.user_id)
        .map_err(|e| format!("Failed to get encryption public key: {}", e))?
        .ok_or("No encryption public key found")?;
    println!("[PUBLISH-POST] Step 3: DONE - Got encryption public key");

    println!("[PUBLISH-POST] Step 4: Getting signing private key from DB...");
    let our_signing_private_key = db
        .get_user_signing_private_key(network.user_id)
        .map_err(|e| format!("Failed to get signing private key: {}", e))?
        .ok_or("No signing private key found")?;
    println!("[PUBLISH-POST] Step 4: DONE - Got signing private key");

    // Get friend encryption public keys
    println!("[PUBLISH-POST] Step 5: Getting friend encryption keys...");
    let friend_encryption_keys = network.get_friend_encryption_public_keys();
    println!(
        "[PUBLISH-POST] Step 5: DONE - Got {} friend encryption keys",
        friend_encryption_keys.len()
    );

    // Get our node ID for blob fetching (needed for both paths)
    println!("[PUBLISH-POST] Step 6: Getting node_id...");
    let node_id = network.get_node_id().await;
    println!(
        "[PUBLISH-POST] Step 6: DONE - Got node_id: {}",
        &node_id[..8.min(node_id.len())]
    );

    // Store attachments as encrypted blobs via the shared helper (same path
    // the backfill re-send and community posts use). Quota checking and
    // storage accounting happen inside the helper.
    println!("[PUBLISH-POST] Step 7: Storing attachments as encrypted blobs...");
    let (blob_refs, dropped_attachments) = network.build_blob_refs_for_post(post_id, true).await;
    if blob_refs.is_empty() && !dropped_attachments.is_empty() {
        // Every requested attachment failed - surface it instead of silently
        // publishing a text-only post and reporting full success
        println!("[PUBLISH-POST] === END iroh_publish_post FAILED (all attachments dropped) ===");
        return Err(format!(
            "All {} attachment(s) failed to store: {}",
            dropped_attachments.len(),
            dropped_attachments.join("; ")
        ));
    }
    if !dropped_attachments.is_empty() {
        eprintln!(
            "[PUBLISH-POST] WARNING: {} attachment(s) dropped from post {}: {}",
            dropped_attachments.len(),
            post_id,
            dropped_attachments.join("; ")
        );
    }
    println!(
        "[PUBLISH-POST] Step 7: DONE - {} blob ref(s), {} dropped",
        blob_refs.len(),
        dropped_attachments.len()
    );

    if friend_encryption_keys.is_empty() {
        // No friends with encryption keys - skip P2P broadcast (post is saved locally)
        // Posts will be synced via device sync when friends are added
        println!("[PUBLISH-POST] No friends with encryption keys - skipping P2P broadcast (post saved locally)");
        println!("[PUBLISH-POST] === END iroh_publish_post SUCCESS (local only) ===");
        return Ok("Post saved locally (no friends to broadcast to)".to_string());
    }

    // PHASE 2: Create sealed envelope with boxes for each friend
    println!(
        "[PUBLISH-POST] Step 7 (PHASE 2): Creating sealed envelope for {} friends...",
        friend_encryption_keys.len()
    );

    // CRITICAL: Use signing public key (Ed25519) for sender identification, NOT encryption key (X25519)
    // The encryption key is only used for creating the sealed boxes
    let envelope = crate::app::crypto::GossipEnvelope::new_post(
        &network.public_key,
        &post_id.to_string(),
        &content,
        &node_id,
        &blob_refs,
        &friend_encryption_keys,
        &our_signing_private_key,
    )
    .map_err(|e| format!("Failed to create sealed envelope: {}", e))?;
    println!("[PUBLISH-POST] Step 6 (PHASE 2): DONE - Envelope created");

    // Serialize envelope to JSON
    println!("[PUBLISH-POST] Step 7 (PHASE 2): Serializing envelope to JSON...");
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;
    println!(
        "[PUBLISH-POST] Step 7 (PHASE 2): DONE - Serialized ({} bytes)",
        envelope_json.len()
    );

    // Create SealedEnvelope message
    let message = P2PMessage::SealedEnvelope { envelope_json };

    // Broadcast to global mesh
    println!("[PUBLISH-POST] Step 8 (PHASE 2): Calling publish_message().await...");
    network.publish_message(CONTENT_TOPIC, message).await?;
    println!("[PUBLISH-POST] Step 8 (PHASE 2): DONE - Message published");

    println!("[PUBLISH-POST] === END iroh_publish_post SUCCESS (sealed) ===");
    if dropped_attachments.is_empty() {
        Ok("Post published (sealed)".to_string())
    } else {
        Ok(format!(
            "Post published (sealed) - {} attachment(s) dropped",
            dropped_attachments.len()
        ))
    }
}

/// Publish a post comment to the global mesh (encrypted for friends)
#[tauri::command]
pub async fn iroh_publish_post_comment(
    comment_id: SqliteUuid,
    post_id: SqliteUuid,
    content: String,
    parent_comment_id: Option<SqliteUuid>,
    db: State<'_, Database>,
) -> Result<String, String> {
    println!(
        "[PUBLISH-COMMENT] Publishing comment {} on post {}",
        comment_id, post_id
    );

    if is_app_shutting_down() {
        return Err("App is shutting down".to_string());
    }

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // Get friend encryption public keys
    let friend_encryption_keys = network.get_friend_encryption_public_keys();

    if friend_encryption_keys.is_empty() {
        // No friends with encryption keys - skip P2P broadcast (comment is saved
        // locally). ALL content on the wire must be encrypted; the old plaintext
        // Content fallback broadcast to the whole mesh with no one able to read it.
        println!("[PUBLISH-COMMENT] No friends with encryption keys - skipping P2P broadcast (comment saved locally)");
        return Ok("Comment saved locally (no friends to broadcast to)".to_string());
    }

    // Get our signing key: sealed envelopes are signed so recipients can
    // verify the sender (boxes are encrypted with ephemeral keys, not ours)
    let our_signing_private_key = db
        .get_user_signing_private_key(network.user_id)
        .map_err(|e| format!("Failed to get signing private key: {}", e))?
        .ok_or("No signing private key found")?;

    // Create sealed envelope
    // CRITICAL: Use signing public key (Ed25519) for sender identification, NOT encryption key (X25519)
    let envelope = crate::app::crypto::GossipEnvelope::new_post_comment(
        &network.public_key,
        &comment_id.to_string(),
        &post_id.to_string(),
        &content,
        parent_comment_id
            .as_ref()
            .map(|id| id.to_string())
            .as_deref(),
        &friend_encryption_keys,
        &our_signing_private_key,
    )
    .map_err(|e| format!("Failed to create sealed envelope: {}", e))?;

    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    let message = P2PMessage::SealedEnvelope { envelope_json };

    network.publish_message(CONTENT_TOPIC, message).await?;

    println!("[PUBLISH-COMMENT] ✓ Comment published (sealed)");
    Ok("Comment published (sealed)".to_string())
}

/// Publish a post reaction to the global mesh (encrypted for friends, or unencrypted fallback)
#[tauri::command]
pub async fn iroh_publish_post_reaction(
    post_id: SqliteUuid,
    emoji: String,
    action: String,
    db: State<'_, Database>,
) -> Result<String, String> {
    println!(
        "[PUBLISH-REACTION] Publishing reaction {} {} on post {}",
        action, emoji, post_id
    );

    if is_app_shutting_down() {
        return Err("App is shutting down".to_string());
    }

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // Get friend encryption public keys
    let friend_encryption_keys = network.get_friend_encryption_public_keys();

    if friend_encryption_keys.is_empty() {
        // No friends with encryption keys - skip P2P broadcast (reaction is saved
        // locally). ALL content on the wire must be encrypted; the old plaintext
        // Content fallback broadcast to the whole mesh with no one able to read it.
        println!("[PUBLISH-REACTION] No friends with encryption keys - skipping P2P broadcast (reaction saved locally)");
        return Ok("Reaction saved locally (no friends to broadcast to)".to_string());
    }

    // Get our signing key: sealed envelopes are signed so recipients can
    // verify the sender (boxes are encrypted with ephemeral keys, not ours)
    let our_signing_private_key = db
        .get_user_signing_private_key(network.user_id)
        .map_err(|e| format!("Failed to get signing private key: {}", e))?
        .ok_or("No signing private key found")?;

    // Create sealed envelope
    // CRITICAL: Use signing public key (Ed25519) for sender identification, NOT encryption key (X25519)
    let envelope = crate::app::crypto::GossipEnvelope::new_post_reaction(
        &network.public_key,
        &post_id.to_string(),
        &emoji,
        &action,
        &friend_encryption_keys,
        &our_signing_private_key,
    )
    .map_err(|e| format!("Failed to create sealed envelope: {}", e))?;

    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    let message = P2PMessage::SealedEnvelope { envelope_json };

    network.publish_message(CONTENT_TOPIC, message).await?;

    println!("[PUBLISH-REACTION] ✓ Reaction published (sealed)");
    Ok("Reaction published (sealed)".to_string())
}

/// Get connection status
#[tauri::command]
pub async fn iroh_get_connection_status() -> Result<serde_json::Value, String> {
    if is_app_shutting_down() {
        return Ok(serde_json::json!({
            "listening": false,
            "connected_peers": 0,
            "shutting_down": true
        }));
    }

    let network_opt = IROH_NETWORK.lock().unwrap().clone();

    if network_opt.is_none() {
        return Ok(serde_json::json!({
            "listening": false,
            "connected_peers": 0
        }));
    }

    let network = network_opt.unwrap();

    // Use timeout for status check
    match tokio::time::timeout(
        tokio::time::Duration::from_secs(P2P_OPERATION_TIMEOUT_SECS),
        network.get_connection_status(),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Ok(serde_json::json!({
            "listening": false,
            "connected_peers": 0,
            "error": "Status check timed out"
        })),
    }
}

/// Shutdown Iroh network - signals all background operations to stop
#[tauri::command]
pub async fn iroh_shutdown() -> Result<String, String> {
    // Signal shutdown first to stop background loops and pending operations
    signal_app_shutdown();

    let network_opt = IROH_NETWORK.lock().unwrap().take();

    if let Some(network) = network_opt {
        // Give a short timeout for shutdown - don't wait forever
        match tokio::time::timeout(tokio::time::Duration::from_secs(2), network.shutdown()).await {
            Ok(result) => result?,
            Err(_) => {
                println!("[IROH] Shutdown timed out, forcing close");
            }
        }
    }

    Ok("Iroh shut down".to_string())
}

/// Re-announce presence to the network
#[tauri::command]
pub async fn iroh_announce_presence() -> Result<String, String> {
    if is_app_shutting_down() {
        return Err("App is shutting down".to_string());
    }

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    with_timeout!(P2P_OPERATION_TIMEOUT_SECS, network.announce_presence())?;

    Ok("Presence announced".to_string())
}

/// Check network health - returns detailed status
/// Use this to determine if reconnection is needed after app resume
#[tauri::command]
pub async fn iroh_health_check() -> Result<serde_json::Value, String> {
    if is_app_shutting_down() {
        return Ok(serde_json::json!({
            "healthy": false,
            "needs_reconnect": false,
            "error": "App is shutting down"
        }));
    }

    let network_opt = IROH_NETWORK.lock().unwrap().clone();

    if network_opt.is_none() {
        return Ok(serde_json::json!({
            "healthy": false,
            "needs_reconnect": true,
            "has_endpoint": false,
            "error": "Iroh not initialized"
        }));
    }

    let network = network_opt.unwrap();

    // Use timeout for health check
    let result = tokio::time::timeout(
        tokio::time::Duration::from_secs(P2P_OPERATION_TIMEOUT_SECS),
        network.health_check(),
    )
    .await;

    match result {
        Ok((is_healthy, needs_reconnect, status)) => Ok(serde_json::json!({
            "healthy": is_healthy,
            "needs_reconnect": needs_reconnect,
            "details": status
        })),
        Err(_) => Ok(serde_json::json!({
            "healthy": false,
            "needs_reconnect": true,
            "error": "Health check timed out"
        })),
    }
}

/// Recover network connectivity without full reinitialization
/// Call this when health_check indicates issues but endpoint still exists
#[tauri::command]
pub async fn iroh_recover() -> Result<String, String> {
    if is_app_shutting_down() {
        return Err("App is shutting down".to_string());
    }

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized - use iroh_initialize instead")?
        .clone();

    // Recovery can take longer, give it 10 seconds
    with_timeout!(10, network.recover())?;

    Ok("Network recovery complete".to_string())
}

/// Generate a friend request QR code
/// Returns a cipher:// URI with ultra-minimal addressing info (public key + NodeId only)
/// ~97 characters - relay discovered via DHT, name/signature sent via FriendRequest
/// GLOBAL MESH: All nodes on cipher/content/v1 - just share connection info
#[tauri::command]
pub async fn iroh_generate_invite() -> Result<String, String> {
    println!("[IROH] Generating friend request QR code...");

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // GLOBAL MESH: Already subscribed to cipher/content/v1 on init
    // Just need to share our connection info
    println!("[IROH] Generating invite - node already on global mesh");

    // Get current NodeAddr from endpoint
    // Clone endpoint to avoid holding lock during await
    let endpoint_clone = network.endpoint.lock().await.clone();
    let node_addr = if let Some(endpoint) = endpoint_clone.as_ref() {
        endpoint.addr()
    } else {
        return Err("Endpoint not initialized".to_string());
    };

    // Extract NodeId (relay discovered via DHT, not in invite)
    let node_id = node_addr.id.to_string();
    let _relay_url = node_addr.relay_urls().next().map(|url| url.to_string());

    // V3 format: [32 ed25519 pubkey][32 x25519 enc key][32 node][1 name_len][name]
    // The encryption key lets the scanner SEAL their friend request to us -
    // without it the request would have to travel in plaintext.
    use base64::engine::general_purpose::STANDARD;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    // Decode keys to binary
    let pubkey_bytes = STANDARD
        .decode(&network.public_key)
        .map_err(|e| format!("Invalid public key base64: {}", e))?;
    let encryption_public_key = network
        .get_user_encryption_public_key()
        .ok_or("No encryption public key - cannot generate invite")?;
    let enc_key_bytes = STANDARD
        .decode(&encryption_public_key)
        .map_err(|e| format!("Invalid encryption key base64: {}", e))?;
    if enc_key_bytes.len() != 32 {
        return Err("Invalid encryption key length".to_string());
    }
    let node_bytes = hex::decode(&node_id).map_err(|e| format!("Invalid node id hex: {}", e))?;

    // Get display name (truncate to 255 bytes max)
    let display_name = network.display_name.clone();
    let name_bytes = display_name.as_bytes();
    let name_len = name_bytes.len().min(255) as u8;

    // Build binary payload: [32 pubkey][32 enc key][32 node][1 name_len][name]
    let mut payload = Vec::new();
    payload.extend_from_slice(&pubkey_bytes); // 32 bytes
    payload.extend_from_slice(&enc_key_bytes); // 32 bytes
    payload.extend_from_slice(&node_bytes); // 32 bytes
    payload.push(name_len); // 1 byte
    payload.extend_from_slice(&name_bytes[..name_len as usize]); // N bytes

    // Encode as base64url
    let encoded = URL_SAFE_NO_PAD.encode(&payload);

    // Create URI (v3 prefix - older i/, f/, add-friend formats are rejected)
    let qr_code = format!("cipher://v3/{}", encoded);

    println!("[IROH] ✓ QR code generated ({} chars)", qr_code.len());
    println!("[IROH]   Public Key: {}", network.public_key);
    println!("[IROH]   NodeId: {}", node_id);
    println!("[IROH]   Display Name: {}", display_name);

    Ok(qr_code)
}

/// Parsed invite code data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedInvite {
    pub public_key: String,
    /// X25519 encryption key (v3 invites) - friend requests are sealed to it
    pub encryption_public_key: Option<String>,
    pub node_id: String,
    pub relay_url: Option<String>,
    pub display_name: Option<String>,
    pub signature: Option<String>,
}

/// Parse an invite code (supports multiple formats)
/// V2 minimal: cipher://i/{base64url_data} - no relay, no signature
/// V1 compressed: cipher://f/{base64url_compressed_data}
/// Legacy: cipher://add-friend?key=...&node=...&relay=...&name=...&sig=...
#[tauri::command]
pub fn parse_invite_code(invite_code: String) -> Result<ParsedInvite, String> {
    println!(
        "[IROH] Parsing invite code: {}...",
        &invite_code[..invite_code.len().min(50)]
    );

    if invite_code.starts_with("cipher://v3/") {
        // V3: carries the encryption key so friend requests can be sealed
        parse_v3_invite(&invite_code)
    } else if invite_code.starts_with("cipher://i/")
        || invite_code.starts_with("cipher://f/")
        || invite_code.starts_with("cipher://add-friend?")
    {
        // Older formats lack the encryption key, so a friend request would
        // have to travel in plaintext - refuse instead of leaking
        Err(
            "This invite was created by an older version of Cipher and can't be \
             used securely. Ask your friend to generate a new invite."
                .to_string(),
        )
    } else {
        Err("Invalid invite code format. Must start with cipher://".to_string())
    }
}

/// Parse V3 format: cipher://v3/{base64url_data}
/// [32 ed25519 pubkey][32 x25519 enc key][32 node][1 name_len][name]
fn parse_v3_invite(invite_code: &str) -> Result<ParsedInvite, String> {
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use base64::Engine;

    let encoded = invite_code
        .strip_prefix("cipher://v3/")
        .ok_or("Invalid v3 invite format")?;

    let payload = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| format!("Failed to decode base64url: {}", e))?;

    // Minimum size: 32 + 32 + 32 = 96 bytes
    if payload.len() < 96 {
        return Err(format!(
            "Payload too short: {} bytes (need at least 96)",
            payload.len()
        ));
    }

    let mut pos = 0;
    let public_key = STANDARD.encode(&payload[pos..pos + 32]);
    pos += 32;
    let encryption_public_key = STANDARD.encode(&payload[pos..pos + 32]);
    pos += 32;
    let node_id = hex::encode(&payload[pos..pos + 32]);
    pos += 32;

    let display_name = if pos < payload.len() {
        let name_len = payload[pos] as usize;
        pos += 1;
        if name_len > 0 && pos + name_len <= payload.len() {
            Some(
                String::from_utf8(payload[pos..pos + name_len].to_vec())
                    .map_err(|_| "Invalid display name encoding")?,
            )
        } else {
            None
        }
    } else {
        None
    };

    println!(
        "[IROH] Parsed v3 invite: pubkey={}..., node={}...",
        &public_key[..8.min(public_key.len())],
        &node_id[..8.min(node_id.len())]
    );

    Ok(ParsedInvite {
        public_key,
        encryption_public_key: Some(encryption_public_key),
        node_id,
        relay_url: None,
        display_name,
        signature: None,
    })
}

/// Parse V2 format: cipher://i/{base64url_data}
/// Ultra-minimal: [32 pubkey][32 node] = 64 bytes only
/// Optional legacy: [32 pubkey][32 node][1 name_len][name][64 sig]
/// Add a friend by public key with optional compact node info
/// Verifies signed display name if provided
/// GLOBAL MESH: Creates friendship in database and sends FriendRequest to global mesh
#[tauri::command]
pub async fn iroh_add_friend_by_public_key(
    friend_public_key: String,
    encryption_public_key: Option<String>,
    node_id: Option<String>,
    relay_url: Option<String>,
    display_name: Option<String>,
    signature: Option<String>,
    db: State<'_, Database>,
) -> Result<String, String> {
    println!(
        "[IROH] Adding friend by public key: {} (global mesh)",
        friend_public_key
    );
    println!(
        "[IROH] Parameters: node_id={:?}, relay_url={:?}, display_name={:?}, signature={:?}",
        node_id,
        relay_url,
        display_name,
        signature.as_ref().map(|s| &s[..20.min(s.len())])
    );

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // Skip if trying to add ourselves
    if friend_public_key == network.public_key {
        return Err("Cannot add yourself as a friend".to_string());
    }

    // Friend requests are SEALED to the target's encryption key - a v3 invite
    // carries it. Old invites can't be used: sending the request in plaintext
    // would leak the new friendship to the entire mesh.
    let friend_encryption_key =
        encryption_public_key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| {
                "This invite was created by an older version of Cipher and can't be \
             used securely. Ask your friend to generate a new invite."
                    .to_string()
            })?;

    // Generate deterministic user_id from friend's public key
    let friend_user_id = super::types::SqliteUuid::from_public_key(&friend_public_key);
    let now = chrono::Utc::now().to_rfc3339();

    // Determine display name to use - verify signature if both name and sig provided
    let verified_display_name = match (&display_name, &signature) {
        (Some(name), Some(sig)) => {
            // Verify signature: "cipher-name:{display_name}:{public_key}"
            let sign_message = format!("cipher-name:{}:{}", name, friend_public_key);
            if Database::verify_signature(&sign_message, sig, &friend_public_key) {
                println!(
                    "[IROH] ✓ Display name '{}' verified with valid signature",
                    name
                );
                name.clone()
            } else {
                println!("[IROH] ⚠ Signature verification failed - using fallback name");
                format!("User_{}", &friend_public_key[..8])
            }
        }
        (Some(name), None) => {
            // Name provided but no signature - use with warning
            println!(
                "[IROH] ⚠ Display name '{}' not verified (no signature)",
                name
            );
            name.clone()
        }
        _ => {
            // No name provided - use fallback
            format!("User_{}", &friend_public_key[..8])
        }
    };

    // 1. Create stub user in database (if not exists)
    println!(
        "[IROH] Creating stub user in database with name: {}",
        verified_display_name
    );
    db.conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO users (id, display_name, public_key, encryption_public_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(public_key) DO UPDATE SET
            encryption_public_key = COALESCE(excluded.encryption_public_key, encryption_public_key),
            updated_at = excluded.updated_at",
            rusqlite::params![
                friend_user_id,
                &verified_display_name,
                &friend_public_key,
                &friend_encryption_key,
                &now,
                &now
            ],
        )
        .map_err(|e| format!("Failed to create user: {}", e))?;

    // 2. Create outgoing friend request (status = 'pending', we initiated it)
    // The friend must accept before it becomes 'accepted'
    println!("[IROH] Creating outgoing friend request in database...");
    db.conn.lock().unwrap().execute(
        "INSERT OR IGNORE INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'pending', ?2, ?4, ?5)",
        rusqlite::params![
            super::types::SqliteUuid::new(),
            network.user_id,
            friend_user_id,
            &now,
            &now
        ],
    ).map_err(|e| format!("Failed to create friend request: {}", e))?;

    // 3. If node_id provided, add peer to endpoint for connectivity
    // Relay URL is optional - DHT discovery can find the peer without it
    if let Some(node_id_str) = &node_id {
        println!("[IROH] Node ID provided - adding to endpoint...");

        // Parse NodeId
        if let Ok(peer_node_id) = node_id_str.parse::<iroh::EndpointId>() {
            // Parse relay URL if provided (optional)
            let relay_url_parsed = relay_url
                .as_ref()
                .and_then(|url| url.parse::<url::Url>().ok());

            // Construct EndpointAddr - relay is optional, DHT will discover it
            let mut node_addr = iroh::EndpointAddr::new(peer_node_id);
            if let Some(url) = relay_url_parsed {
                node_addr = node_addr.with_relay_url(url.into());
            }

            // Register the address so gossip can find the peer (replaces add_node_addr)
            network.register_peer_address(node_addr.clone());
            println!(
                "[IROH] ✓ Node address added to address book (relay: {})",
                relay_url
                    .as_ref()
                    .map(|_| "provided")
                    .unwrap_or("DHT discovery")
            );

            // CRITICAL: Join the new friend into the EXISTING content-topic
            // mesh via join_peers() on the current subscription. Simply calling
            // endpoint.connect() doesn't establish the gossip neighbor
            // relationship! (The old join_gossip_mesh_with_peer() re-subscribed
            // the global topic, leaving a stale stream handler racing the new
            // one - duplicate handlers and NeighborDown loops.)
            println!("[IROH] Joining new friend into the gossip mesh...");
            match network.add_peer_to_content_topic(peer_node_id).await {
                Ok(_) => {
                    println!("[IROH] ✓ Successfully joined gossip mesh with friend!");
                }
                Err(e) => {
                    println!("[IROH] Warning: Failed to join gossip mesh: {}", e);
                    println!("[IROH]   Friend may not be online, will retry via discovery loop");
                }
            }

            // Save peer address for persistent reconnection (node_id always, relay if available)
            if let Err(e) = db.save_friend_peer_address(
                network.user_id,
                friend_user_id,
                node_id_str,
                relay_url.as_ref().map(|s| s.as_str()).unwrap_or(""),
            ) {
                println!("[IROH] Warning: Failed to save friend peer address: {}", e);
            } else {
                println!("[IROH] ✓ Friend peer address saved");
            }
        }
    }

    // 4. Send the FriendRequest sealed to the friend's encryption key - only
    // they can even see that a request happened
    println!("[IROH] Sending sealed FriendRequest...");
    match network
        .send_friend_request_sealed(&friend_encryption_key)
        .await
    {
        Ok(_) => {
            println!("[IROH] ✓ Sealed FriendRequest sent");
            println!("[IROH]   Target: {}", friend_public_key);
        }
        Err(e) => {
            println!("[IROH] Warning: Failed to send FriendRequest: {}", e);
        }
    }

    println!("[IROH] ✓ Friend request sent successfully!");
    println!("[IROH]   User ID: {}", friend_user_id);

    Ok(friend_public_key)
}

/// Read blob data by hash - downloads from remote peer if not available locally
/// Used by frontend to fetch attachment data for encrypted posts (SealedEnvelope)
/// If encryption_key is provided, decrypts the blob after downloading
///
/// Note: Uses spawn_blocking to work around the non-Send AsyncSliceReader issue
#[tauri::command]
pub async fn iroh_read_blob(
    node_id: String,
    blob_hash: String,
    encryption_key: Option<String>,
) -> Result<String, String> {
    println!(
        "[BLOB-READ] Reading blob {} from node {} (encrypted: {})",
        blob_hash,
        node_id,
        encryption_key.is_some()
    );

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // Parse blob hash
    let hash_bytes = hex::decode(&blob_hash).map_err(|e| format!("Invalid blob hash: {}", e))?;

    if hash_bytes.len() != 32 {
        return Err("Blob hash must be 32 bytes".to_string());
    }

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(&hash_bytes);
    let hash = iroh_blobs::Hash::from_bytes(hash_arr);

    // Parse sender's NodeId for downloading
    let sender_node_id = node_id
        .parse::<iroh::EndpointId>()
        .map_err(|e| format!("Invalid node_id: {}", e))?;

    // Download from the remote peer and read from the local store, with up to
    // 3 attempts and short exponential backoff (500ms/1s/2s) - a single failed
    // attempt from a peer that was still connecting dropped the attachment.
    const MAX_ATTEMPTS: u32 = 3;
    let mut encrypted_data: Option<Vec<u8>> = None;
    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        // First, try to download the blob from the remote peer (ignore errors - it may
        // already be cached locally). The downloader resolves the peer via address lookups.
        if let Some(downloader) = network.downloader.lock().await.clone() {
            println!(
                "[BLOB-READ] Attempt {}/{}: downloading blob from peer...",
                attempt, MAX_ATTEMPTS
            );
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                downloader.download(hash, vec![sender_node_id]),
            )
            .await
            {
                Ok(Ok(_)) => println!("[BLOB-READ] Blob downloaded successfully"),
                Ok(Err(e)) => println!("[BLOB-READ] Download failed: {} - will try local store", e),
                Err(_) => println!("[BLOB-READ] Download timed out - will try local store"),
            }
        }

        // Read the blob from the local store. The new iroh-blobs read path is Send-safe,
        // so the previous spawn_blocking/LocalSet workaround is no longer needed.
        let read_result = {
            let store_guard = network.store.lock().await;
            let store = store_guard
                .as_ref()
                .ok_or_else(|| "Blob store not initialized".to_string())?;
            store.blobs().get_bytes(hash).await
        };
        match read_result {
            Ok(bytes) => {
                encrypted_data = Some(bytes.to_vec());
                break;
            }
            Err(e) => {
                last_error = format!("Failed to read blob: {}", e);
                println!(
                    "[BLOB-READ] Attempt {}/{} failed: {}",
                    attempt, MAX_ATTEMPTS, last_error
                );
                if attempt < MAX_ATTEMPTS {
                    let backoff = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
    let encrypted_data = encrypted_data
        .ok_or_else(|| format!("{} (after {} attempts)", last_error, MAX_ATTEMPTS))?;

    // Decrypt if encryption key provided
    let final_data = if let Some(key_b64) = encryption_key {
        println!("[BLOB-READ] Decrypting blob with provided key...");

        let plaintext = IrohNetwork::decrypt_blob_data(&encrypted_data, &key_b64)?;

        println!(
            "[BLOB-READ] Decrypted {} bytes -> {} bytes",
            encrypted_data.len(),
            plaintext.len()
        );
        plaintext
    } else {
        // No encryption key - return raw data (legacy or unencrypted blob)
        println!("[BLOB-READ] No encryption key provided, returning raw data");
        encrypted_data
    };

    // Return as base64
    let base64_data = general_purpose::STANDARD.encode(&final_data);
    println!(
        "[BLOB-READ] Successfully read blob, base64 length: {}",
        base64_data.len()
    );
    Ok(base64_data)
}

/// Signal app entering background - pauses P2P operations to prevent crashes during termination
/// Call this when visibilitychange fires with hidden=true or on pagehide
#[tauri::command]
pub async fn iroh_enter_background(db: State<'_, Database>) -> Result<String, String> {
    println!("[IROH] App entering background - pausing operations");

    // MOBILE ONLY: the OS may suspend or kill us at any point once backgrounded, so
    // stop new P2P operations to avoid async IPC writes to a destroyed WebView.
    //
    // On DESKTOP this command also fires on minimize/occlusion (visibilitychange),
    // where the app must STAY connected - shutting down P2P here made a minimized
    // window stop heartbeating, so peers marked us stale within 45s.
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        // Mark as not in foreground (enables next foreground call)
        APP_IN_FOREGROUND.store(false, Ordering::Relaxed);

        // Signal shutdown to prevent new operations from starting
        signal_app_shutdown();
    }

    // Checkpoint SQLite WAL to ensure all data is persisted to main database file
    // This prevents data loss if the app is terminated while backgrounded
    if let Err(e) = db.checkpoint() {
        println!("[IROH] Warning: WAL checkpoint failed: {}", e);
    }

    Ok("Background mode entered".to_string())
}

/// Signal app returning to foreground - resumes P2P operations
/// Call this when visibilitychange fires with hidden=false or on pageshow
#[tauri::command]
pub async fn iroh_enter_foreground() -> Result<String, String> {
    // ALWAYS clear the shutdown flag first - even on the debounced path. Returning
    // early with the flag still set (e.g. after a backgrounding signal that was never
    // paired with a foreground transition) blocked every P2P command indefinitely.
    reset_app_shutdown();

    // Debounce: skip if already in foreground
    if APP_IN_FOREGROUND.swap(true, Ordering::Relaxed) {
        println!("[IROH] App already in foreground - skipping duplicate call");
        return Ok("Already in foreground".to_string());
    }

    println!("[IROH] App entering foreground - resuming operations");

    // Perform full network recovery (restarts background loops, triggers sync)
    let network_opt = IROH_NETWORK.lock().unwrap().clone();
    if let Some(network) = network_opt {
        // Call full recover() which restarts all background loops and triggers device sync
        // Use 10 second timeout - recovery needs time for peer discovery
        match tokio::time::timeout(tokio::time::Duration::from_secs(10), network.recover()).await {
            Ok(Ok(())) => {
                println!("[IROH] Full recovery completed on foreground");
            }
            Ok(Err(e)) => {
                println!("[IROH] Recovery failed on foreground: {}", e);
            }
            Err(_) => {
                println!("[IROH] Recovery timed out on foreground (continuing in background)");
            }
        }
    }

    Ok("Foreground mode entered".to_string())
}
