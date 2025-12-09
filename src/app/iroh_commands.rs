// Tauri commands for Iroh P2P networking
// Global mesh architecture: all nodes on cipher/content/v1

use lazy_static::lazy_static;
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
    network.publish_message(CONTENT_TOPIC, message).await?;

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
        // No friends yet - still create post locally but don't broadcast encrypted content
        // Just broadcast the legacy Post message for backwards compatibility
        println!("[IROH] No friends with encryption keys - sending legacy Post");
        let message = P2PMessage::Post {
            user_id: network.user_id,
            public_key: network.public_key.clone(),
            content,
            timestamp: chrono::Utc::now().timestamp(),
            device_id: network.device_id.clone(),
            attachments,
        };
        network.publish_message(CONTENT_TOPIC, message).await?;
        return Ok("Post published (legacy)".to_string());
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
    let network_opt = IROH_NETWORK.lock().unwrap().clone();

    if network_opt.is_none() {
        return Ok(serde_json::json!({
            "listening": false,
            "connected_peers": 0
        }));
    }

    let network = network_opt.unwrap();
    network.get_connection_status().await
}

/// Shutdown Iroh network
#[tauri::command]
pub async fn iroh_shutdown() -> Result<String, String> {
    let network_opt = IROH_NETWORK.lock().unwrap().take();

    if let Some(network) = network_opt {
        network.shutdown().await?;
    }

    Ok("Iroh shut down".to_string())
}

/// Re-announce presence to the network
#[tauri::command]
pub async fn iroh_announce_presence() -> Result<String, String> {
    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    network.announce_presence().await?;

    Ok("Presence announced".to_string())
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

    // Extract compact info: NodeId and relay URL only (omit direct addresses)
    let node_id = node_addr.node_id.to_string();
    let relay_url = node_addr
        .relay_url()
        .map(|url| url.to_string())
        .unwrap_or_else(|| "https://euw1-1.relay.iroh.network.".to_string());

    // Get signing private key for signing the display name
    let (signing_private_key, _encryption_private_key) = db.get_user_keys(network.user_id)
        .map_err(|e| format!("Failed to get user keys: {}", e))?;

    // Sign: "cipher-name:{display_name}:{public_key}" to prove ownership
    let sign_message = format!("cipher-name:{}:{}", network.display_name, network.public_key);
    let signature = Database::sign_message(&sign_message, &signing_private_key)
        .map_err(|e| format!("Failed to sign display name: {}", e))?;

    // Create compact URI with signed display name
    let qr_code = format!(
        "cipher://add-friend?key={}&node={}&relay={}&name={}&sig={}",
        network.public_key,
        node_id,
        urlencoding::encode(&relay_url),
        urlencoding::encode(&network.display_name),
        urlencoding::encode(&signature)
    );

    println!("[IROH] ✓ QR code generated successfully!");
    println!("[IROH]   Public Key: {}", network.public_key);
    println!("[IROH]   Display Name: {} (signed)", network.display_name);
    println!("[IROH]   NodeId: {}", node_id);
    println!("[IROH]   Relay: {}", relay_url);
    println!("[IROH]   QR code length: {}", qr_code.len());

    Ok(qr_code)
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

                // Let the gossip protocol handle the connection internally
                // Don't call endpoint.connect() directly - let subscribe_and_join() do it
                // The gossip protocol needs to establish the connection itself for proper
                // neighbor relationship formation
                println!("[IROH] Forming gossip mesh with new friend as bootstrap...");
                println!("[IROH]   Letting gossip protocol handle connection establishment");
                match network.resubscribe_with_bootstrap(CONTENT_TOPIC, vec![peer_node_id]).await {
                    Ok(_) => {
                        println!("[IROH] ✓ Successfully joined gossip mesh with friend!");
                        network.add_connected_peer(peer_node_id).await;
                    }
                    Err(e) => {
                        println!("[IROH] Warning: Gossip mesh formation failed: {}", e);
                        println!("[IROH]   Friend may not be online, will retry via presence discovery");
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
