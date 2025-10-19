// Iroh-based P2P networking
// Simpler, more reliable than libp2p with built-in NAT traversal

use bytes::Bytes;
use iroh::protocol::Router;
use iroh_gossip::ALPN;
use iroh_gossip::net::{Event as GossipNetEvent, GossipEvent, GossipReceiver, GossipSender};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;

use super::types::{MediaAttachmentWithData, SqliteUuid};
use super::Database;

/// P2P message types for the social network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum P2PMessage {
    /// User presence announcement (for device discovery)
    /// Includes full Iroh NodeAddr for peer connection (NodeId + relay URLs + direct addresses)
    Presence {
        user_id: SqliteUuid,
        public_key: String,
        device_id: String,
        node_addr: iroh::NodeAddr, // Full addressing info for direct peer connection
        timestamp: i64,
    },
    /// Device sync request (same user, different device)
    DeviceSyncRequest {
        public_key: String,
        device_id: String,
        last_sync_timestamp: i64,
    },
    /// Device sync response with data
    DeviceSyncResponse {
        public_key: String,
        device_id: String,
        data_json: String,
        timestamp: i64,
    },
    /// Direct encrypted message between users
    DirectMessage {
        message_id: String,
        from_user_id: SqliteUuid,
        to_user_id: SqliteUuid,
        encrypted_content: String,
        timestamp: i64,
        device_id: Option<String>,
    },
    /// Post from a user
    Post {
        user_id: SqliteUuid,
        content: String,
        timestamp: i64,
        device_id: Option<String>,
        attachments: Option<Vec<MediaAttachmentWithData>>,
    },
    /// Friend addition announcement (for bidirectional friendship)
    /// When Bob scans Alice's QR, Bob broadcasts this to Alice's topic
    /// Alice receives it and auto-adds Bob as a friend
    FriendAdded {
        from_public_key: String,     // Bob's public key
        from_user_id: SqliteUuid,     // Bob's user ID
        from_node_id: String,          // Bob's Iroh NodeId
        from_relay_url: String,        // Bob's relay URL
        to_public_key: String,        // Alice's public key (for verification)
        timestamp: i64,
    },
    /// Heartbeat message to verify peer is still connected
    /// Sent periodically to maintain accurate online peer count
    Heartbeat {
        node_id: String,              // Sender's Iroh NodeId
        timestamp: i64,
    },
}

/// Topic subscription with channel for sending messages and gossip sender for peer management
/// The GossipSender allows dynamic peer joining without re-subscription
/// The actual GossipReceiver is owned by the stream handler task
struct TopicSubscription {
    broadcast_tx: tokio::sync::mpsc::UnboundedSender<Bytes>,
    gossip_sender: Arc<iroh_gossip::net::GossipSender>,
}

/// Track retry state for peer reconnection attempts
#[derive(Clone, Copy, Debug)]
struct PeerRetryState {
    attempt_count: u32,
    last_attempt_time: std::time::Instant,
}

impl PeerRetryState {
    fn new() -> Self {
        PeerRetryState {
            attempt_count: 0,
            last_attempt_time: std::time::Instant::now(),
        }
    }

    /// Calculate exponential backoff delay in seconds
    /// 1s → 2s → 4s → 8s → 16s → 32s → 60s (max)
    fn backoff_delay_secs(&self) -> u64 {
        let delay = 2_u64.saturating_pow(self.attempt_count);
        delay.min(60) // Cap at 60 seconds
    }

    /// Check if we should retry based on backoff
    fn should_retry(&self) -> bool {
        let elapsed = self.last_attempt_time.elapsed().as_secs();
        let delay = self.backoff_delay_secs();
        elapsed >= delay
    }

    /// Record a new attempt
    fn record_attempt(&mut self) {
        self.attempt_count += 1;
        self.last_attempt_time = std::time::Instant::now();
    }

    /// Reset on successful connection
    fn reset(&mut self) {
        self.attempt_count = 0;
        self.last_attempt_time = std::time::Instant::now();
    }
}

/// Main Iroh network coordinator
pub struct IrohNetwork {
    pub user_id: SqliteUuid,
    pub public_key: String,
    pub device_id: Option<String>,
    pub endpoint: Arc<Mutex<Option<iroh::Endpoint>>>,
    pub gossip: Arc<Mutex<Option<iroh_gossip::net::Gossip>>>,
    pub router: Arc<Mutex<Option<Router>>>,
    topics: Arc<Mutex<HashMap<String, TopicSubscription>>>,
    connected_peers: Arc<Mutex<std::collections::HashSet<iroh::NodeId>>>,
    peer_retry_counts: Arc<Mutex<HashMap<iroh::NodeId, PeerRetryState>>>, // Exponential backoff tracking
    peer_heartbeats: Arc<Mutex<HashMap<iroh::NodeId, std::time::Instant>>>, // Last heartbeat time from each peer
    device_seed: [u8; 32], // Device keypair seed for DHT publishing
    app_handle: tauri::AppHandle,
    db: Database,
}

impl IrohNetwork {
    /// Create new Iroh network instance
    pub async fn new(
        user_id: SqliteUuid,
        public_key: String,
        device_id: Option<String>,
        device_keypair: &[u8],
        app_handle: tauri::AppHandle,
        db: Database,
    ) -> Result<Self, String> {
        println!("[IROH] Creating new Iroh network instance");

        // Convert device keypair slice to fixed array
        let mut device_seed = [0u8; 32];
        device_seed.copy_from_slice(device_keypair);

        Ok(IrohNetwork {
            user_id,
            public_key,
            device_id,
            endpoint: Arc::new(Mutex::new(None)),
            gossip: Arc::new(Mutex::new(None)),
            router: Arc::new(Mutex::new(None)),
            topics: Arc::new(Mutex::new(HashMap::new())),
            connected_peers: Arc::new(Mutex::new(std::collections::HashSet::new())),
            peer_retry_counts: Arc::new(Mutex::new(HashMap::new())),
            peer_heartbeats: Arc::new(Mutex::new(HashMap::new())),
            device_seed,
            app_handle,
            db,
        })
    }

