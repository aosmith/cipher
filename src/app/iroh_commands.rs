// Tauri commands for Iroh P2P networking

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tauri::{Manager, State};

use super::iroh_network::{IrohNetwork, P2PMessage};
use super::types::SqliteUuid;
use super::Database;

lazy_static! {
    /// Global Iroh network instance
    pub static ref IROH_NETWORK: StdMutex<Option<Arc<IrohNetwork>>> = StdMutex::new(None);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub is_connected: bool,
}

/// Initialize Iroh network
#[tauri::command]
pub async fn iroh_initialize(
    app_handle: tauri::AppHandle,
    user_id: SqliteUuid,
    public_key: String,
    device_id: Option<String>,
    db: tauri::State<'_, Database>,
) -> Result<String, String> {
    println!("========================================");
    println!("iroh_initialize called");
    println!("   User ID: {}", user_id);
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

    // CRITICAL: Subscribe to discovery topics so we can receive presence announcements
    println!("[IROH-INIT] Subscribing to discovery topics...");
    network.ensure_discovery_subscribed().await?;
    println!("[IROH-INIT] Discovery topics subscribed");

    println!("[IROH-INIT] Storing network globally...");
    let network_arc = Arc::new(network);
    *IROH_NETWORK.lock().unwrap() = Some(network_arc);

    println!("========================================");
    println!("Iroh initialization COMPLETE");
    println!("========================================");

    Ok(format!("Iroh network initialized for user {}", user_id))
}

/// Subscribe to a friend's topic to receive their updates
#[tauri::command]
pub async fn iroh_subscribe_friend(friend_public_key: String) -> Result<String, String> {
    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // Subscribe to friend's topic (based on their public key)
    let topic = format!("cipher/user/{}", friend_public_key);
    network.subscribe_topic(&topic).await?;

    Ok("Subscribed to friend's topic".to_string())
}

/// Send a direct message to a user
#[tauri::command]
pub async fn iroh_send_message(
    to_user_id: SqliteUuid,
    to_public_key: String,
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

    // Publish to the recipient's topic
    let topic = format!("cipher/user/{}", to_public_key);
    network.publish_message(&topic, message).await?;

    Ok(message_id)
}

/// Publish a post to your own topic
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

    let message = P2PMessage::Post {
        user_id: network.user_id,
        content,
        timestamp: chrono::Utc::now().timestamp(),
        device_id: network.device_id.clone(),
        attachments,
    };

    // Publish to our own topic
    let topic = format!("cipher/user/{}", network.public_key);
    network.publish_message(&topic, message).await?;

    Ok("Post published".to_string())
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
/// Omits direct IP addresses to keep QR code scannable
#[tauri::command]
pub async fn iroh_generate_invite() -> Result<String, String> {
    println!("[IROH] Generating friend request QR code...");

    let network = IROH_NETWORK
        .lock()
        .unwrap()
        .as_ref()
        .ok_or("Iroh not initialized")?
        .clone();

    // CRITICAL: Subscribe to cipher/presence topic to receive discovery announcements
    network.ensure_discovery_subscribed().await?;

    // CRITICAL: Subscribe to our own topic when generating QR code
    // This ensures we can receive Presence messages when friends scan our QR
    let our_topic = format!("cipher/user/{}", network.public_key);
    if !network.is_topic_subscribed(&our_topic).await {
        println!("[IROH] Subscribing to our own topic to receive friend Presence messages...");
        match network.subscribe_topic(&our_topic).await {
            Ok(_) => {
                println!("[IROH] ✓ Subscribed to our own topic: {}", our_topic);
            }
            Err(e) => {
                return Err(format!("Failed to subscribe to own topic: {}", e));
            }
        }
    } else {
        println!("[IROH] Already subscribed to our own topic");
    }

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

    // Create compact URI with only essential information
    let qr_code = format!(
        "cipher://add-friend?key={}&node={}&relay={}",
        network.public_key,
        node_id,
        urlencoding::encode(&relay_url)
    );

    println!("[IROH] ✓ QR code generated successfully!");
    println!("[IROH]   Public Key: {}", network.public_key);
    println!("[IROH]   NodeId: {}", node_id);
    println!("[IROH]   Relay: {}", relay_url);
    println!("[IROH]   QR code length: {}", qr_code.len());

    Ok(qr_code)
}

