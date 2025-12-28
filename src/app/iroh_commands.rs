// Tauri commands for Iroh P2P networking
// Global mesh architecture: all nodes on cipher/content/v1

use lazy_static::lazy_static;
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
    if is_app_shutting_down() {
        return Err("App is shutting down".to_string());
    }

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // Get post attachments from database
    let attachments = db.get_post_media(post_id).ok();

    // Get our encryption keys
    let our_encryption_public_key = db
        .get_user_encryption_public_key(network.user_id)
        .map_err(|e| format!("Failed to get encryption public key: {}", e))?
        .ok_or("No encryption public key found")?;

    let our_encryption_private_key = db
        .get_user_encryption_private_key(network.user_id)
        .map_err(|e| format!("Failed to get encryption private key: {}", e))?
        .ok_or("No encryption private key found")?;

    // Get friend encryption public keys
    let friend_encryption_keys = network.get_friend_encryption_public_keys();

    if friend_encryption_keys.is_empty() {
        // No friends with encryption keys yet - use PostWithBlobs via gossip
        // ALL posts use iroh-blobs connection for NAT hole-punching benefit
        println!("[IROH] No friends with encryption keys - broadcasting PostWithBlobs to gossip mesh");

        // Get our node ID for blob fetching
        let node_id = network.get_node_id().await;

        // Store each attachment as a blob and create references
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

                // Store as blob
                match network.store_blob(data).await {
                    Ok(hash) => {
                        // Track storage used
                        let _ = db.add_storage_used(data_size);

                        let blob_ref = crate::app::types::BlobReference {
                            id: attachment.id,
                            file_type: attachment.file_type.clone(),
                            file_size: attachment.file_size,
                            blob_hash: hex::encode(hash.as_bytes()),
                            downloaded: true, // We're the sender, blob is local
                        };
                        blob_refs.push(blob_ref);
                        println!(
                            "[IROH] [OK] Stored attachment {} as blob {}",
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

        // Always use PostWithBlobs - iroh connection handles hole-punching
        let message = P2PMessage::PostWithBlobs {
            user_id: network.user_id,
            public_key: network.public_key.clone(),
            node_id,
            content,
            timestamp: chrono::Utc::now().timestamp(),
            device_id: network.device_id.clone(),
            blob_refs,
        };

        network.publish_message(CONTENT_TOPIC, message).await?;
        return Ok("Post published (PostWithBlobs)".to_string());
    }

    // PHASE 2: Create sealed envelope with boxes for each friend
    println!("[IROH] Creating sealed envelope for {} friends", friend_encryption_keys.len());

    let envelope = crate::app::crypto::GossipEnvelope::new_post(
        &our_encryption_public_key,
        &content,
        attachments,
        &friend_encryption_keys,
        &our_encryption_private_key,
    ).map_err(|e| format!("Failed to create sealed envelope: {}", e))?;

    // Serialize envelope to JSON
    let envelope_json = serde_json::to_string(&envelope)
        .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

    // Create SealedEnvelope message
    let message = P2PMessage::SealedEnvelope { envelope_json };

    // Broadcast to global mesh
    network.publish_message(CONTENT_TOPIC, message).await?;

    println!("[IROH] ✓ Sealed post published to {} friends", friend_encryption_keys.len());
    Ok("Post published (sealed)".to_string())
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
/// Returns a cipher:// URI with compact addressing info (public key, NodeId, relay)
/// Includes signed display name for verified identity
/// GLOBAL MESH: All nodes on cipher/content/v1 - just share connection info
#[tauri::command]
pub async fn iroh_generate_invite(
    db: State<'_, Database>,
) -> Result<String, String> {
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
    let endpoint_guard = network.endpoint.lock().await;
    let node_addr = if let Some(endpoint) = endpoint_guard.as_ref() {
        endpoint
            .node_addr()
            .await
            .map_err(|e| format!("Failed to get node address: {}", e))?
    } else {
        return Err("Endpoint not initialized".to_string());
    };
    drop(endpoint_guard);

    // Extract NodeId only - relay will be discovered via DHT/DNS
    let node_id = node_addr.node_id.to_string();

    // Get signing private key for signing the display name
    let (signing_private_key, _encryption_private_key) = db.get_user_keys(network.user_id)
        .map_err(|e| format!("Failed to get user keys: {}", e))?;

    // Sign: "cipher-name:{display_name}" - shorter message, pubkey implicit
    let sign_message = format!("cipher-name:{}", network.display_name);
    let signature = Database::sign_message(&sign_message, &signing_private_key)
        .map_err(|e| format!("Failed to sign display name: {}", e))?;

    // V2 format with signature: [32 pubkey][32 node][1 name_len][name][64 sig]
    // No relay (discovered via DHT/DNS)
    // No compression - random signature data gets LARGER with DEFLATE
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    // Decode keys to binary
    use base64::engine::general_purpose::STANDARD;
    let pubkey_bytes = STANDARD.decode(&network.public_key)
        .map_err(|e| format!("Invalid public key base64: {}", e))?;
    let node_bytes = hex::decode(&node_id)
        .map_err(|e| format!("Invalid node id hex: {}", e))?;
    let sig_bytes = STANDARD.decode(&signature)
        .map_err(|e| format!("Invalid signature base64: {}", e))?;

    // Build binary payload
    let mut payload = Vec::new();
    payload.extend_from_slice(&pubkey_bytes);  // 32 bytes
    payload.extend_from_slice(&node_bytes);     // 32 bytes

    // Display name (length-prefixed, max 255)
    let name_bytes = network.display_name.as_bytes();
    payload.push(name_bytes.len() as u8);
    payload.extend_from_slice(name_bytes);

    // Signature (64 bytes)
    payload.extend_from_slice(&sig_bytes);

    // Encode as base64url directly (no compression - random data expands with DEFLATE)
    let encoded = URL_SAFE_NO_PAD.encode(&payload);

    // Create compact URI
    let qr_code = format!("cipher://i/{}", encoded);

    println!("[IROH] ✓ QR code generated successfully!");
    println!("[IROH]   Public Key: {}", network.public_key);
    println!("[IROH]   Display Name: {} (signed)", network.display_name);
    println!("[IROH]   NodeId: {}", node_id);
    println!("[IROH]   Payload: {} bytes", payload.len());
    println!("[IROH]   QR code length: {} chars", qr_code.len());

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
/// Format: [32 pubkey][32 node][1 name_len][name][64 sig]
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

    // Minimum size: 32 + 32 + 1 + 64 = 129 bytes (with 0-length name)
    if payload.len() < 129 {
        return Err(format!("Payload too short: {} bytes", payload.len()));
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

    // Read display name (length-prefixed)
    let name_len = payload[pos] as usize;
    pos += 1;
    let display_name = if name_len > 0 && pos + name_len <= payload.len() {
        let name = String::from_utf8(payload[pos..pos + name_len].to_vec())
            .map_err(|_| "Invalid display name encoding")?;
        pos += name_len;
        Some(name)
    } else {
        None
    };

    // Read signature (64 bytes)
    let signature = if pos + 64 <= payload.len() {
        let sig_bytes = &payload[pos..pos + 64];
        Some(STANDARD.encode(sig_bytes))
    } else {
        None
    };

    println!("[IROH] ✓ Parsed v2 invite:");
    println!("[IROH]   Public Key: {}", public_key);
    println!("[IROH]   Node ID: {}", node_id);
    println!("[IROH]   Display Name: {:?}", display_name);
    println!("[IROH]   Signature: {}", if signature.is_some() { "present" } else { "none" });

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

    // 3. If node info provided, add peer to endpoint for better connectivity
    if let (Some(node_id_str), Some(relay_url_str)) = (&node_id, &relay_url) {
        println!("[IROH] Node info provided - adding to endpoint...");

        // Parse NodeId
        if let Ok(peer_node_id) = node_id_str.parse::<iroh::NodeId>() {
            // Parse relay URL
            if let Ok(relay_url_parsed) = relay_url_str.parse::<url::Url>() {
                // Construct minimal NodeAddr with just NodeId and relay
                let node_addr = iroh::NodeAddr::from_parts(
                    peer_node_id,
                    Some(relay_url_parsed.into()),
                    vec![],
                );

                // Add NodeAddr to endpoint - this enables gossip to find the peer
                let endpoint_guard = network.endpoint.lock().await;
                if let Some(endpoint) = endpoint_guard.as_ref() {
                    if let Err(e) = endpoint.add_node_addr(node_addr.clone()) {
                        println!("[IROH] Warning: Failed to add node address: {}", e);
                    } else {
                        println!("[IROH] ✓ Node address added to endpoint");
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

                // Save peer address for persistent reconnection
                if let Err(e) = db.save_friend_peer_address(
                    network.user_id,
                    friend_user_id,
                    node_id_str,
                    relay_url_str,
                ) {
                    println!("[IROH] Warning: Failed to save friend peer address: {}", e);
                } else {
                    println!("[IROH] ✓ Friend peer address saved");
                }
            }
        }
    }

    // 4. GLOBAL MESH: Send FriendRequest via global content topic
    println!("[IROH] Sending FriendRequest via global mesh...");
    let endpoint_guard = network.endpoint.lock().await;
    if let Some(endpoint) = endpoint_guard.as_ref() {
        if let Ok(our_node_addr) = endpoint.node_addr().await {
            drop(endpoint_guard);

            let friend_request = P2PMessage::FriendRequest {
                from_public_key: network.public_key.clone(),
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
        } else {
            drop(endpoint_guard);
        }
    } else {
        drop(endpoint_guard);
    }

    println!("[IROH] ✓ Friend request sent successfully!");
    println!("[IROH]   User ID: {}", friend_user_id);

    Ok(friend_public_key)
}

/// Read blob data from the local store by hash
/// Used by frontend to fetch attachment data for PostWithBlobs messages
/// The blob must have been downloaded already (via PostWithBlobs handler)
///
/// Note: Uses spawn_blocking to work around the non-Send AsyncSliceReader issue
#[tauri::command]
pub async fn iroh_read_blob(blob_hash: String) -> Result<String, String> {
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
                .ok_or_else(|| format!("Blob not found in store"))?;

            let size = entry.size().value() as usize;

            let mut reader = entry
                .data_reader()
                .await
                .map_err(|e| format!("Failed to get data reader: {}", e))?;

            let data = reader
                .read_at(0, size)
                .await
                .map_err(|e| format!("Failed to read blob: {}", e))?;

            // Return as base64
            let base64_data = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &data,
            );

            Ok::<String, String>(base64_data)
        });

        let _ = tx.send(result);
    });

    rx.await.map_err(|_| "Blob read task failed".to_string())?
}

/// Signal app entering background - pauses P2P operations to prevent crashes during termination
/// Call this when visibilitychange fires with hidden=true or on pagehide
#[tauri::command]
pub async fn iroh_enter_background() -> Result<String, String> {
    println!("[IROH] App entering background - pausing operations");

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