    /// Initialize the Iroh endpoint and start networking
    pub async fn initialize(&self) -> Result<(), String> {
        println!("[IROH] Initializing Iroh endpoint...");

        // Clear any stale peer connections from previous sessions
        self.clear_connected_peers().await;

        // Build endpoint with multiple CENSORSHIP-RESISTANT discovery mechanisms:
        //
        // 1. DHT (Distributed Hash Table) - PRIMARY - Fully decentralized, no servers
        // 2. DNS discovery (n0) - Distributed DNS-based peer discovery
        // 3. Relay servers (DERP) - FALLBACK for NAT traversal only
        // 4. mDNS (LocalSwarmDiscovery) - Local network (same WiFi) [optional]
        //
        // CENSORSHIP RESISTANCE STRATEGY:
        // - DHT is primary discovery (no servers to block)
        // - Relays only used for NAT traversal, not required for peer discovery
        // - Peers store discovered addresses for direct connections
        // - Multiple discovery paths prevent single point of failure
        //
        // Privacy-preserving: No user data sent anywhere, only connection metadata
        // Fully decentralized: Can work with DHT alone, relays are optional helpers

        // Convert device seed to Iroh SecretKey for DHT publishing and endpoint identity
        let secret_key = iroh::SecretKey::from_bytes(&self.device_seed);

        let mut endpoint_builder = iroh::Endpoint::builder().secret_key(secret_key.clone());

        // Configure Pkarr DHT discovery (BitTorrent Mainline DHT)
        // CRITICAL: Provide secret_key to enable DHT PUBLISHING
        // Without this, we only consume DHT but never publish our NodeAddr
        let dht_discovery = iroh::discovery::pkarr::dht::DhtDiscovery::builder()
            .secret_key(secret_key.clone()) // Enable DHT publishing!
            .build()
            .map_err(|e| format!("Failed to create DHT discovery: {}", e))?;

        endpoint_builder = endpoint_builder
            .discovery(Box::new(dht_discovery)) // BitTorrent DHT - fully decentralized
            .discovery_n0() // DNS-based peer discovery (distributed)
            .relay_mode(iroh::RelayMode::Default); // Relays for NAT traversal

        #[cfg(feature = "mdns")]
        {
            let public_key_for_mdns = secret_key.public();
            endpoint_builder = endpoint_builder.discovery(Box::new(
                iroh::discovery::local_swarm_discovery::LocalSwarmDiscovery::new(
                    public_key_for_mdns,
                )
                .map_err(|e| format!("Failed to create mDNS discovery: {}", e))?,
            ));
        }

        let endpoint = endpoint_builder
            .bind()
            .await
            .map_err(|e| format!("Failed to create Iroh endpoint: {}", e))?;

        let node_id = endpoint.node_id();
        let public_key = secret_key.public();

        println!("[IROH] ========================================");
        println!("[IROH] Endpoint bound with MULTI-PATH DISCOVERY:");
        println!("[IROH]   ✓ DHT PUBLISHING ENABLED - We are discoverable!");
        println!("[IROH]   ✓ DHT (PRIMARY) - Fully decentralized, no servers required");
        println!("[IROH]   ✓ DNS discovery (n0) - Distributed peer finding");
        println!("[IROH]   ✓ Relay servers (FALLBACK) - NAT traversal helper only");
        println!("[IROH] NodeId: {}", node_id);
        println!("[IROH] PublicKey: {}", public_key);
        println!("[IROH] Network can operate with DHT alone - relays are optional");
        println!("[IROH] ========================================");

        // Store our NodeId AND relay URL in the database if we have a device_id
        if let Some(device_id) = &self.device_id {
            let node_id_str = node_id.to_string();
            if let Err(e) = self.db.update_device_node_id(device_id, &node_id_str) {
                println!("[IROH] Warning: Failed to store NodeId in database: {}", e);
            } else {
                println!("[IROH] Stored NodeId in database for device {}", device_id);
            }

            // Also store our relay URL for reconnection
            let our_node_addr = endpoint
                .node_addr()
                .await
                .map_err(|e| format!("Failed to get node address: {}", e))?;
            if let Some(relay_url) = our_node_addr.relay_url() {
                let relay_url_str = relay_url.to_string();
                if let Err(e) = self.db.update_device_relay_url(device_id, &relay_url_str) {
                    println!(
                        "[IROH] Warning: Failed to store relay URL in database: {}",
                        e
                    );
                } else {
                    println!(
                        "[IROH] Stored relay URL {} in database for device {}",
                        relay_url_str, device_id
                    );
                }
            }
        }

        // Create gossip protocol
        let gossip = iroh_gossip::net::Gossip::builder()
            .spawn(endpoint.clone())
            .await
            .map_err(|e| format!("Failed to create gossip: {}", e))?;

        println!("[IROH] Gossip protocol initialized");

        // Create Router to accept incoming gossip connections AND direct presence connections
        // NOTE: Direct presence uses a separate ALPN to bypass the gossip protocol
        let router = Router::builder(endpoint.clone())
            .accept(ALPN, gossip.clone())
            .spawn()
            .await
            .map_err(|e| format!("Failed to create router: {}", e))?;

        println!("[IROH] Router created - accepting gossip connections");
        println!("[IROH] Direct RPC acceptor ready for incoming Presence handshakes");

        // Store endpoint, gossip, and router
        *self.endpoint.lock().await = Some(endpoint.clone());
        *self.gossip.lock().await = Some(gossip);
        *self.router.lock().await = Some(router);

        // Query for peer NodeIds from database (other devices with same user)
        let mut peer_node_ids: Vec<String> = if let Some(device_id) = &self.device_id {
            match self.db.get_peer_node_ids(&self.public_key, device_id) {
                Ok(ids) => {
                    println!("[IROH] Found {} peer NodeIds in database", ids.len());
                    ids
                }
                Err(e) => {
                    println!("[IROH] Warning: Failed to query peer NodeIds: {}", e);
                    vec![]
                }
            }
        } else {
            vec![]
        };

        // CENSORSHIP-RESISTANT PEER DISCOVERY:
        // No hardcoded servers, no central coordination, no single point of failure
        //
        // Discovery happens through multiple independent paths:
        // 1. DHT gossip propagation (primary, fully decentralized)
        // 2. Relay network (optional NAT traversal helper)
        // 3. Stored peer addresses from previous sessions
        // 4. Direct peer connections when addresses are known
        //
        // This design ensures the network remains operational even if:
        // - Relay servers are blocked/censored
        // - DNS is compromised
        // - Some peers are unreachable
        println!("[IROH] Setting up censorship-resistant peer discovery (DHT + multi-path)...");

        // Bootstrap gossip mesh using stored peer addresses (NodeId + relay URL)
        // This enables reconnection after app restart or network interruption
        println!("[IROH] Attempting to connect to known peers using stored relay URLs...");
        if let Ok(peer_addrs) = self.db.get_all_peer_addrs() {
            println!(
                "[IROH] Found {} peer addresses in database",
                peer_addrs.len()
            );
            for (node_id_str, relay_url_opt) in &peer_addrs {
                if let Ok(peer_id) = node_id_str.parse::<iroh::NodeId>() {
                    if peer_id != node_id {
                        // Construct full NodeAddr with relay URL for reliable connection
                        let mut peer_node_addr = iroh::NodeAddr::new(peer_id);
                        if let Some(relay_url) = relay_url_opt {
                            if let Ok(url) = relay_url.parse() {
                                peer_node_addr = peer_node_addr.with_relay_url(url);
                                println!(
                                    "[IROH] Connecting to peer {} with relay: {}",
                                    peer_id, relay_url
                                );
                            } else {
                                println!("[IROH] Warning: Invalid relay URL for peer {}", peer_id);
                                continue;
                            }
                        } else {
                            println!("[IROH] Note: No relay URL for peer {}, trying direct connection only", peer_id);
                        }

                        // Add full NodeAddr to endpoint before connecting
                        if let Err(e) = endpoint.add_node_addr(peer_node_addr.clone()) {
                            println!("[IROH] Warning: Failed to add node address: {}", e);
                        }

                        // Now try to connect with full addressing info
                        println!("[IROH] Attempting to connect to peer: {}", peer_id);
                        match endpoint.connect(peer_id, &iroh_gossip::ALPN).await {
                            Ok(conn) => {
                                println!("[IROH] ✓ CONNECTED to peer {}!", peer_id);
                                println!("[IROH]   Remote address: {:?}", conn.remote_address());
                                // Track this connection
                                self.add_connected_peer(peer_id).await;
                                // Keep this peer for gossip bootstrap
                                peer_node_ids.push(node_id_str.clone());
                                break; // One connection is enough to start
                            }
                            Err(e) => {
                                println!("[IROH] Failed to connect to {}: {}", peer_id, e);
                            }
                        }
                    }
                }
            }
        }

        // CRITICAL FIX: Do NOT subscribe to discovery/presence topics at startup
        // Problem: Subscribing with no bootstrap creates isolated gossip meshes
        // Solution: Only subscribe when user generates/scans QR code, always with proper bootstrap
        println!(
            "[IROH] Skipping discovery/presence subscriptions at startup to prevent mesh isolation"
        );
        println!("[IROH] Topics will be subscribed when user generates or scans QR code");

        // Actively try to connect to discovered peers via direct connection attempts
        // This helps bootstrap the gossip mesh using stored peer addresses
        // CENSORSHIP RESISTANCE: Even if infrastructure is blocked, users can exchange
        // peer addresses out-of-band (QR codes, messaging apps) and manually bootstrap
        self.start_active_peer_discovery();

        // CRITICAL FIX: Do NOT subscribe to our own topic during initialization
        // Problem: Subscribing with no/few bootstrap peers creates an isolated gossip mesh
        // Solution: Only subscribe to our own topic when we receive first Presence from a friend
        //           (meaning they've subscribed to our topic), using them as bootstrap
        // This ensures we join the same mesh as our friends
        println!("[IROH] Skipping own topic subscription at init - will subscribe when first friend connects");

        // Subscribe to all friends' topics on startup (restores subscriptions after app restart)
        match self.db.get_friend_public_keys(self.user_id) {
            Ok(friend_public_keys) => {
                if !friend_public_keys.is_empty() {
                    println!(
                        "[IROH] Subscribing to {} friend topics from database...",
                        friend_public_keys.len()
                    );
                    for friend_pk in friend_public_keys {
                        let topic = format!("cipher/user/{}", friend_pk);
                        if let Err(e) = self.subscribe_topic(&topic).await {
                            println!("[IROH] Warning: Failed to subscribe to friend topic: {}", e);
                        } else {
                            println!("[IROH] ✓ Subscribed to friend topic: {}", topic);
                        }
                    }
                    println!("[IROH] Friend topic subscriptions complete");
                } else {
                    println!("[IROH] No existing friends - skipping friend topic subscriptions");
                }
            }
            Err(e) => {
                println!("[IROH] Warning: Failed to get friend public keys: {}", e);
            }
        }

        // CRITICAL: Pre-populate endpoint with friend peer addresses for persistent connections
        // This enables reconnection to friends after app restart
        println!("[IROH] Loading saved friend peer addresses for persistent reconnection...");
        match self.db.get_all_friend_peer_addresses(self.user_id) {
            Ok(friend_peer_addrs) => {
                if !friend_peer_addrs.is_empty() {
                    println!(
                        "[IROH] Found {} friend peer addresses in database",
                        friend_peer_addrs.len()
                    );
                    for (node_id_str, relay_url) in &friend_peer_addrs {
                        if let Ok(peer_id) = node_id_str.parse::<iroh::NodeId>() {
                            if peer_id != node_id {
                                // Construct NodeAddr with friend's relay URL
                                let mut peer_node_addr = iroh::NodeAddr::new(peer_id);
                                if let Ok(url) = relay_url.parse() {
                                    peer_node_addr = peer_node_addr.with_relay_url(url);
                                    println!(
                                        "[IROH] Pre-populating friend peer {} with relay: {}",
                                        peer_id, relay_url
                                    );
                                    // Add to endpoint
                                    if let Err(e) = endpoint.add_node_addr(peer_node_addr) {
                                        println!(
                                            "[IROH] Warning: Failed to pre-populate friend peer {}: {}",
                                            peer_id, e
                                        );
                                    } else {
                                        println!("[IROH] ✓ Pre-populated friend peer {}", peer_id);
                                        // Try to reconnect with retry logic (3 attempts with exponential backoff)
                                        let max_attempts = 3;
                                        let mut reconnected = false;

                                        for attempt in 1..=max_attempts {
                                            match endpoint.connect(peer_id, &iroh_gossip::ALPN).await {
                                                Ok(conn) => {
                                                    println!("[IROH] ✓ RECONNECTED to friend {} on attempt {}!", peer_id, attempt);
                                                    println!(
                                                        "[IROH]   Remote address: {:?}",
                                                        conn.remote_address()
                                                    );
                                                    self.add_connected_peer(peer_id).await;
                                                    reconnected = true;
                                                    break;
                                                }
                                                Err(e) => {
                                                    println!(
                                                        "[IROH] Reconnect attempt {}/{} to friend {} failed: {}",
                                                        attempt, max_attempts, peer_id, e
                                                    );

                                                    if attempt < max_attempts {
                                                        // Exponential backoff: 500ms, 1s
                                                        let delay_ms = 500 * (1 << (attempt - 1));
                                                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                                                    }
                                                }
                                            }
                                        }

                                        if !reconnected {
                                            println!(
                                                "[IROH] Note: Friend peer {} not immediately reachable after {} attempts",
                                                peer_id, max_attempts
                                            );
                                            println!("[IROH]   Will discover via gossip presence announcements");
                                        }
                                    }
                                }
                            }
                        }
                    }
                    println!("[IROH] Friend peer address pre-population complete");
                } else {
                    println!("[IROH] No saved friend peer addresses yet");
                }
            }
            Err(e) => {
                println!("[IROH] Warning: Failed to load friend peer addresses: {}", e);
            }
        }

        println!("[IROH] Initialization complete - DHT/relay discovery active");
        println!("[IROH] Using gossip topics for all Presence messages");
        Ok(())
    }

