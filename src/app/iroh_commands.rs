// Tauri commands for Iroh P2P networking
// Global mesh architecture: all nodes on cipher/content/v1

use base64::{engine::general_purpose, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    Key, XChaCha20Poly1305, XNonce,
};
use lazy_static::lazy_static;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};
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

        match tokio::time::timeout(
            tokio::time::Duration::from_secs($timeout_secs),
            $operation
        ).await {
            Ok(result) => {
                if is_app_shutting_down() {
                    return Err("App shutdown during operation".to_string());
                }
                result
            }
            Err(_) => {
                Err("Operation timed out".to_string())
            }
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
    println!("[IROH-INIT] Global mesh initialized - node joined {}", CONTENT_TOPIC);

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
    println!("[IROH] iroh_subscribe_friend called for {} - using global mesh", friend_public_key);

    // Still try to add friend's node address for better connectivity
    let peer_info = db.get_friend_peer_info_by_public_key(network.user_id, &friend_public_key).ok().flatten();
    if let Some((node_id, relay_url)) = peer_info {
        println!("[IROH] Adding friend peer info: NodeId={}, Relay={}", node_id, relay_url);
        if let Ok(peer_node_id) = node_id.parse::<iroh::NodeId>() {
            if let Ok(relay_url_parsed) = relay_url.parse::<url::Url>() {
                let node_addr = iroh::NodeAddr::from_parts(
                    peer_node_id,
                    Some(iroh::RelayUrl::from(relay_url_parsed)),
                    vec![],
                );
                let endpoint_guard = network.endpoint.lock().await;
                if let Some(endpoint) = endpoint_guard.as_ref() {
                    let _ = endpoint.add_node_addr(node_addr);
                    println!("[IROH] Added friend node address to endpoint");
                }
                drop(endpoint_guard);
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
    _to_public_key: String, // Not used in global mesh - filtering by to_user_id
    encrypted_content: String,
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

    let message = P2PMessage::DirectMessage {
        message_id: message_id.clone(),
        from_user_id: network.user_id,
        to_user_id,
        encrypted_content,
        timestamp: chrono::Utc::now().timestamp(),
        device_id: network.device_id.clone(),
    };

    // GLOBAL MESH: Broadcast to all nodes, recipient filters by to_user_id
    with_timeout!(P2P_OPERATION_TIMEOUT_SECS, network.publish_message(CONTENT_TOPIC, message))?;

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
    println!("[PUBLISH-POST] === START iroh_publish_post for post_id: {} ===", post_id);

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

    // Get post attachments from database
    println!("[PUBLISH-POST] Step 2: Getting post attachments from DB...");
    let attachments = db.get_post_media(post_id).ok();
    println!("[PUBLISH-POST] Step 2: DONE - Got {} attachments", attachments.as_ref().map(|a| a.len()).unwrap_or(0));

    // Get our encryption keys (public key fetched for future use with multi-device sync)
    println!("[PUBLISH-POST] Step 3: Getting encryption public key from DB...");
    let _our_encryption_public_key = db
        .get_user_encryption_public_key(network.user_id)
        .map_err(|e| format!("Failed to get encryption public key: {}", e))?
        .ok_or("No encryption public key found")?;
    println!("[PUBLISH-POST] Step 3: DONE - Got encryption public key");

    println!("[PUBLISH-POST] Step 4: Getting encryption private key from DB...");
    let our_encryption_private_key = db
        .get_user_encryption_private_key(network.user_id)
        .map_err(|e| format!("Failed to get encryption private key: {}", e))?
        .ok_or("No encryption private key found")?;
    println!("[PUBLISH-POST] Step 4: DONE - Got encryption private key");

    // Get friend encryption public keys
    println!("[PUBLISH-POST] Step 5: Getting friend encryption keys...");
    let friend_encryption_keys = network.get_friend_encryption_public_keys();
    println!("[PUBLISH-POST] Step 5: DONE - Got {} friend encryption keys", friend_encryption_keys.len());

    // Get our node ID for blob fetching (needed for both paths)
    println!("[PUBLISH-POST] Step 6: Getting node_id...");
    let node_id = network.get_node_id().await;
    println!("[PUBLISH-POST] Step 6: DONE - Got node_id: {}", &node_id[..8.min(node_id.len())]);

    // Store attachments as blobs (same path for both encrypted and unencrypted)
    let mut blob_refs = Vec::new();
    if let Some(ref atts) = attachments {
        println!(
            "[IROH] Post has {} attachments, storing as blobs via iroh",
            atts.len()
        );

        for attachment in atts {
            // Decode base64 data
            let data = match base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &attachment.data,
            ) {
                Ok(d) => d,
                Err(e) => {
                    println!(
                        "[IROH] Failed to decode attachment {}: {}",
                        attachment.id, e
                    );
                    continue;
                }
            };

            println!(
                "[IROH] Storing attachment {} ({} bytes) as blob...",
                attachment.id, data.len()
            );

            // Check storage quota before storing
            let data_size = data.len() as i64;
            if let Ok(can_store) = db.can_store(data_size) {
                if !can_store {
                    println!(
                        "[IROH] Storage quota exceeded, skipping attachment {}",
                        attachment.id
                    );
                    continue;
                }
            }

            // Encrypt blob data before storing (Signal/WhatsApp style)
            // Scope the encryption to avoid rng crossing await boundary
            let (encrypted_data, key_bytes) = {
                // Generate random 32-byte key for this blob
                let mut rng = rand::thread_rng();
                let mut key_bytes = [0u8; 32];
                rng.fill(&mut key_bytes);

                // Generate random 24-byte nonce
                let mut nonce_bytes = [0u8; 24];
                rng.fill(&mut nonce_bytes);

                // Encrypt with XChaCha20Poly1305
                let key = Key::from_slice(&key_bytes);
                let cipher = XChaCha20Poly1305::new(key);
                let nonce = XNonce::from_slice(&nonce_bytes);

                match cipher.encrypt(nonce, data.as_slice()) {
                    Ok(ciphertext) => {
                        // Prepend nonce to ciphertext (nonce is not secret)
                        let mut encrypted = nonce_bytes.to_vec();
                        encrypted.extend(ciphertext);
                        (encrypted, key_bytes)
                    }
                    Err(e) => {
                        println!(
                            "[IROH] Failed to encrypt attachment {}: {:?}",
                            attachment.id, e
                        );
                        continue;
                    }
                }
            };

            println!(
                "[IROH] Encrypted attachment {} ({} bytes -> {} bytes)",
                attachment.id, data.len(), encrypted_data.len()
            );

            // Store encrypted blob
            match network.store_blob(encrypted_data).await {
                Ok(hash) => {
                    // Track storage used
                    let _ = db.add_storage_used(data_size);

                    let blob_ref = crate::app::types::BlobReference {
                        id: attachment.id,
                        file_type: attachment.file_type.clone(),
                        file_size: attachment.file_size,
                        blob_hash: hex::encode(hash.as_bytes()),
                        downloaded: true, // We're the sender, blob is local
                        encryption_key: Some(general_purpose::STANDARD.encode(&key_bytes)),
                    };
                    blob_refs.push(blob_ref);
                    println!(
                        "[IROH] [OK] Stored encrypted attachment {} as blob {}",
                        attachment.id,
                        hex::encode(hash.as_bytes())
                    );
                }
                Err(e) => {
                    println!(
                        "[IROH] Failed to store attachment {} as blob: {}",
                        attachment.id, e
                    );
                }
            }
        }
    }

    if friend_encryption_keys.is_empty() {
        // No friends with encryption keys - skip P2P broadcast (post is saved locally)
        // Posts will be synced via device sync when friends are added
        println!("[PUBLISH-POST] No friends with encryption keys - skipping P2P broadcast (post saved locally)");
        println!("[PUBLISH-POST] === END iroh_publish_post SUCCESS (local only) ===");
        return Ok("Post saved locally (no friends to broadcast to)".to_string());
    }

    // PHASE 2: Create sealed envelope with boxes for each friend
    println!("[PUBLISH-POST] Step 7 (PHASE 2): Creating sealed envelope for {} friends...", friend_encryption_keys.len());

    // CRITICAL: Use signing public key (Ed25519) for sender identification, NOT encryption key (X25519)
    // The encryption key is only used for creating the sealed boxes
    let envelope = crate::app::crypto::GossipEnvelope::new_post(
        &network.public_key,
        &post_id.to_string(),
        &content,
        &node_id,
        &blob_refs,
        &friend_encryption_keys,
        &our_encryption_private_key,
    ).map_err(|e| format!("Failed to create sealed envelope: {}", e))?;
    println!("[PUBLISH-POST] Step 6 (PHASE 2): DONE - Envelope created");

    // Serialize envelope to JSON
    println!("[PUBLISH-POST] Step 7 (PHASE 2): Serializing envelope to JSON...");
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;
    println!("[PUBLISH-POST] Step 7 (PHASE 2): DONE - Serialized ({} bytes)", envelope_json.len());

    // Create SealedEnvelope message
    let message = P2PMessage::SealedEnvelope { envelope_json };

    // Broadcast to global mesh
    println!("[PUBLISH-POST] Step 8 (PHASE 2): Calling publish_message().await...");
    network.publish_message(CONTENT_TOPIC, message).await?;
    println!("[PUBLISH-POST] Step 8 (PHASE 2): DONE - Message published");

    println!("[PUBLISH-POST] === END iroh_publish_post SUCCESS (sealed) ===");
    Ok("Post published (sealed)".to_string())
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
    println!("[PUBLISH-COMMENT] Publishing comment {} on post {}", comment_id, post_id);

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
        // No friends with encryption keys - use unified Content message (same transport as posts)
        println!("[PUBLISH-COMMENT] No friends with encryption keys - using Content message");

        let node_id = network.get_node_id().await;
        let payload = serde_json::json!({
            "comment_id": comment_id.to_string(),
            "post_id": post_id.to_string(),
            "content": content,
            "parent_comment_id": parent_comment_id.map(|id| id.to_string())
        });

        let message = P2PMessage::Content {
            content_type: "comment".to_string(),
            user_id: network.user_id,
            public_key: network.public_key.clone(),
            node_id,
            payload_json: payload.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            device_id: network.device_id.clone(),
            blob_refs: vec![],
        };

        network.publish_message(CONTENT_TOPIC, message).await?;
        println!("[PUBLISH-COMMENT] ✓ Comment published via Content message");
        return Ok("Comment published".to_string());
    }

    // Get our encryption keys for sealed envelope
    let our_encryption_private_key = db
        .get_user_encryption_private_key(network.user_id)
        .map_err(|e| format!("Failed to get encryption private key: {}", e))?
        .ok_or("No encryption private key found")?;

    // Create sealed envelope
    // CRITICAL: Use signing public key (Ed25519) for sender identification, NOT encryption key (X25519)
    let envelope = crate::app::crypto::GossipEnvelope::new_post_comment(
        &network.public_key,
        &comment_id.to_string(),
        &post_id.to_string(),
        &content,
        parent_comment_id.as_ref().map(|id| id.to_string()).as_deref(),
        &friend_encryption_keys,
        &our_encryption_private_key,
    ).map_err(|e| format!("Failed to create sealed envelope: {}", e))?;

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
    println!("[PUBLISH-REACTION] Publishing reaction {} {} on post {}", action, emoji, post_id);

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
        // No friends with encryption keys - use unified Content message (same transport as posts)
        println!("[PUBLISH-REACTION] No friends with encryption keys - using Content message");

        let node_id = network.get_node_id().await;
        let payload = serde_json::json!({
            "post_id": post_id.to_string(),
            "emoji": emoji,
            "action": action
        });

        let message = P2PMessage::Content {
            content_type: "reaction".to_string(),
            user_id: network.user_id,
            public_key: network.public_key.clone(),
            node_id,
            payload_json: payload.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            device_id: network.device_id.clone(),
            blob_refs: vec![],
        };

        network.publish_message(CONTENT_TOPIC, message).await?;
        println!("[PUBLISH-REACTION] ✓ Reaction published via Content message");
        return Ok("Reaction published".to_string());
    }

    // Get our encryption keys for sealed envelope
    let our_encryption_private_key = db
        .get_user_encryption_private_key(network.user_id)
        .map_err(|e| format!("Failed to get encryption private key: {}", e))?
        .ok_or("No encryption private key found")?;

    // Create sealed envelope
    // CRITICAL: Use signing public key (Ed25519) for sender identification, NOT encryption key (X25519)
    let envelope = crate::app::crypto::GossipEnvelope::new_post_reaction(
        &network.public_key,
        &post_id.to_string(),
        &emoji,
        &action,
        &friend_encryption_keys,
        &our_encryption_private_key,
    ).map_err(|e| format!("Failed to create sealed envelope: {}", e))?;

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
        network.get_connection_status()
    ).await {
        Ok(result) => result,
        Err(_) => Ok(serde_json::json!({
            "listening": false,
            "connected_peers": 0,
            "error": "Status check timed out"
        }))
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
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(2),
            network.shutdown()
        ).await {
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
        network.health_check()
    ).await;

    match result {
        Ok((is_healthy, needs_reconnect, status)) => {
            Ok(serde_json::json!({
                "healthy": is_healthy,
                "needs_reconnect": needs_reconnect,
                "details": status
            }))
        }
        Err(_) => {
            Ok(serde_json::json!({
                "healthy": false,
                "needs_reconnect": true,
                "error": "Health check timed out"
            }))
        }
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
        endpoint
            .node_addr()
            .await
            .map_err(|e| format!("Failed to get node address: {}", e))?
    } else {
        return Err("Endpoint not initialized".to_string());
    };

    // Extract NodeId (relay discovered via DHT, not in invite)
    let node_id = node_addr.node_id.to_string();
    let _relay_url = node_addr.relay_url().map(|url| url.to_string());

    // V2 ultra-minimal format: [32 pubkey][32 node] = 64 bytes = 97 chars
    // No relay (DHT discovery), no name (comes via Presence/FriendRequest gossip)
    // Guaranteed to fit in SMS (160 char limit)
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;

    // Decode keys to binary
    let pubkey_bytes = STANDARD.decode(&network.public_key)
        .map_err(|e| format!("Invalid public key base64: {}", e))?;
    let node_bytes = hex::decode(&node_id)
        .map_err(|e| format!("Invalid node id hex: {}", e))?;

    // Build ultra-minimal binary payload
    let mut payload = Vec::new();
    payload.extend_from_slice(&pubkey_bytes);  // 32 bytes
    payload.extend_from_slice(&node_bytes);     // 32 bytes

    // Encode as base64url
    let encoded = URL_SAFE_NO_PAD.encode(&payload);

    // Create ultra-minimal URI
    let qr_code = format!("cipher://i/{}", encoded);

    println!("[IROH] ✓ QR code generated (ultra-minimal, 97 chars)");
    println!("[IROH]   Public Key: {}", network.public_key);
    println!("[IROH]   NodeId: {}", node_id);
    println!("[IROH]   Display name will come via gossip");

    Ok(qr_code)
}