/// Add a friend by public key with optional compact node info
/// Creates friendship in database, subscribes to their topic, and establishes peer connection
#[tauri::command]
pub async fn iroh_add_friend_by_public_key(
    friend_public_key: String,
    node_id: Option<String>,
    relay_url: Option<String>,
    db: State<'_, Database>,
) -> Result<String, String> {
    println!("[IROH] Adding friend by public key: {}", friend_public_key);

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

    // 1. Create stub user in database (if not exists)
    println!("[IROH] Creating stub user in database...");
    db.conn
        .lock()
        .unwrap()
        .execute(
            "INSERT OR IGNORE INTO users (id, username, public_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                friend_user_id,
                format!("User_{}", &friend_public_key[..8]), // Temporary display name
                &friend_public_key,
                &now,
                &now
            ],
        )
        .map_err(|e| format!("Failed to create user: {}", e))?;

    // 2. Create friendship (bidirectional, status = 'accepted')
    println!("[IROH] Creating friendship in database...");
    db.conn.lock().unwrap().execute(
        "INSERT OR IGNORE INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'accepted', ?2, ?4, ?5)",
        rusqlite::params![
            super::types::SqliteUuid::new(),
            network.user_id,
            friend_user_id,
            &now,
            &now
        ],
    ).map_err(|e| format!("Failed to create friendship: {}", e))?;

    // 3. If node info provided, add peer to endpoint for gossip bootstrap
    // Store peer address add success state to determine if we can use peer as bootstrap
    let peer_added = if let (Some(node_id_str), Some(relay_url_str)) = (&node_id, &relay_url) {
        println!("[IROH] Node info provided - establishing immediate peer connection...");

        // Parse NodeId
        let peer_node_id: iroh::NodeId = node_id_str
            .parse()
            .map_err(|e| format!("Failed to parse NodeId: {}", e))?;

        // Parse relay URL
        let relay_url_parsed: url::Url = relay_url_str
            .parse()
            .map_err(|e| format!("Failed to parse relay URL: {}", e))?;

        // Construct minimal NodeAddr with just NodeId and relay
        let node_addr = iroh::NodeAddr::from_parts(
            peer_node_id,
            Some(relay_url_parsed.into()), // Convert Url to RelayUrl
            vec![],                        // No direct addresses - let relay handle it
        );

        println!("[IROH-CONNECT] 🔗 Attempting connection to peer...");
        println!("[IROH-CONNECT]   Peer NodeId: {}", peer_node_id);
        println!("[IROH-CONNECT]   Relay: {:?}", node_addr.relay_url());
        println!("[IROH-CONNECT]   ALPN: {}", std::str::from_utf8(&iroh_gossip::ALPN).unwrap_or("invalid utf8"));

        // Add NodeAddr to endpoint
        // IMPORTANT: Following iroh-gossip chat example pattern - do NOT manually call endpoint.connect()
        // Only add_node_addr() and let the gossip protocol handle connections internally
        // Manual connections may interfere with gossip protocol's internal connection management
        let endpoint_guard = network.endpoint.lock().await;
        let addr_added = if let Some(endpoint) = endpoint_guard.as_ref() {
            println!("[IROH-CONNECT] Adding peer address to endpoint (gossip protocol will handle connection)...");
            if let Err(e) = endpoint.add_node_addr(node_addr.clone()) {
                println!("[IROH-CONNECT] ✗ Failed to add node address: {}", e);
                false
            } else {
                println!("[IROH-CONNECT] ✓ Node address added to endpoint");
                println!("[IROH-CONNECT]   Following iroh-gossip chat example: NOT manually connecting");
                println!("[IROH-CONNECT]   The gossip protocol will establish connections when subscribe_and_join() is called");
                true
            }
        } else {
            println!("[IROH-CONNECT] ✗ Endpoint not initialized!");
            false
        };
        drop(endpoint_guard);

        if addr_added {
            // CRITICAL: Save peer address for persistent reconnection on app restart
            println!("[IROH-CONNECT] Saving friend peer address to database...");
            if let Err(e) = db.save_friend_peer_address(
                network.user_id,
                friend_user_id,
                node_id_str,
                relay_url_str,
            ) {
                println!("[IROH-CONNECT] ✗ Failed to save friend peer address: {}", e);
            } else {
                println!("[IROH-CONNECT] ✓ Friend peer address saved successfully");
            }
        }

        addr_added
    } else {
        println!("[IROH] No node info provided - will discover peer via presence");
        false
    };

    // 4. Subscribe to friend's topic WITH peer as bootstrap
    // This ensures we join the same gossip mesh as the friend
    let friend_topic = format!("cipher/user/{}", friend_public_key);
    if peer_added {
        if let Some(node_id_str) = &node_id {
            if let Ok(peer_node_id) = node_id_str.parse::<iroh::NodeId>() {
                // CRITICAL: Resubscribe to shared topics WITH friend as bootstrap FIRST
                // This tears down our isolated root subscriptions and joins friend's mesh
                // Do this BEFORE subscribing to friend's topic for maximum mesh formation chance
                println!("[IROH] Resubscribing to shared topics WITH friend as bootstrap...");
                println!("[IROH] This will unsubscribe from isolated subscriptions and join friend's mesh");

                if let Err(e) = network.resubscribe_with_bootstrap("cipher/presence", vec![peer_node_id]).await {
                    println!("[IROH] ⚠️  Failed to resubscribe to cipher/presence: {}", e);
                } else {
                    println!("[IROH] ✓ Resubscribed to cipher/presence with friend - mesh should form!");
                }

                if let Err(e) = network.resubscribe_with_bootstrap("cipher/discovery/v1", vec![peer_node_id]).await {
                    println!("[IROH] ⚠️  Failed to resubscribe to cipher/discovery/v1: {}", e);
                } else {
                    println!("[IROH] ✓ Resubscribed to cipher/discovery/v1 with friend - mesh should form!");
                }

                // Now subscribe to friend's topic
                println!("[IROH] Subscribing to friend's topic WITH peer as bootstrap...");
                match network
                    .subscribe_with_bootstrap(&friend_topic, vec![peer_node_id])
                    .await
                {
                    Ok(_) => {
                        println!("[IROH] ✓ Subscribed to {} with bootstrap", friend_topic);

                        // 5. Send Presence to friend's topic via gossip
                        // Friend is already subscribed to their own topic, so they'll receive it
                        println!("[IROH] Sending Presence to friend's topic via gossip...");

                        let endpoint_guard = network.endpoint.lock().await;
                        if let Some(endpoint) = endpoint_guard.as_ref() {
                            if let Ok(our_node_addr) = endpoint.node_addr().await {
                                drop(endpoint_guard);

                                let presence = super::iroh_network::P2PMessage::Presence {
                                    user_id: network.user_id,
                                    public_key: network.public_key.clone(),
                                    device_id: network
                                        .device_id
                                        .clone()
                                        .unwrap_or_else(|| "unknown".to_string()),
                                    node_addr: our_node_addr.clone(),
                                    timestamp: chrono::Utc::now().timestamp(),
                                };

                                match network.publish_message(&friend_topic, presence).await {
                                    Ok(_) => {
                                        println!("[IROH] ✓ Presence sent via gossip to friend's topic!");
                                        println!("[IROH] Friend will receive and process our Presence");
                                    }
                                    Err(e) => {
                                        println!("[IROH] Warning: Failed to publish Presence: {}", e);
                                    }
                                }

                                // Send FriendAdded message to create bidirectional friendship
                                // Alice will receive this and auto-add Bob as a friend
                                println!("[IROH] Sending FriendAdded message for bidirectional friendship...");
                                let friend_added = super::iroh_network::P2PMessage::FriendAdded {
                                    from_public_key: network.public_key.clone(),
                                    from_user_id: network.user_id,
                                    from_node_id: our_node_addr.node_id.to_string(),
                                    from_relay_url: our_node_addr
                                        .relay_url()
                                        .map(|url| url.to_string())
                                        .unwrap_or_else(|| "https://euw1-1.relay.iroh.network.".to_string()),
                                    to_public_key: friend_public_key.clone(),
                                    timestamp: chrono::Utc::now().timestamp(),
                                };

                                match network.publish_message(&friend_topic, friend_added).await {
                                    Ok(_) => {
                                        println!("[IROH] ✓ FriendAdded message sent - friend will auto-add us!");
                                    }
                                    Err(e) => {
                                        println!("[IROH] Warning: Failed to send FriendAdded: {}", e);
                                    }
                                }
                            } else {
                                drop(endpoint_guard);
                            }
                        } else {
                            drop(endpoint_guard);
                        }
                    }
                    Err(e) => {
                        println!("[IROH] Warning: Failed to subscribe with bootstrap: {}", e);
                        println!("[IROH] Falling back to ensure_discovery_subscribed");
                        network.ensure_discovery_subscribed().await?;
                    }
                }
            }
        }
    } else {
        println!("[IROH] No peer connection - subscribing to friend's topic WITHOUT bootstrap...");
        // Even without bootstrap, subscribe to friend's topic so we receive their posts
        // When they connect and send Presence, handle_message will re-subscribe WITH them as bootstrap
        match network.subscribe_topic(&friend_topic).await {
            Ok(_) => {
                println!("[IROH] ✓ Subscribed to friend's topic (will update to bootstrap when they connect)");
            }
            Err(e) => {
                println!("[IROH] Warning: Failed to subscribe to friend's topic: {}", e);
            }
        }
        network.ensure_discovery_subscribed().await?;
    }

    // CRITICAL: Ensure we're subscribed to our own topic so friend can reach us
    // This is essential for manual adds where we might not have generated QR yet
    let our_topic = format!("cipher/user/{}", network.public_key);
    if !network.is_topic_subscribed(&our_topic).await {
        println!("[IROH] Ensuring our own topic is subscribed...");
        match network.subscribe_topic(&our_topic).await {
            Ok(_) => {
                println!("[IROH] ✓ Subscribed to our own topic");
            }
            Err(e) => {
                println!("[IROH] Warning: Failed to subscribe to our own topic: {}", e);
            }
        }
    } else {
        println!("[IROH] Already subscribed to our own topic");
    }

    // Note: Friend's topic subscription handled above (with or without bootstrap)
    println!("[IROH] Gossip mesh connection complete!");

    println!("[IROH] ✓ Friend added successfully!");
    println!("[IROH]   User ID: {}", friend_user_id);
    println!("[IROH]   Topic: {}", friend_topic);

    Ok(friend_public_key)
}