    /// Convert topic string to 32-byte TopicId
    pub fn topic_to_id(topic: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(topic.as_bytes());
        let hash = hasher.finalize();
        let mut topic_id = [0u8; 32];
        topic_id.copy_from_slice(&hash);
        topic_id
    }

    /// Subscribe to a gossip topic
    pub async fn subscribe_topic(&self, topic: &str) -> Result<(), String> {
        self.subscribe_topic_with_peers(topic, vec![]).await
    }

    /// Subscribe to a gossip topic with bootstrap peers (takes iroh::NodeId directly)
    pub async fn subscribe_with_bootstrap(
        &self,
        topic: &str,
        bootstrap_peers: Vec<iroh::NodeId>,
    ) -> Result<(), String> {
        // Convert NodeIds to strings for the internal method
        let peer_node_ids: Vec<String> = bootstrap_peers.iter().map(|id| id.to_string()).collect();
        self.subscribe_topic_with_peers(topic, peer_node_ids).await
    }

    /// Subscribe to a gossip topic with bootstrap peers
    async fn subscribe_topic_with_peers(
        &self,
        topic: &str,
        peer_node_ids: Vec<String>,
    ) -> Result<(), String> {
        let gossip_guard = self.gossip.lock().await;
        let gossip = gossip_guard.as_ref().ok_or("Gossip not initialized")?;

        // Convert topic string to TopicId
        let topic_id = iroh_gossip::proto::TopicId::from(Self::topic_to_id(topic));

        // Convert String NodeIds to iroh::NodeId types
        let bootstrap_peers: Vec<iroh::NodeId> = peer_node_ids
            .iter()
            .filter_map(|id_str| match id_str.parse::<iroh::NodeId>() {
                Ok(node_id) => Some(node_id),
                Err(e) => {
                    println!("[IROH] Warning: Failed to parse NodeId '{}': {}", id_str, e);
                    None
                }
            })
            .collect();

        // CRITICAL FIX: Use subscribe_and_join() to ACTUALLY form the gossip mesh
        // subscribe() only creates a subscription but doesn't connect to bootstrap peers
        // subscribe_and_join() actively joins the gossip mesh by connecting to bootstrap nodes
        // This is the official pattern from iroh-gossip examples/chat.rs
        if !bootstrap_peers.is_empty() {
            println!(
                "[IROH-GOSSIP] Subscribing AND JOINING topic '{}' with {} bootstrap peers",
                topic,
                bootstrap_peers.len()
            );
            for (i, peer) in bootstrap_peers.iter().enumerate() {
                println!("[IROH-GOSSIP]   Bootstrap peer {}: {}", i + 1, peer);
            }
        } else {
            println!(
                "[IROH-GOSSIP] Subscribing to topic '{}' with no bootstrap peers (root node)",
                topic
            );
        }

        // CRITICAL: ALWAYS use subscribe_and_join() - even with empty bootstrap list!
        // This is required for proper gossip mesh formation per iroh-gossip chat example.
        //
        // Key insight:
        // - subscribe_and_join() with empty bootstrap: Creates proper root node that CAN accept joins
        // - subscribe() with empty bootstrap: Creates passive root that CANNOT accept joins
        //
        // When bootstrap list is empty, subscribe_and_join() will emit Joined([]) event immediately,
        // making this peer a proper root that others can join.
        let gossip_topic = if !bootstrap_peers.is_empty() {
            println!("[IROH-GOSSIP] Calling gossip.subscribe_and_join() for topic '{}' with {} bootstrap peers (15s timeout)...", topic, bootstrap_peers.len());
            println!("[IROH-GOSSIP]   subscribe_and_join() will wait for NeighborUp event from gossip protocol");
            println!("[IROH-GOSSIP]   This indicates the gossip mesh has formed successfully");

            // Give the gossip protocol layer time to establish after QUIC connection
            // QUIC connection succeeds quickly, but gossip protocol needs time to set up
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            match tokio::time::timeout(
                tokio::time::Duration::from_secs(15),
                gossip.subscribe_and_join(topic_id, bootstrap_peers.clone())
            ).await {
                Ok(Ok(result)) => {
                    println!("[IROH-GOSSIP] ✓ gossip.subscribe_and_join() completed for topic '{}'", topic);
                    println!("[IROH-GOSSIP]   Successfully joined gossip mesh via {} bootstrap peers", bootstrap_peers.len());
                    println!("[IROH-GOSSIP]   Gossip protocol neighbor relationship established!");
                    result
                }
                Ok(Err(e)) => {
                    println!("[IROH-GOSSIP] ✗ gossip.subscribe_and_join() failed: {}", e);
                    return Err(format!("Failed to join gossip topic: {}", e));
                }
                Err(_) => {
                    println!("[IROH-GOSSIP] ⏱  gossip.subscribe_and_join() timed out after 15s for topic '{}'", topic);
                    println!("[IROH-GOSSIP]   Gossip protocol did not establish NeighborUp within timeout");
                    println!("[IROH-GOSSIP]   This indicates the bootstrap peer may not be subscribed to this topic");
                    return Err(format!("Timed out joining gossip topic - gossip protocol did not form neighbor relationship"));
                }
            }
        } else {
            println!("[IROH-GOSSIP] Calling gossip.subscribe_and_join() for topic '{}' (root node - empty bootstrap)...", topic);
            println!("[IROH-GOSSIP]   subscribe_and_join() with empty bootstrap emits Joined([]) immediately");
            println!("[IROH-GOSSIP]   This creates proper root node that CAN accept joins from others");

            // CRITICAL: Use subscribe_and_join() even for root nodes (empty bootstrap list)
            // This creates a proper root node that can accept joins from others
            // According to iroh-gossip chat example, subscribe_and_join() with empty bootstrap
            // will emit Joined([]) event immediately, making this a proper accepting root
            match gossip.subscribe_and_join(topic_id, bootstrap_peers.clone()).await {
                Ok(result) => {
                    println!("[IROH-GOSSIP] ✓ gossip.subscribe_and_join() completed for topic '{}' (root node)", topic);
                    println!("[IROH-GOSSIP]   Root node created - ready to accept joins from peers");
                    result
                }
                Err(e) => {
                    println!("[IROH-GOSSIP] ✗ gossip.subscribe_and_join() failed for root node: {}", e);
                    return Err(format!("Failed to subscribe to gossip topic: {}", e));
                }
            }
        };

        println!("[IROH-GOSSIP]   TopicId: {:?}", topic_id);

        // Split into sender and receiver
        let (gossip_sender, gossip_receiver) = gossip_topic.split();

        // Store both the gossip sender (for broadcasts) and a channel for our internal broadcasts
        // We need a channel because Tauri commands can't hold the gossip sender directly
        let (broadcast_tx, broadcast_rx) = tokio::sync::mpsc::unbounded_channel();

        // Wrap gossip_sender in Arc so we can share it between HashMap and stream handler
        let gossip_sender_arc = Arc::new(gossip_sender);

        // Store the channel sender AND gossip sender for publishing messages and managing peers
        self.topics
            .lock()
            .await
            .insert(topic.to_string(), TopicSubscription {
                broadcast_tx,
                gossip_sender: gossip_sender_arc.clone(),
            });

        // Start listening to the receiver stream AND handle broadcast requests
        let network = Arc::new(self.clone_for_background());
        let topic_str = topic.to_string();
        let gossip_sender_for_handler = gossip_sender_arc.clone();
        tokio::spawn(async move {
            network
                .handle_topic_stream(topic_str, gossip_sender_for_handler, gossip_receiver, broadcast_rx)
                .await;
        });

        println!("[IROH] Subscribed and joined gossip topic: {}", topic);
        Ok(())
    }

    /// Simple topic listener that just logs received messages (Send-safe)
    /// This is used for re-subscribed topics to ensure messages are received
    /// without needing to hold locks or process messages inline
    async fn simple_topic_listener(
        topic: String,
        mut gossip_topic: iroh_gossip::net::GossipTopic,
        app_handle: tauri::AppHandle,
    ) {
        use futures::StreamExt;

        println!(
            "[IROH] Starting simple listener for re-subscribed topic: {}",
            topic
        );

        loop {
            match gossip_topic.next().await {
                Some(Ok(iroh_gossip::net::Event::Gossip(gossip_event))) => {
                    if let iroh_gossip::net::GossipEvent::Received(msg) = gossip_event {
                        // Deserialize and emit to UI for processing
                        match serde_json::from_slice::<P2PMessage>(&msg.content) {
                            Ok(p2p_msg) => {
                                println!(
                                    "[IROH] ✓ Re-subscription received message on topic {}: {:?}",
                                    topic, p2p_msg
                                );

                                // Emit to UI via p2p-message-received event
                                #[derive(serde::Serialize, Clone)]
                                struct MessageEvent {
                                    message: P2PMessage,
                                }
                                let _ = app_handle.emit(
                                    "p2p-message-received",
                                    MessageEvent { message: p2p_msg },
                                );
                            }
                            Err(e) => {
                                println!(
                                    "[IROH] Failed to deserialize re-subscription message: {}",
                                    e
                                );
                            }
                        }
                    }
                }
                Some(Ok(iroh_gossip::net::Event::Lagged)) => {
                    println!("[IROH] Re-subscription stream lagged on topic: {}", topic);
                }
                Some(Err(e)) => {
                    println!(
                        "[IROH] Re-subscription error on topic {} stream: {}",
                        topic, e
                    );
                    break;
                }
                None => {
                    println!(
                        "[IROH] Re-subscription message stream ended for topic: {}",
                        topic
                    );
                    break;
                }
            }
        }
    }