/// Parsed invite code data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedInvite {
    pub public_key: String,
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
    println!("[IROH] Parsing invite code: {}...", &invite_code[..invite_code.len().min(50)]);

    if invite_code.starts_with("cipher://i/") {
        // V2 minimal format (no relay, no signature)
        parse_minimal_invite(&invite_code)
    } else if invite_code.starts_with("cipher://f/") {
        // V1 compressed format (with relay and signature)
        parse_compressed_invite(&invite_code)
    } else if invite_code.starts_with("cipher://add-friend?") {
        // Legacy URL parameter format
        parse_legacy_invite(&invite_code)
    } else {
        Err("Invalid invite code format. Must start with cipher://".to_string())
    }
}

/// Parse V2 format: cipher://i/{base64url_data}
/// Ultra-minimal: [32 pubkey][32 node] = 64 bytes only
/// Optional legacy: [32 pubkey][32 node][1 name_len][name][64 sig]
/// No relay (discovered via DHT/DNS), no compression
fn parse_minimal_invite(invite_code: &str) -> Result<ParsedInvite, String> {
    use base64::engine::general_purpose::{URL_SAFE_NO_PAD, STANDARD};
    use base64::Engine;

    // Extract base64url data after cipher://i/
    let encoded = invite_code.strip_prefix("cipher://i/")
        .ok_or("Invalid v2 invite format")?;

    // Decode base64url directly (no compression)
    let payload = URL_SAFE_NO_PAD.decode(encoded)
        .map_err(|e| format!("Failed to decode base64url: {}", e))?;

    // Minimum size: 32 + 32 = 64 bytes (ultra-minimal)
    if payload.len() < 64 {
        return Err(format!("Payload too short: {} bytes (need at least 64)", payload.len()));
    }

    let mut pos = 0;

    // Read public key (32 bytes) - encode as base64 to match app format
    let pubkey_bytes = &payload[pos..pos + 32];
    let public_key = STANDARD.encode(pubkey_bytes);
    pos += 32;

    // Read node ID (32 bytes) - encode as hex
    let node_bytes = &payload[pos..pos + 32];
    let node_id = hex::encode(node_bytes);
    pos += 32;

    // Optional: Read display name (length-prefixed) - only if payload is longer
    let display_name = if pos < payload.len() {
        let name_len = payload[pos] as usize;
        pos += 1;
        if name_len > 0 && pos + name_len <= payload.len() {
            let name = String::from_utf8(payload[pos..pos + name_len].to_vec())
                .map_err(|_| "Invalid display name encoding")?;
            pos += name_len;
            Some(name)
        } else {
            None
        }
    } else {
        None
    };

    // Optional: Read signature (64 bytes) - only if payload is longer
    let signature = if pos + 64 <= payload.len() {
        let sig_bytes = &payload[pos..pos + 64];
        Some(STANDARD.encode(sig_bytes))
    } else {
        None
    };

    println!("[IROH] ✓ Parsed v2 invite (ultra-minimal):");
    println!("[IROH]   Public Key: {}", public_key);
    println!("[IROH]   Node ID: {}", node_id);
    println!("[IROH]   Display Name: {:?} (from FriendRequest if None)", display_name);
    println!("[IROH]   Signature: {}", if signature.is_some() { "present" } else { "from FriendRequest" });

    Ok(ParsedInvite {
        public_key,
        node_id,
        relay_url: None, // Discovered via DHT/DNS
        display_name,
        signature,
    })
}

/// Parse V1 compressed format: cipher://f/{base64url_deflate_data}
fn parse_compressed_invite(invite_code: &str) -> Result<ParsedInvite, String> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use flate2::read::DeflateDecoder;
    use std::io::Read;

    // Extract base64url data after cipher://f/
    let encoded = invite_code.strip_prefix("cipher://f/")
        .ok_or("Invalid compressed invite format")?;

    // Decode base64url
    let compressed = URL_SAFE_NO_PAD.decode(encoded)
        .map_err(|e| format!("Failed to decode base64url: {}", e))?;

    // Decompress DEFLATE
    let mut decoder = DeflateDecoder::new(&compressed[..]);
    let mut payload = Vec::new();
    decoder.read_to_end(&mut payload)
        .map_err(|e| format!("Failed to decompress: {}", e))?;

    // Parse binary format: [32 pubkey][32 node][1 relay_len][relay][1 name_len][name][64 sig]
    // Minimum size: 32 + 32 + 1 + 0 + 1 + 0 + 64 = 130 bytes
    if payload.len() < 130 {
        return Err(format!("Payload too short: {} bytes", payload.len()));
    }

    let mut pos = 0;

    // Read public key (32 bytes) - encode as base64 to match app format
    use base64::engine::general_purpose::STANDARD;
    let pubkey_bytes = &payload[pos..pos + 32];
    let public_key = STANDARD.encode(pubkey_bytes);
    pos += 32;

    // Read node ID (32 bytes) - encode as hex
    let node_bytes = &payload[pos..pos + 32];
    let node_id = hex::encode(node_bytes);
    pos += 32;

    // Read relay URL (length-prefixed)
    let relay_len = payload[pos] as usize;
    pos += 1;
    if pos + relay_len > payload.len() {
        return Err("Invalid relay URL length".to_string());
    }
    let relay_url = if relay_len > 0 {
        Some(String::from_utf8(payload[pos..pos + relay_len].to_vec())
            .map_err(|_| "Invalid relay URL encoding")?)
    } else {
        None
    };
    pos += relay_len;

    // Read display name (length-prefixed)
    let name_len = payload[pos] as usize;
    pos += 1;
    if pos + name_len > payload.len() {
        return Err("Invalid display name length".to_string());
    }
    let display_name = if name_len > 0 {
        Some(String::from_utf8(payload[pos..pos + name_len].to_vec())
            .map_err(|_| "Invalid display name encoding")?)
    } else {
        None
    };
    pos += name_len;

    // Read signature (64 bytes) - encode as base64 to match verify_signature format
    if pos + 64 > payload.len() {
        return Err("Invalid signature length".to_string());
    }
    let sig_bytes = &payload[pos..pos + 64];
    let signature = Some(STANDARD.encode(sig_bytes));

    println!("[IROH] ✓ Parsed compressed invite:");
    println!("[IROH]   Public Key: {}", public_key);
    println!("[IROH]   Node ID: {}", node_id);
    println!("[IROH]   Relay: {:?}", relay_url);
    println!("[IROH]   Display Name: {:?}", display_name);

    Ok(ParsedInvite {
        public_key,
        node_id,
        relay_url,
        display_name,
        signature,
    })
}