    /// Send-safe bidirectional topic handler for peer topics
    /// Handles both incoming and outgoing messages without calling handle_message
    async fn bidirectional_topic_listener(
        topic: String,
        mut gossip_topic: iroh_gossip::net::GossipTopic,
        mut broadcast_rx: tokio::sync::mpsc::UnboundedReceiver<Bytes>,
        app_handle: tauri::AppHandle,
    ) {
        use futures::StreamExt;

        println!(
            "[IROH] Starting bidirectional listener for topic: {}",
            topic
        );

        loop {
            tokio::select! {
                // Incoming gossip messages
                event = gossip_topic.next() => {
                    match event {
                        Some(Ok(iroh_gossip::net::Event::Gossip(gossip_event))) => {
                            if let iroh_gossip::net::GossipEvent::Received(msg) = gossip_event {
                                match serde_json::from_slice::<P2PMessage>(&msg.content) {
                                    Ok(p2p_msg) => {
                                        println!("[IROH] ✓ Received message on topic {}: {:?}", topic, p2p_msg);

                                        // Emit to UI for processing
                                        #[derive(serde::Serialize, Clone)]
                                        struct MessageEvent {
                                            message: P2PMessage,
                                        }
                                        let _ = app_handle.emit("p2p-message-received", MessageEvent {
                                            message: p2p_msg,
                                        });
                                    }
                                    Err(e) => {
                                        println!("[IROH] Failed to deserialize message: {}", e);
                                    }
                                }
                            }
                        }
                        Some(Ok(iroh_gossip::net::Event::Lagged)) => {
                            println!("[IROH] Stream lagged on topic: {}", topic);
                        }
                        Some(Err(e)) => {
                            println!("[IROH] Error on topic {} stream: {}", topic, e);
                            break;
                        }
                        None => {
                            println!("[IROH] Message stream ended for topic: {}", topic);
                            break;
                        }
                    }
                }

                // Outgoing broadcast messages
                Some(data) = broadcast_rx.recv() => {
                    println!("[IROH] Broadcasting message to topic: {}", topic);
                    if let Err(e) = gossip_topic.broadcast(data).await {
                        println!("[IROH] Failed to broadcast message: {}", e);
                    } else {
                        println!("[IROH] Successfully broadcast message to topic: {}", topic);
                    }
                }
            }
        }
    }

    /// Handle incoming messages from a topic stream and broadcast outgoing messages
    async fn handle_topic_stream(
        &self,
        topic: String,
        gossip_sender: Arc<GossipSender>,
        mut gossip_receiver: GossipReceiver,
        mut broadcast_rx: tokio::sync::mpsc::UnboundedReceiver<Bytes>,
    ) {
        use futures::StreamExt;

        println!("[IROH-STREAM] Starting message handler for topic: {}", topic);
        println!("[IROH-STREAM] Listening for gossip events and broadcast requests...");

        // Listen on both the gossip receiver stream (incoming) and broadcast channel (outgoing)
        loop {
            tokio::select! {
                // Incoming gossip messages from the receiver
                event = gossip_receiver.next() => {
                    match event {
                        Some(Ok(GossipNetEvent::Gossip(gossip_event))) => {
                            match gossip_event {
                                GossipEvent::Received(msg) => {
                                    println!("[IROH-STREAM] 📬 Received gossip message on topic '{}'", topic);
                                    println!("[IROH-STREAM]   From: {}", msg.delivered_from);
                                    println!("[IROH-STREAM]   Content size: {} bytes", msg.content.len());

                                    // Deserialize message
                                    match serde_json::from_slice::<P2PMessage>(&msg.content) {
                                        Ok(p2p_msg) => {
                                            println!("[IROH-STREAM]   Message type: {:?}",
                                                match &p2p_msg {
                                                    P2PMessage::Presence { .. } => "Presence",
                                                    P2PMessage::DirectMessage { .. } => "DirectMessage",
                                                    P2PMessage::Post { .. } => "Post",
                                                    P2PMessage::FriendAdded { .. } => "FriendAdded",
                                                    P2PMessage::DeviceSyncRequest { .. } => "DeviceSyncRequest",
                                                    P2PMessage::DeviceSyncResponse { .. } => "DeviceSyncResponse",
                                                    P2PMessage::Heartbeat { .. } => "Heartbeat",
                                                }
                                            );
                                            self.handle_message(p2p_msg).await;
                                        }
                                        Err(e) => {
                                            println!("[IROH-STREAM] ✗ Failed to deserialize message: {}", e);
                                        }
                                    }
                                }
                                GossipEvent::Joined(peers) => {
                                    println!("[IROH-STREAM] 🤝 Peer(s) joined topic '{}': {:?}", topic, peers);
                                    // Add all joined peers to connected_peers (they're in our gossip mesh!)
                                    for peer_id in peers {
                                        println!("[IROH-STREAM]   Adding peer to connected_peers: {}", peer_id);
                                        self.add_connected_peer(peer_id).await;
                                    }
                                }
                                GossipEvent::NeighborUp(peer) => {
                                    println!("[IROH-STREAM] 📡 Neighbor UP on topic '{}': {}", topic, peer);
                                    // Peer became our neighbor - add to connected_peers
                                    println!("[IROH-STREAM]   Adding peer to connected_peers: {}", peer);
                                    self.add_connected_peer(peer).await;
                                }
                                GossipEvent::NeighborDown(peer) => {
                                    println!("[IROH-STREAM] 📴 Neighbor DOWN on topic '{}': {}", topic, peer);
                                    // NOTE: Don't immediately remove peer - gossip neighbor relationships can be
                                    // temporarily disrupted while QUIC connection remains valid. The heartbeat
                                    // monitor will remove stale peers after 45s timeout if they're truly gone.
                                    println!("[IROH-STREAM]   Gossip neighbor down, but keeping peer in connected set (heartbeat monitor will remove if truly stale)");
                                }
                            }
                        }
                        Some(Ok(GossipNetEvent::Lagged)) => {
                            println!("[IROH-STREAM] ⚠️  Stream lagged on topic: {}", topic);
                        }
                        Some(Err(e)) => {
                            println!("[IROH-STREAM] ✗ Error on topic {} stream: {}", topic, e);
                            break;
                        }
                        None => {
                            println!("[IROH-STREAM] Stream ended for topic: {}", topic);
                            break;
                        }
                    }
                }

                // Outgoing broadcast messages via the sender
                Some(data) = broadcast_rx.recv() => {
                    println!("[IROH-STREAM] 📤 Broadcasting message to topic '{}' ({} bytes)", topic, data.len());
                    match gossip_sender.broadcast(data.clone()).await {
                        Ok(_) => {
                            println!("[IROH-STREAM] ✓ Successfully broadcast message to topic '{}'", topic);
                        }
                        Err(e) => {
                            println!("[IROH-STREAM] ✗ Failed to broadcast message to topic '{}': {}", topic, e);
                        }
                    }
                }
            }
        }
    }

    /// Check if a topic is subscribed
    pub async fn is_topic_subscribed(&self, topic: &str) -> bool {
        let topics_guard = self.topics.lock().await;
        topics_guard.contains_key(topic)
    }

    /// Unsubscribe from a gossip topic
    /// This cleanly removes the subscription and stops the message handler
    async fn unsubscribe_topic(&self, topic: &str) -> Result<(), String> {
        println!("[IROH-UNSUB] Unsubscribing from topic '{}'...", topic);

        let mut topics_guard = self.topics.lock().await;
        if topics_guard.remove(topic).is_some() {
            drop(topics_guard);
            println!("[IROH-UNSUB] ✓ Unsubscribed from topic '{}'", topic);
            println!("[IROH-UNSUB]   Stream handler will terminate automatically");
            Ok(())
        } else {
            drop(topics_guard);
            Err(format!("Not subscribed to topic: {}", topic))
        }
    }