/// Parse old URL parameter format: cipher://add-friend?key=...&node=...&relay=...&name=...&sig=...
fn parse_legacy_invite(invite_code: &str) -> Result<ParsedInvite, String> {
    let query_part = invite_code.strip_prefix("cipher://add-friend?")
        .ok_or("Invalid legacy invite format")?;

    let mut public_key = None;
    let mut node_id = None;
    let mut relay_url = None;
    let mut display_name = None;
    let mut signature = None;

    for param in query_part.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            let decoded = urlencoding::decode(value)
                .map_err(|_| format!("Invalid encoding for {}", key))?
                .to_string();
            match key {
                "key" => public_key = Some(decoded),
                "node" => node_id = Some(decoded),
                "relay" => relay_url = Some(decoded),
                "name" | "display_name" => display_name = Some(decoded),
                "sig" | "signature" => signature = Some(decoded),
                _ => {} // Ignore unknown parameters
            }
        }
    }

    let public_key = public_key.ok_or("Missing public key in invite")?;
    let node_id = node_id.ok_or("Missing node ID in invite")?;

    println!("[IROH] ✓ Parsed legacy invite:");
    println!("[IROH]   Public Key: {}", public_key);
    println!("[IROH]   Node ID: {}", node_id);
    println!("[IROH]   Relay: {:?}", relay_url);
    println!("[IROH]   Display Name: {:?}", display_name);

    Ok(ParsedInvite {
        public_key,
        node_id,
        relay_url,
        display_name,
        signature,
    })
}

/// Add a friend by public key with optional compact node info
/// Verifies signed display name if provided
/// GLOBAL MESH: Creates friendship in database and sends FriendRequest to global mesh
#[tauri::command]
pub async fn iroh_add_friend_by_public_key(
    friend_public_key: String,
    node_id: Option<String>,
    relay_url: Option<String>,
    display_name: Option<String>,
    signature: Option<String>,
    db: State<'_, Database>,
) -> Result<String, String> {
    println!("[IROH] Adding friend by public key: {} (global mesh)", friend_public_key);
    println!("[IROH] Parameters: node_id={:?}, relay_url={:?}, display_name={:?}, signature={:?}",
        node_id, relay_url, display_name, signature.as_ref().map(|s| &s[..20.min(s.len())]));

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

    // Generate deterministic user_id from friend's public key
    let friend_user_id = super::types::SqliteUuid::from_public_key(&friend_public_key);
    let now = chrono::Utc::now().to_rfc3339();

    // Determine display name to use - verify signature if both name and sig provided
    let verified_display_name = match (&display_name, &signature) {
        (Some(name), Some(sig)) => {
            // Verify signature: "cipher-name:{display_name}:{public_key}"
            let sign_message = format!("cipher-name:{}:{}", name, friend_public_key);
            if Database::verify_signature(&sign_message, sig, &friend_public_key) {
                println!("[IROH] ✓ Display name '{}' verified with valid signature", name);
                name.clone()
            } else {
                println!("[IROH] ⚠ Signature verification failed - using fallback name");
                format!("User_{}", &friend_public_key[..8])
            }
        }
        (Some(name), None) => {
            // Name provided but no signature - use with warning
            println!("[IROH] ⚠ Display name '{}' not verified (no signature)", name);
            name.clone()
        }
        _ => {
            // No name provided - use fallback
            format!("User_{}", &friend_public_key[..8])
        }
    };

    // 1. Create stub user in database (if not exists)
    println!("[IROH] Creating stub user in database with name: {}", verified_display_name);
    db.conn
        .lock()
        .unwrap()
        .execute(
            "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                friend_user_id,
                &verified_display_name,
                &friend_public_key,
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
        if let Ok(peer_node_id) = node_id_str.parse::<iroh::NodeId>() {
            // Parse relay URL if provided (optional)
            let relay_url_parsed = relay_url.as_ref().and_then(|url| url.parse::<url::Url>().ok());

            // Construct NodeAddr - relay is optional, DHT will discover it
            let node_addr = iroh::NodeAddr::from_parts(
                peer_node_id,
                relay_url_parsed.map(|u| u.into()),
                vec![],
            );

            // Add NodeAddr to endpoint - this enables gossip to find the peer
            let endpoint_guard = network.endpoint.lock().await;
            if let Some(endpoint) = endpoint_guard.as_ref() {
                if let Err(e) = endpoint.add_node_addr(node_addr.clone()) {
                    println!("[IROH] Warning: Failed to add node address: {}", e);
                } else {
                    println!("[IROH] ✓ Node address added to endpoint (relay: {})",
                        relay_url.as_ref().map(|_| "provided").unwrap_or("DHT discovery"));
                }
            }
            drop(endpoint_guard);

            // CRITICAL: Join the gossip mesh with the new friend as bootstrap
            // This is the INITIATING device (scanning QR code), so we need to actively
            // join the peer's gossip mesh via subscribe_and_join().
            // Simply calling endpoint.connect() doesn't establish gossip neighbor relationship!
            println!("[IROH] Joining gossip mesh with new friend as bootstrap...");
            match network.join_gossip_mesh_with_peer(peer_node_id).await {
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

    // 4. GLOBAL MESH: Send FriendRequest via global content topic
    println!("[IROH] Sending FriendRequest via global mesh...");
    // Clone endpoint to avoid holding lock during await
    let endpoint_clone = network.endpoint.lock().await.clone();
    if let Some(endpoint) = endpoint_clone.as_ref() {
        if let Ok(our_node_addr) = endpoint.node_addr().await {

            // Get our encryption public key for sealed envelope encryption (comments, reactions)
            let our_encryption_public_key = db
                .get_user_encryption_public_key(network.user_id)
                .ok()
                .flatten()
                .unwrap_or_default();

            let friend_request = P2PMessage::FriendRequest {
                from_public_key: network.public_key.clone(),
                from_encryption_public_key: our_encryption_public_key,
                from_user_id: network.user_id,
                from_display_name: network.display_name.clone(),
                from_node_id: our_node_addr.node_id.to_string(),
                from_relay_url: our_node_addr
                    .relay_url()
                    .map(|url| url.to_string())
                    .unwrap_or_else(|| "https://euw1-1.relay.iroh.network.".to_string()),
                to_public_key: friend_public_key.clone(),
                timestamp: chrono::Utc::now().timestamp(),
            };

            match network.publish_message(CONTENT_TOPIC, friend_request).await {
                Ok(_) => {
                    println!("[IROH] ✓ FriendRequest sent via global mesh!");
                    println!("[IROH]   Target: {}", friend_public_key);
                    println!("[IROH]   All nodes see it, only target processes it");
                }
                Err(e) => {
                    println!("[IROH] Warning: Failed to send FriendRequest: {}", e);
                }
            }
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
pub async fn iroh_read_blob(node_id: String, blob_hash: String, encryption_key: Option<String>) -> Result<String, String> {
    println!("[BLOB-READ] Reading blob {} from node {} (encrypted: {})", blob_hash, node_id, encryption_key.is_some());

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // Parse blob hash
    let hash_bytes = hex::decode(&blob_hash)
        .map_err(|e| format!("Invalid blob hash: {}", e))?;

    if hash_bytes.len() != 32 {
        return Err("Blob hash must be 32 bytes".to_string());
    }

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(&hash_bytes);
    let hash = iroh_blobs::Hash::from_bytes(hash_arr);

    // Parse sender's NodeId for downloading
    let sender_node_id = node_id.parse::<iroh::NodeId>()
        .map_err(|e| format!("Invalid node_id: {}", e))?;

    // First, try to download the blob from the remote peer
    {
        let blobs_guard = network.blobs.lock().await;
        if let Some(blobs) = blobs_guard.as_ref() {
            let downloader = blobs.downloader().clone();
            drop(blobs_guard); // Release lock before async operation

            println!("[BLOB-READ] Attempting to download blob from peer...");
            let request = iroh_blobs::downloader::DownloadRequest::new(
                iroh_blobs::HashAndFormat::raw(hash),
                vec![sender_node_id],
            );
            let handle = downloader.queue(request).await;

            // Wait for download with timeout (30 seconds for larger files)
            match tokio::time::timeout(std::time::Duration::from_secs(30), handle).await {
                Ok(Ok(stats)) => {
                    println!("[BLOB-READ] Blob downloaded successfully ({} bytes)", stats.bytes_read);
                }
                Ok(Err(e)) => {
                    println!("[BLOB-READ] Download failed: {} - will try local store", e);
                }
                Err(_) => {
                    println!("[BLOB-READ] Download timed out - will try local store");
                }
            }
        }
    }

    // Use a oneshot channel to communicate with a LocalSet task
    let (tx, rx) = tokio::sync::oneshot::channel();
    let blobs_arc = network.blobs.clone();

    // Spawn a new runtime in a blocking task to run the non-Send future
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build runtime");

        let local = tokio::task::LocalSet::new();
        let result = local.block_on(&rt, async move {
            use iroh_blobs::store::bao_tree::io::fsm::AsyncSliceReader;
            use iroh_blobs::store::{Map, MapEntry};

            let blobs_guard = blobs_arc.lock().await;
            let blobs = blobs_guard
                .as_ref()
                .ok_or_else(|| "Blob store not initialized".to_string())?;

            // Read from store
            let store = blobs.store();
            let entry = store
                .get(&hash)
                .await
                .map_err(|e| format!("Failed to get blob entry: {}", e))?
                .ok_or_else(|| format!("Blob not found in store after download attempt"))?;

            let size = entry.size().value() as usize;
            println!("[BLOB-READ] Reading {} bytes from local store", size);

            let mut reader = entry
                .data_reader()
                .await
                .map_err(|e| format!("Failed to get data reader: {}", e))?;

            let data = reader
                .read_at(0, size)
                .await
                .map_err(|e| format!("Failed to read blob: {}", e))?;

            println!("[BLOB-READ] Read {} bytes from store", data.len());
            Ok::<Vec<u8>, String>(data.to_vec())
        });

        let _ = tx.send(result);
    });

    let encrypted_data = rx.await.map_err(|_| "Blob read task failed".to_string())??;

    // Decrypt if encryption key provided
    let final_data = if let Some(key_b64) = encryption_key {
        println!("[BLOB-READ] Decrypting blob with provided key...");

        // Decode key
        let key_bytes = general_purpose::STANDARD
            .decode(&key_b64)
            .map_err(|e| format!("Invalid encryption key: {}", e))?;

        if key_bytes.len() != 32 {
            return Err("Encryption key must be 32 bytes".to_string());
        }

        // Extract nonce (first 24 bytes) and ciphertext
        if encrypted_data.len() < 24 {
            return Err("Encrypted blob too short (missing nonce)".to_string());
        }

        let (nonce_bytes, ciphertext) = encrypted_data.split_at(24);

        // Decrypt
        let key = Key::from_slice(&key_bytes);
        let cipher = XChaCha20Poly1305::new(key);
        let nonce = XNonce::from_slice(nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| "Blob decryption failed - invalid key or corrupted data")?;

        println!("[BLOB-READ] Decrypted {} bytes -> {} bytes", encrypted_data.len(), plaintext.len());
        plaintext
    } else {
        // No encryption key - return raw data (legacy or unencrypted blob)
        println!("[BLOB-READ] No encryption key provided, returning raw data");
        encrypted_data
    };

    // Return as base64
    let base64_data = general_purpose::STANDARD.encode(&final_data);
    println!("[BLOB-READ] Successfully read blob, base64 length: {}", base64_data.len());
    Ok(base64_data)
}

/// Signal app entering background - pauses P2P operations to prevent crashes during termination
/// Call this when visibilitychange fires with hidden=true or on pagehide
#[tauri::command]
pub async fn iroh_enter_background() -> Result<String, String> {
    println!("[IROH] App entering background - pausing operations");

    // Mark as not in foreground (enables next foreground call)
    APP_IN_FOREGROUND.store(false, Ordering::Relaxed);

    // Signal shutdown to prevent new operations from starting
    // This helps avoid the race condition where async IPC responses
    // try to write to a destroyed WebView during termination
    signal_app_shutdown();

    Ok("Background mode entered".to_string())
}

/// Signal app returning to foreground - resumes P2P operations
/// Call this when visibilitychange fires with hidden=false or on pageshow
#[tauri::command]
pub async fn iroh_enter_foreground() -> Result<String, String> {
    // Debounce: skip if already in foreground
    if APP_IN_FOREGROUND.swap(true, Ordering::Relaxed) {
        println!("[IROH] App already in foreground - skipping duplicate call");
        return Ok("Already in foreground".to_string());
    }

    println!("[IROH] App entering foreground - resuming operations");

    // Reset shutdown flag to allow operations again
    reset_app_shutdown();

    // Try to recover network if it was paused
    let network_opt = IROH_NETWORK.lock().unwrap().clone();
    if let Some(network) = network_opt {
        // Also reset the network's shutdown flag
        network.shutdown_flag.store(false, Ordering::Relaxed);

        // Don't wait for recovery - let it happen in background
        // Just trigger it if we can
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            network.announce_presence()
        ).await;
    }

    Ok("Foreground mode entered".to_string())
}