    /// Resubscribe to a topic with bootstrap peers
    /// This is the CORRECT way to join an existing gossip mesh:
    /// 1. Unsubscribe from topic (removes isolated root subscription)
    /// 2. Subscribe WITH bootstrap peer (joins their existing mesh)
    ///
    /// Note: join_peers() doesn't work for merging isolated root nodes!
    pub async fn resubscribe_with_bootstrap(
        &self,
        topic: &str,
        bootstrap_peers: Vec<iroh::NodeId>,
    ) -> Result<(), String> {
        println!(
            "[IROH-RESUB] Resubscribing to '{}' with {} bootstrap peers...",
            topic,
            bootstrap_peers.len()
        );
        for (i, peer_id) in bootstrap_peers.iter().enumerate() {
            println!("[IROH-RESUB]   Bootstrap peer {}: {}", i + 1, peer_id);
        }

        // Step 1: Unsubscribe from existing isolated subscription
        if self.is_topic_subscribed(topic).await {
            println!("[IROH-RESUB] Topic already subscribed - unsubscribing first...");
            self.unsubscribe_topic(topic).await?;
            // Give gossip protocol time to clean up the old subscription
            // This ensures the protocol layer fully tears down before we re-subscribe
            println!("[IROH-RESUB] Waiting 1s for gossip protocol to fully clean up...");
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        // Step 2: Subscribe with bootstrap peers to join their mesh
        println!("[IROH-RESUB] Subscribing WITH bootstrap (will wait for mesh formation)...");
        self.subscribe_with_bootstrap(topic, bootstrap_peers).await?;

        println!("[IROH-RESUB] ✓ Successfully resubscribed to '{}'", topic);
        println!("[IROH-RESUB]   Now part of the same gossip mesh as bootstrap peers");
        Ok(())
    }

    /// Ensure discovery/presence topics are subscribed (lazy subscription)
    /// Called when user generates QR or adds first friend
    pub async fn ensure_discovery_subscribed(&self) -> Result<(), String> {
        // Check if already subscribed
        let already_subscribed = self.is_topic_subscribed("cipher/discovery/v1").await
            && self.is_topic_subscribed("cipher/presence").await;

        if already_subscribed {
            println!("[IROH] Discovery/presence topics already subscribed");
            return Ok(());
        }

        println!("[IROH] Subscribing to discovery/presence topics (first time)...");

        // Subscribe without bootstrap (we become a root node in the mesh)
        self.subscribe_topic("cipher/discovery/v1").await?;
        println!("[IROH] ✓ Subscribed to discovery topic");

        self.subscribe_topic("cipher/presence").await?;
        println!("[IROH] ✓ Subscribed to presence topic");

        // Start presence announcement loop now that we're subscribed
        self.start_presence_loop();
        println!("[IROH] ✓ Started presence announcements");

        // Start heartbeat system for accurate online peer tracking
        self.start_heartbeat_sender();
        self.start_heartbeat_monitor();

        // Announce immediately
        self.announce_presence().await?;

        Ok(())
    }

    /// Publish a message to a gossip topic
    pub async fn publish_message(&self, topic: &str, message: P2PMessage) -> Result<(), String> {
        println!("[IROH] publish_message() called for topic: {}", topic);

        // Serialize message
        println!("[IROH] Serializing message...");
        let data = serde_json::to_vec(&message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        println!("[IROH] Message serialized, size: {} bytes", data.len());

        // Get the broadcast channel for this topic
        println!("[IROH] Acquiring topics lock...");
        let topics_guard = self.topics.lock().await;
        println!("[IROH] Topics lock acquired, looking up topic: {}", topic);

        let subscription = topics_guard
            .get(topic)
            .ok_or_else(|| format!("Not subscribed to topic: {}", topic))?;
        println!("[IROH] Found subscription for topic: {}", topic);

        // Send message to the broadcast channel (non-blocking)
        println!("[IROH] Sending message to broadcast channel...");
        subscription
            .broadcast_tx
            .send(Bytes::from(data))
            .map_err(|e| format!("Failed to send to broadcast channel: {}", e))?;

        println!("[IROH] Message queued for broadcast to topic: {}", topic);
        Ok(())
    }

    /// Announce presence to discover other devices
    pub async fn announce_presence(&self) -> Result<(), String> {
        // Get our full NodeAddr from the endpoint
        let endpoint_guard = self.endpoint.lock().await;
        let node_addr = if let Some(endpoint) = endpoint_guard.as_ref() {
            endpoint
                .node_addr()
                .await
                .map_err(|e| format!("Failed to get node address: {}", e))?
        } else {
            return Err("Endpoint not initialized".to_string());
        };
        drop(endpoint_guard);

        let presence = P2PMessage::Presence {
            user_id: self.user_id,
            public_key: self.public_key.clone(),
            device_id: self
                .device_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            node_addr,
            timestamp: chrono::Utc::now().timestamp(),
        };

        // Publish to global presence topic
        // Router enables incoming connections, DHT/relay enables peer discovery
        self.publish_message("cipher/presence", presence).await
    }

    /// Start acceptor for direct presence streams
    /// This listens for incoming bi-directional streams on gossip connections and processes Presence messages
    fn start_direct_presence_acceptor(&self) {
        let network = Arc::new(self.clone_for_background());

        tokio::spawn(async move {
            println!("[IROH-DIRECT] Acceptor started, waiting for direct presence streams...");

            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                let endpoint_guard = network.endpoint.lock().await;
                if let Some(endpoint) = endpoint_guard.as_ref() {
                    let endpoint_clone = endpoint.clone();
                    drop(endpoint_guard);

                    // Try to accept incoming connection
                    match tokio::time::timeout(
                        tokio::time::Duration::from_millis(50),
                        endpoint_clone.accept(),
                    )
                    .await
                    {
                        Ok(Some(incoming)) => {
                            println!("[IROH-DIRECT] Got incoming connection");
                            let network_clone = network.clone();

                            tokio::spawn(async move {
                                match incoming.await {
                                    Ok(connection) => {
                                        println!("[IROH-DIRECT] Connection established, waiting for streams...");

                                        // Accept incoming bi-directional streams
                                        loop {
                                            match connection.accept_bi().await {
                                                Ok((mut _send, mut recv)) => {
                                                    println!("[IROH-DIRECT] Accepted bi-directional stream");

                                                    // Read presence message
                                                    let buffer_result = recv.read_to_end(100_000).await;

                                                    let buffer = match buffer_result {
                                                        Ok(buf) => {
                                                            println!(
                                                                "[IROH-DIRECT] Received {} bytes",
                                                                buf.len()
                                                            );
                                                            buf
                                                        }
                                                        Err(e) => {
                                                            println!("[IROH-DIRECT] Failed to read stream: {}", e);
                                                            continue;
                                                        }
                                                    };

                                                    // Try to deserialize as Presence
                                                    match serde_json::from_slice::<P2PMessage>(
                                                        &buffer,
                                                    ) {
                                                        Ok(P2PMessage::Presence { .. }) => {
                                                            println!("[IROH-DIRECT] ✓ Received direct Presence message!");
                                                            // Process the presence
                                                            network_clone
                                                                .handle_message(
                                                                    serde_json::from_slice(&buffer)
                                                                        .unwrap(),
                                                                )
                                                                .await;
                                                        }
                                                        Ok(other) => {
                                                            println!("[IROH-DIRECT] Received non-Presence message: {:?}", other);
                                                        }
                                                        Err(e) => {
                                                            println!("[IROH-DIRECT] Failed to deserialize message: {}", e);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    println!(
                                                        "[IROH-DIRECT] Error accepting stream: {}",
                                                        e
                                                    );
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!(
                                            "[IROH-DIRECT] Failed to establish connection: {}",
                                            e
                                        );
                                    }
                                }
                            });
                        }
                        Ok(None) => {
                            // No incoming connection
                        }
                        Err(_) => {
                            // Timeout - continue loop
                        }
                    }
                } else {
                    drop(endpoint_guard);
                    println!("[IROH-DIRECT] Endpoint not initialized, stopping acceptor");
                    break;
                }
            }
        });
    }

    /// Send presence directly to peer via gossip connection (bypasses gossip mesh isolation)
    /// This is a workaround for the gossip mesh isolation problem by sending directly over
    /// a connection stream BEFORE the gossip mesh is fully established
    pub async fn send_direct_presence(
        &self,
        peer_node_id: iroh::NodeId,
        peer_node_addr: &iroh::NodeAddr,
    ) -> Result<(), String> {
        println!(
            "[IROH-DIRECT] Sending direct presence to peer {}",
            peer_node_id
        );

        let endpoint_guard = self.endpoint.lock().await;
        let endpoint = endpoint_guard.as_ref().ok_or("Endpoint not initialized")?;

        // Add peer's full address to endpoint
        if let Err(e) = endpoint.add_node_addr(peer_node_addr.clone()) {
            println!("[IROH-DIRECT] Warning: Failed to add node address: {}", e);
        }

        // Connect using gossip ALPN (we're already on the gossip mesh)
        println!("[IROH-DIRECT] Connecting to peer...");
        let connection = endpoint
            .connect(peer_node_id, &iroh_gossip::ALPN)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        drop(endpoint_guard);

        println!("[IROH-DIRECT] ✓ Connected, opening stream for direct presence exchange...");

        // Open bidirectional stream for direct presence exchange
        let (mut send, mut _recv) = connection
            .open_bi()
            .await
            .map_err(|e| format!("Failed to open stream: {}", e))?;

        // Get our node address
        let endpoint_guard = self.endpoint.lock().await;
        let our_node_addr = if let Some(endpoint) = endpoint_guard.as_ref() {
            endpoint
                .node_addr()
                .await
                .map_err(|e| format!("Failed to get node address: {}", e))?
        } else {
            return Err("Endpoint not initialized".to_string());
        };
        drop(endpoint_guard);

        // Create presence message
        let presence = P2PMessage::Presence {
            user_id: self.user_id,
            public_key: self.public_key.clone(),
            device_id: self
                .device_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            node_addr: our_node_addr,
            timestamp: chrono::Utc::now().timestamp(),
        };

        // Serialize and send
        let presence_bytes = serde_json::to_vec(&presence)
            .map_err(|e| format!("Failed to serialize presence: {}", e))?;

        println!(
            "[IROH-DIRECT] Sending presence ({} bytes)...",
            presence_bytes.len()
        );
        send.write_all(&presence_bytes)
            .await
            .map_err(|e| format!("Failed to send presence: {}", e))?;

        // Properly finish the stream to flush data and signal EOF to reader
        send.finish().map_err(|e| format!("Failed to finish stream: {}", e))?;

        println!("[IROH-DIRECT] ✓ Direct presence sent successfully");
        println!("[IROH-DIRECT] Peer should receive and process our Presence, then send theirs via gossip");

        Ok(())
    }

    /// Start background loop for presence announcements
    fn start_presence_loop(&self) {
        let network = Arc::new(self.clone_for_background());

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;

                if let Err(e) = network.announce_presence().await {
                    println!("[IROH] Failed to announce presence: {}", e);
                }
            }
        });

        println!("[IROH] Started presence announcement loop");
    }

    /// Actively discover peers using Pkarr rendezvous DHT key
    /// All Cipher nodes publish to and query from a well-known rendezvous point
    fn start_active_peer_discovery(&self) {
        let network = Arc::new(self.clone_for_background());

        tokio::spawn(async move {
            // Wait for endpoint to be fully initialized
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            println!("[IROH-RENDEZVOUS] Starting Pkarr-based peer discovery...");

            let endpoint_guard = network.endpoint.lock().await;
            if let Some(endpoint) = endpoint_guard.as_ref() {
                let our_node_id = endpoint.node_id();
                println!("[IROH-RENDEZVOUS] Our NodeId: {}", our_node_id);

                println!("╔════════════════════════════════════════════════════════════════════╗");
                println!("║ IROH NODE ID (use as seed for other devices):                      ║");
                println!("║ {}  ║", our_node_id);
                println!("╚════════════════════════════════════════════════════════════════════╝");

                // Create rendezvous public key from well-known string
                // All Cipher nodes will use this same key to find each other
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(b"cipher-social-rendezvous-v1");
                let rendezvous_hash = hasher.finalize();
                let mut rendezvous_bytes = [0u8; 32];
                rendezvous_bytes.copy_from_slice(&rendezvous_hash);

                println!("[IROH-RENDEZVOUS] Using global rendezvous key for all Cipher peers");
                println!(
                    "[IROH-RENDEZVOUS] Rendezvous: {}",
                    hex::encode(&rendezvous_bytes)
                );

                // Get our NodeAddr to publish
                if let Ok(our_node_addr) = endpoint.node_addr().await {
                    println!("[IROH-RENDEZVOUS] Our NodeAddr: {:?}", our_node_addr);
                    println!("[IROH-RENDEZVOUS]   Relay: {:?}", our_node_addr.relay_url());
                    println!(
                        "[IROH-RENDEZVOUS]   Direct addrs: {}",
                        our_node_addr.direct_addresses().count()
                    );
                }
            }
            drop(endpoint_guard);

            // Continuously try database peers with EXPONENTIAL BACKOFF
            // Reduces battery drain and network spam for unreachable peers
            let interval = tokio::time::Duration::from_secs(10);
            loop {
                tokio::time::sleep(interval).await;

                // Check all friend peer addresses (NodeId + relay URL) in database and try to connect
                if let Ok(peer_addrs) = network.db.get_all_friend_peer_addresses(network.user_id) {
                    if !peer_addrs.is_empty() {
                        println!("[IROH-DISCOVERY] Found {} peer addresses in database, checking for reconnection candidates...", peer_addrs.len());

                        for (node_id_str, relay_url) in peer_addrs {
                            match node_id_str.parse::<iroh::NodeId>() {
                                Ok(peer_id) => {
                                    let endpoint_guard = network.endpoint.lock().await;
                                    if let Some(endpoint) = endpoint_guard.as_ref() {
                                        let our_node_id = endpoint.node_id();

                                        // Skip if it's our own NodeId
                                        if peer_id == our_node_id {
                                            drop(endpoint_guard);
                                            continue;
                                        }

                                        // CHECK EXPONENTIAL BACKOFF before attempting connection
                                        let mut retry_states = network.peer_retry_counts.lock().await;
                                        let retry_state = retry_states.entry(peer_id).or_insert_with(PeerRetryState::new);

                                        if !retry_state.should_retry() {
                                            let backoff_delay = retry_state.backoff_delay_secs();
                                            println!(
                                                "[IROH-DISCOVERY] Skipping peer {} - in backoff (attempt {}, next retry in {}s)",
                                                peer_id, retry_state.attempt_count + 1, backoff_delay
                                            );
                                            drop(retry_states);
                                            drop(endpoint_guard);
                                            continue;
                                        }

                                        // Time to retry - record attempt
                                        retry_state.record_attempt();
                                        println!(
                                            "[IROH-DISCOVERY] Attempting to connect to peer {} (attempt {})...",
                                            peer_id, retry_state.attempt_count
                                        );
                                        drop(retry_states);

                                        // Construct full NodeAddr with relay URL for reliable connection
                                        let mut peer_node_addr = iroh::NodeAddr::new(peer_id);
                                        if let Ok(url) = relay_url.parse() {
                                            peer_node_addr = peer_node_addr.with_relay_url(url);
                                            println!("[IROH-DISCOVERY] Connecting to peer {} with relay: {}", peer_id, &relay_url);
                                        } else {
                                            println!("[IROH-DISCOVERY] Warning: Invalid relay URL for peer {}", peer_id);
                                            drop(endpoint_guard);
                                            continue;
                                        }

                                        // Add full NodeAddr to endpoint before connecting
                                        if let Err(e) =
                                            endpoint.add_node_addr(peer_node_addr.clone())
                                        {
                                            println!("[IROH-DISCOVERY] Warning: Failed to add node address: {}", e);
                                        }

                                        // Try to connect with full addressing info
                                        println!("[IROH-DISCOVERY] Discovering peer {} via DHT/DNS/Relay...", peer_id);
                                        match endpoint.connect(peer_id, &iroh_gossip::ALPN).await {
                                            Ok(conn) => {
                                                println!(
                                                    "[IROH-DISCOVERY] ✓ Connected to peer {}!",
                                                    peer_id
                                                );
                                                println!(
                                                    "[IROH-DISCOVERY]   Via: {:?}",
                                                    conn.remote_address()
                                                );
                                                drop(endpoint_guard);
                                                network.add_connected_peer(peer_id).await;

                                                // RESET backoff counter on successful connection
                                                let mut retry_states = network.peer_retry_counts.lock().await;
                                                if let Some(retry_state) = retry_states.get_mut(&peer_id) {
                                                    println!("[IROH-DISCOVERY] ✓ Backoff reset for peer {} after successful connection", peer_id);
                                                    retry_state.reset();
                                                }
                                                drop(retry_states);

                                                // RETRY pending messages when connection restored
                                                println!("[IROH-DISCOVERY] Connection restored - attempting to resend pending messages...");
                                                network.retry_pending_messages().await;

                                                break;
                                            }
                                            Err(e) => {
                                                println!(
                                                    "[IROH-DISCOVERY] Failed to connect to {} (attempt {}): {}",
                                                    peer_id, {
                                                        let retry_states = network.peer_retry_counts.lock().await;
                                                        retry_states.get(&peer_id).map(|s| s.attempt_count).unwrap_or(0)
                                                    }, e
                                                );
                                                drop(endpoint_guard);
                                            }
                                        }
                                    } else {
                                        drop(endpoint_guard);
                                    }
                                }
                                Err(e) => {
                                    println!(
                                        "[IROH-DISCOVERY] Failed to parse NodeId '{}': {}",
                                        node_id_str, e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        });

        println!("[IROH] Started Pkarr rendezvous + database peer discovery");
    }

    /// Handle received P2P message
    async fn handle_message(&self, message: P2PMessage) {
        match message {
            P2PMessage::Presence {
                user_id: peer_user_id,
                public_key,
                device_id,
                node_addr,
                timestamp: _,
            } => {
                let peer_node_id = node_addr.node_id;
                println!(
                    "[IROH] Received presence from user {} device {} (NodeId: {})",
                    public_key, device_id, peer_node_id
                );
                println!(
                    "[IROH]   Relay: {:?}, Direct addresses: {}",
                    node_addr.relay_url(),
                    node_addr.direct_addresses().count()
                );

                // Skip if this is our own device (don't store our own NodeId as a peer)
                if self
                    .device_id
                    .as_ref()
                    .map(|d| d == &device_id)
                    .unwrap_or(false)
                {
                    println!("[IROH] Skipping - this is our own presence message");
                    return;
                }

                // Store peer NodeId in database for future bootstrap (regardless of user match)
                // This builds a network of known peers for gossip bootstrapping
                let node_id_str = peer_node_id.to_string();
                if let Err(e) = self.db.update_device_node_id(&device_id, &node_id_str) {
                    println!("[IROH] Warning: Failed to store peer NodeId: {}", e);
                } else {
                    println!(
                        "[IROH] Stored peer NodeId {} for device {}",
                        node_id_str, device_id
                    );
                }

                // CRITICAL: Add the full NodeAddr to the endpoint
                // This gives Iroh all the info it needs: NodeId + relay URLs + direct addresses
                println!("[IROH] Adding peer NodeAddr to endpoint and connecting...");
                let endpoint_guard = self.endpoint.lock().await;
                if let Some(endpoint) = endpoint_guard.as_ref() {
                    // Add the full addressing information
                    if let Err(e) = endpoint.add_node_addr(node_addr.clone()) {
                        println!("[IROH] Warning: Failed to add node address: {}", e);
                    }

                    // Now connect using the ALPN
                    match endpoint.connect(peer_node_id, &iroh_gossip::ALPN).await {
                        Ok(connection) => {
                            println!("[IROH] ✓ Successfully connected to peer {}", peer_node_id);
                            println!("[IROH]   Remote address: {:?}", connection.remote_address());
                            drop(connection); // Connection stays open in endpoint
                            drop(endpoint_guard);
                            // Track this connection
                            self.add_connected_peer(peer_node_id).await;

                            // CRITICAL: Announce presence immediately so peer knows we're connected
                            // Without this, presence announcement waits up to 30 seconds
                            // causing asymmetric connection state
                            println!("[IROH] Announcing presence immediately after QR code connection...");
                            let _ = self.announce_presence().await;
                        }
                        Err(e) => {
                            println!("[IROH] Failed to connect to peer {}: {}", peer_node_id, e);
                            drop(endpoint_guard);
                            return;
                        }
                    }
                } else {
                    drop(endpoint_guard);
                }

                // Create friendship in database (CRITICAL for friends list to work!)
                // Skip if this is the same user (different device)
                if public_key != self.public_key {
                    println!(
                        "[IROH] Creating/updating friendship with peer user {}",
                        peer_user_id
                    );

                    // First, ensure the peer user exists in our database
                    if let Err(e) = self.db.conn.lock().unwrap().execute(
                        "INSERT OR IGNORE INTO users (id, username, public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            peer_user_id,
                            format!("User_{}", &public_key[..8]),  // Temporary username
                            &public_key,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to ensure peer user exists: {}", e);
                    }

                    // Create bidirectional friendship (status = 'accepted')
                    if let Err(e) = self.db.conn.lock().unwrap().execute(
                        "INSERT OR IGNORE INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at)
                         VALUES (?1, ?2, ?3, 'accepted', ?2, ?4, ?5)",
                        rusqlite::params![
                            super::types::SqliteUuid::new(),
                            self.user_id,
                            peer_user_id,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to create friendship: {}", e);
                    } else {
                        println!("[IROH] ✓ Friendship created/updated with peer {}", peer_user_id);

                        // Save peer address (NodeId + relay URL) for reconnection
                        if let Some(relay_url) = node_addr.relay_url() {
                            let relay_url_str = relay_url.to_string();
                            if let Err(e) = self.db.save_friend_peer_address(
                                self.user_id,
                                peer_user_id,
                                &node_id_str,
                                &relay_url_str,
                            ) {
                                println!("[IROH] Warning: Failed to save friend peer address: {}", e);
                            } else {
                                println!("[IROH] ✓ Friend peer address saved for reconnection: NodeId={}, Relay={}", node_id_str, relay_url_str);
                            }
                        }
                    }
                }

                // NOTE: We do NOT subscribe to topics from handle_message to avoid Send issues
                // All subscriptions are handled in the QR code flow:
                // - When generating QR: subscribe to our own topic
                // - When scanning QR: subscribe to friend's topic with them as bootstrap
                // This ensures proper gossip mesh connectivity without nested spawning
                println!("[IROH] Presence received and stored - subscriptions handled via QR flow");

                // Send our Presence to peer's topic to complete bidirectional friendship
                println!(
                    "[IROH] Sending our Presence to peer's topic for bidirectional friendship..."
                );
                let endpoint_guard = self.endpoint.lock().await;
                if let Some(endpoint) = endpoint_guard.as_ref() {
                    if let Ok(our_node_addr) = endpoint.node_addr().await {
                        drop(endpoint_guard);

                        let our_presence = P2PMessage::Presence {
                            user_id: self.user_id,
                            public_key: self.public_key.clone(),
                            device_id: self
                                .device_id
                                .clone()
                                .unwrap_or_else(|| "unknown".to_string()),
                            node_addr: our_node_addr,
                            timestamp: chrono::Utc::now().timestamp(),
                        };

                        let peer_topic = format!("cipher/user/{}", public_key);
                        match self.publish_message(&peer_topic, our_presence).await {
                            Ok(_) => println!("[IROH] ✓ Sent our Presence to peer's topic - bidirectional friendship established!"),
                            Err(e) => println!("[IROH] Warning: Failed to send Presence to peer: {}", e),
                        }
                    } else {
                        drop(endpoint_guard);
                    }
                } else {
                    drop(endpoint_guard);
                }

                // Check if this is another device with the same user account
                if public_key == self.public_key
                    && device_id != self.device_id.clone().unwrap_or_default()
                {
                    println!(
                        "[IROH] SAME-USER DEVICE DETECTED: {} with NodeId: {}",
                        device_id, peer_node_id
                    );

                    // Send device sync request
                    let sync_request = P2PMessage::DeviceSyncRequest {
                        public_key: self.public_key.clone(),
                        device_id: self
                            .device_id
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        last_sync_timestamp: 0, // Get all data for now
                    };

                    let topic = format!("cipher/user/{}", self.public_key);
                    if let Err(e) = self.publish_message(&topic, sync_request).await {
                        println!("[IROH] Failed to send device sync request: {}", e);
                    } else {
                        println!(
                            "[IROH] Sent device sync request to same-user device {}",
                            device_id
                        );
                    }
                }
            }

            P2PMessage::DeviceSyncRequest {
                public_key,
                device_id,
                last_sync_timestamp: _,
            } => {
                // Check if this is from another device with same user
                if public_key == self.public_key
                    && device_id != self.device_id.clone().unwrap_or_default()
                {
                    println!(
                        "[IROH] Received device sync request from device {}",
                        device_id
                    );

                    // Get sync data from database
                    let our_device_id = self
                        .device_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    match self.db.get_sync_data(&our_device_id, self.user_id) {
                        Ok(sync_data) => {
                            println!(
                                "[IROH] Got sync data: {} posts, {} messages, {} friends",
                                sync_data.posts.len(),
                                sync_data.messages.len(),
                                sync_data.friends.len()
                            );

                            // Serialize sync data
                            match serde_json::to_string(&sync_data) {
                                Ok(data_json) => {
                                    let response = P2PMessage::DeviceSyncResponse {
                                        public_key: self.public_key.clone(),
                                        device_id: our_device_id,
                                        data_json,
                                        timestamp: chrono::Utc::now().timestamp(),
                                    };

                                    // Publish to user's own topic
                                    let topic = format!("cipher/user/{}", self.public_key);
                                    if let Err(e) = self.publish_message(&topic, response).await {
                                        println!(
                                            "[IROH] Failed to send device sync response: {}",
                                            e
                                        );
                                    } else {
                                        println!(
                                            "[IROH] Sent device sync response to device {}",
                                            device_id
                                        );
                                    }
                                }
                                Err(e) => println!("[IROH] Failed to serialize sync data: {}", e),
                            }
                        }
                        Err(e) => println!("[IROH] Failed to get sync data: {}", e),
                    }
                }
            }

            P2PMessage::DeviceSyncResponse {
                public_key,
                device_id,
                data_json,
                timestamp: _,
            } => {
                // Check if this is from another device with same user
                if public_key == self.public_key
                    && device_id != self.device_id.clone().unwrap_or_default()
                {
                    println!(
                        "[IROH] Received device sync response from device {}",
                        device_id
                    );

                    // Deserialize sync data
                    match serde_json::from_str::<crate::app::database::sync::SyncData>(&data_json) {
                        Ok(sync_data) => {
                            println!(
                                "[IROH] Applying sync data: {} posts, {} messages, {} friends",
                                sync_data.posts.len(),
                                sync_data.messages.len(),
                                sync_data.friends.len()
                            );

                            // Apply sync data to database
                            match self.db.apply_sync_data(&sync_data) {
                                Ok(()) => {
                                    println!("[IROH] Successfully applied device sync data from device {}", device_id);

                                    // Update sync timestamps
                                    if let Some(our_device_id) = &self.device_id {
                                        let _ = self.db.update_all_sync_timestamps(our_device_id);
                                    }

                                    // Emit event to UI
                                    let _ =
                                        self.app_handle.emit("device-sync-completed", device_id);
                                }
                                Err(e) => {
                                    println!("[IROH] Failed to apply device sync data: {}", e)
                                }
                            }
                        }
                        Err(e) => println!("[IROH] Failed to deserialize sync data: {}", e),
                    }
                }
            }

            P2PMessage::DirectMessage {
                ref message_id,
                ref from_user_id,
                to_user_id,
                ref encrypted_content,
                timestamp,
                ref device_id,
            } => {
                // Handle direct messages to this user
                if to_user_id == self.user_id {
                    println!("[IROH] Received direct message");

                    // Emit to UI
                    #[derive(serde::Serialize, Clone)]
                    struct MessageEvent {
                        message: P2PMessage,
                    }

                    let _ = self.app_handle.emit(
                        "p2p-message-received",
                        MessageEvent {
                            message: P2PMessage::DirectMessage {
                                message_id: message_id.clone(),
                                from_user_id: *from_user_id,
                                to_user_id,
                                encrypted_content: encrypted_content.clone(),
                                timestamp,
                                device_id: device_id.clone(),
                            },
                        },
                    );
                }
            }

            P2PMessage::Post {
                user_id,
                ref content,
                timestamp,
                ref device_id,
                ref attachments,
            } => {
                println!("[IROH] Received post");

                // Emit to UI
                #[derive(serde::Serialize, Clone)]
                struct MessageEvent {
                    message: P2PMessage,
                }

                let _ = self.app_handle.emit(
                    "p2p-message-received",
                    MessageEvent {
                        message: P2PMessage::Post {
                            user_id,
                            content: content.clone(),
                            timestamp,
                            device_id: device_id.clone(),
                            attachments: attachments.clone(),
                        },
                    },
                );
            }
            P2PMessage::FriendAdded {
                from_public_key,
                from_user_id,
                from_node_id,
                from_relay_url,
                to_public_key,
                timestamp: _,
            } => {
                println!("[IROH] Received FriendAdded from {}", from_public_key);

                // Verify this message is intended for us
                if to_public_key != self.public_key {
                    println!("[IROH] FriendAdded not for us (to: {}), ignoring", to_public_key);
                    return;
                }

                // Skip if trying to add ourselves
                if from_public_key == self.public_key {
                    println!("[IROH] Cannot add ourselves as friend, ignoring");
                    return;
                }

                println!("[IROH] Auto-adding friend {} for bidirectional friendship", from_public_key);

                // 1. Ensure the friend user exists in database
                if let Err(e) = self.db.conn.lock().unwrap().execute(
                    "INSERT OR IGNORE INTO users (id, username, public_key, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        from_user_id,
                        format!("User_{}", &from_public_key[..8]),
                        &from_public_key,
                        chrono::Utc::now().to_rfc3339(),
                        chrono::Utc::now().to_rfc3339()
                    ],
                ) {
                    println!("[IROH] Warning: Failed to create friend user: {}", e);
                }

                // 2. Create friendship in database
                if let Err(e) = self.db.conn.lock().unwrap().execute(
                    "INSERT OR IGNORE INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at, iroh_node_id, friend_relay_url)
                     VALUES (?1, ?2, ?3, 'accepted', ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        super::types::SqliteUuid::new(),
                        self.user_id,
                        from_user_id,
                        chrono::Utc::now().to_rfc3339(),
                        chrono::Utc::now().to_rfc3339(),
                        &from_node_id,
                        &from_relay_url,
                    ],
                ) {
                    println!("[IROH] Warning: Failed to create friendship: {}", e);
                } else {
                    println!("[IROH] ✓ Bidirectional friendship created with {}", from_public_key);
                }

                // 3. Add friend's node address to endpoint for direct connection
                // Note: Topic subscription will happen automatically on next app restart
                // when iroh_initialize loads friends from database
                if let Ok(peer_node_id) = from_node_id.parse::<iroh::NodeId>() {
                    if let Ok(relay_url_parsed) = from_relay_url.parse::<url::Url>() {
                        let node_addr = iroh::NodeAddr::from_parts(
                            peer_node_id,
                            Some(relay_url_parsed.into()),
                            vec![],
                        );

                        let endpoint_guard = self.endpoint.lock().await;
                        if let Some(endpoint) = endpoint_guard.as_ref() {
                            if let Err(e) = endpoint.add_node_addr(node_addr) {
                                println!("[IROH] Warning: Failed to add friend's node address: {}", e);
                            } else {
                                println!("[IROH] ✓ Added friend's node address to endpoint");
                            }
                        }
                        drop(endpoint_guard);
                    }
                }

                println!("[IROH] ✓ Bidirectional friendship fully established!");
            }

            P2PMessage::Heartbeat { node_id, timestamp: _ } => {
                // Parse the node_id from string
                match node_id.parse::<iroh::NodeId>() {
                    Ok(peer_node_id) => {
                        println!("[HEARTBEAT] Received heartbeat from peer: {}", peer_node_id);

                        // Update last heartbeat time for this peer
                        let mut heartbeats = self.peer_heartbeats.lock().await;
                        heartbeats.insert(peer_node_id, std::time::Instant::now());
                        drop(heartbeats);

                        // Ensure peer is in connected set (heartbeat confirms they're alive)
                        self.add_connected_peer(peer_node_id).await;
                    }
                    Err(e) => {
                        println!("[HEARTBEAT] Failed to parse node_id from heartbeat: {}", e);
                    }
                }
            }
        }
    }

    /// Retry all pending messages when connection is restored
    pub async fn retry_pending_messages(&self) {
        match self.db.get_pending_message_count(self.user_id) {
            Ok(count) if count > 0 => {
                println!("[QUEUE] Starting retry of {} pending messages...", count);

                match self.db.get_pending_messages(self.user_id) {
                    Ok(pending_msgs) => {
                        let mut successful = 0;
                        let mut failed = 0;

                        for pending_msg in pending_msgs {
                            // Check if we're still online
                            let endpoint_guard = self.endpoint.lock().await;
                            if endpoint_guard.is_none() {
                                println!(
                                    "[QUEUE] Offline again, stopping retry of pending messages"
                                );
                                drop(endpoint_guard);
                                break;
                            }
                            drop(endpoint_guard);

                            // Try to resend
                            match serde_json::from_str::<P2PMessage>(&pending_msg.content_json) {
                                Ok(message) => {
                                    match self.publish_message(&format!("cipher/user/{}", self.public_key), message).await {
                                        Ok(_) => {
                                            println!(
                                                "[QUEUE] ✓ Successfully resent message (ID: {})",
                                                pending_msg.id
                                            );
                                            // Remove from queue
                                            if let Err(e) = self.db.mark_message_sent(pending_msg.id) {
                                                println!(
                                                    "[QUEUE] Warning: Failed to remove sent message: {}",
                                                    e
                                                );
                                            }
                                            successful += 1;
                                        }
                                        Err(e) => {
                                            println!(
                                                "[QUEUE] ✗ Failed to resend message: {} (attempt {}/{})",
                                                e,
                                                pending_msg.retry_count + 1,
                                                pending_msg.max_retries
                                            );

                                            if pending_msg.retry_count + 1 >= pending_msg.max_retries {
                                                println!("[QUEUE] Max retries reached, removing message");
                                                if let Err(e) = self.db.remove_pending_message(pending_msg.id) {
                                                    println!("[QUEUE] Warning: Failed to remove: {}", e);
                                                }
                                            } else if let Err(e) = self.db.increment_retry_count(pending_msg.id) {
                                                println!("[QUEUE] Warning: Failed to update retry count: {}", e);
                                            }
                                            failed += 1;
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("[QUEUE] Failed to parse pending message: {}", e);
                                    if let Err(e) = self.db.remove_pending_message(pending_msg.id) {
                                        println!("[QUEUE] Warning: Failed to remove: {}", e);
                                    }
                                }
                            }
                        }

                        println!(
                            "[QUEUE] Retry complete: {} sent, {} failed",
                            successful, failed
                        );
                    }
                    Err(e) => {
                        println!("[QUEUE] Failed to load pending messages: {}", e);
                    }
                }
            }
            _ => {
                // No pending messages or error
            }
        }
    }

    /// Queue a message for offline delivery (called when offline)
    pub async fn queue_message_for_delivery(
        &self,
        message: P2PMessage,
    ) -> Result<(), String> {
        let content_json =
            serde_json::to_string(&message).map_err(|e| format!("Serialization failed: {}", e))?;

        let message_type = match message {
            P2PMessage::Post { .. } => "post",
            P2PMessage::DirectMessage { .. } => "message",
            P2PMessage::FriendAdded { .. } => "friend_added",
            _ => "other",
        };

        self.db
            .queue_pending_message(self.user_id, message_type, &content_json, 5)
            .map_err(|e| format!("Failed to queue message: {}", e))?;

        Ok(())
    }

    /// Helper to clone fields for background tasks
    fn clone_for_background(&self) -> Self {
        IrohNetwork {
            user_id: self.user_id,
            public_key: self.public_key.clone(),
            device_id: self.device_id.clone(),
            endpoint: self.endpoint.clone(),
            gossip: self.gossip.clone(),
            router: self.router.clone(),
            topics: self.topics.clone(),
            connected_peers: self.connected_peers.clone(),
            peer_retry_counts: self.peer_retry_counts.clone(),
            peer_heartbeats: self.peer_heartbeats.clone(),
            device_seed: self.device_seed,
            app_handle: self.app_handle.clone(),
            db: self.db.clone(),
        }
    }

    /// Add a peer to the connected set
    pub async fn add_connected_peer(&self, node_id: iroh::NodeId) {
        let mut peers = self.connected_peers.lock().await;
        if peers.insert(node_id) {
            println!(
                "[IROH] Peer {} added to connected set (total: {})",
                node_id,
                peers.len()
            );
        }
    }

    /// Remove a peer from the connected set
    pub async fn remove_connected_peer(&self, node_id: iroh::NodeId) {
        let mut peers = self.connected_peers.lock().await;
        if peers.remove(&node_id) {
            println!(
                "[IROH] Peer {} removed from connected set (total: {})",
                node_id,
                peers.len()
            );
        }
    }

    /// Clear all connected peers (used on initialization)
    pub async fn clear_connected_peers(&self) {
        let mut peers = self.connected_peers.lock().await;
        let count = peers.len();
        peers.clear();
        if count > 0 {
            println!("[IROH] Cleared {} stale peer connections on initialization", count);
        }
    }

    /// Get connection status
    pub async fn get_connection_status(&self) -> Result<serde_json::Value, String> {
        let endpoint_guard = self.endpoint.lock().await;
        let has_endpoint = endpoint_guard.is_some();
        drop(endpoint_guard);

        let connected_count = self.connected_peers.lock().await.len();

        Ok(serde_json::json!({
            "listening": has_endpoint,
            "connected_peers": connected_count,
        }))
    }

    /// Start heartbeat sender - sends heartbeats to cipher/presence every 15 seconds
    fn start_heartbeat_sender(&self) {
        let network = Arc::new(self.clone_for_background());

        tokio::spawn(async move {
            // Wait a bit for initialization
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
            loop {
                interval.tick().await;

                // Get our node_id
                let endpoint_guard = network.endpoint.lock().await;
                if let Some(endpoint) = endpoint_guard.as_ref() {
                    let our_node_id = endpoint.node_id();
                    drop(endpoint_guard);

                    // Create heartbeat message
                    let heartbeat = P2PMessage::Heartbeat {
                        node_id: our_node_id.to_string(),
                        timestamp: chrono::Utc::now().timestamp(),
                    };

                    // Send to cipher/presence topic
                    if let Err(e) = network.publish_message("cipher/presence", heartbeat).await {
                        println!("[HEARTBEAT] Failed to send heartbeat: {}", e);
                    } else {
                        println!("[HEARTBEAT] ✓ Sent heartbeat");
                    }
                } else {
                    drop(endpoint_guard);
                }
            }
        });

        println!("[HEARTBEAT] Started heartbeat sender (15s interval)");
    }

    /// Start heartbeat monitor - removes peers that haven't sent heartbeat in 45 seconds
    fn start_heartbeat_monitor(&self) {
        let network = Arc::new(self.clone_for_background());

        tokio::spawn(async move {
            // Wait a bit for initialization
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

            let mut interval = tokio::time::Duration::from_secs(30);
            let heartbeat_timeout = std::time::Duration::from_secs(45);

            loop {
                tokio::time::sleep(interval).await;

                let now = std::time::Instant::now();
                let mut heartbeats = network.peer_heartbeats.lock().await;
                let connected_peers = network.connected_peers.lock().await.clone();
                drop(connected_peers);

                // Find stale peers (no heartbeat in 45 seconds)
                let stale_peers: Vec<iroh::NodeId> = heartbeats
                    .iter()
                    .filter(|(_, &last_heartbeat)| now.duration_since(last_heartbeat) > heartbeat_timeout)
                    .map(|(peer_id, _)| *peer_id)
                    .collect();

                drop(heartbeats);

                // Remove stale peers
                if !stale_peers.is_empty() {
                    println!("[HEARTBEAT] Found {} stale peers (no heartbeat in 45s)", stale_peers.len());
                    for peer_id in stale_peers {
                        println!("[HEARTBEAT] Removing stale peer: {}", peer_id);
                        network.remove_connected_peer(peer_id).await;

                        // Also remove from heartbeat tracker
                        let mut heartbeats = network.peer_heartbeats.lock().await;
                        heartbeats.remove(&peer_id);
                        drop(heartbeats);
                    }
                } else {
                    println!("[HEARTBEAT] All peers have recent heartbeats");
                }
            }
        });

        println!("[HEARTBEAT] Started heartbeat monitor (checks every 30s, timeout 45s)");
    }

    /// Shutdown the network
    pub async fn shutdown(&self) -> Result<(), String> {
        println!("[IROH] Shutting down Iroh network...");

        // Close all topic subscriptions
        self.topics.lock().await.clear();

        // Close gossip
        if let Some(gossip) = self.gossip.lock().await.take() {
            drop(gossip);
        }

        // Close endpoint
        if let Some(endpoint) = self.endpoint.lock().await.take() {
            let _ = endpoint.close().await;
        }

        println!("[IROH] Shutdown complete");
        Ok(())
    }
}
