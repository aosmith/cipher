// Iroh-based P2P networking
// Global mesh architecture: all nodes connect, only friends can decrypt content
//
// ARCHITECTURE:
// - Single `cipher/content/v1` topic for ALL encrypted content
// - All nodes join the same global mesh
// - Content is encrypted per-friend (sealed boxes)
// - Only intended recipients can decrypt
// - Non-friends relay and optionally cache for later purging

use bytes::Bytes;
use iroh::protocol::Router;
use iroh_blobs::net_protocol::Blobs;
use iroh_gossip::ALPN;
use iroh_gossip::net::{Event as GossipNetEvent, GossipEvent, GossipReceiver, GossipSender};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;

use super::types::{BlobReference, MediaAttachmentWithData, SqliteUuid};
use super::Database;

/// Global topic for all encrypted content
pub const CONTENT_TOPIC: &str = "cipher/content/v1";

/// P2P message types for the social network
/// All messages are broadcast to the global mesh; encryption ensures privacy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum P2PMessage {
    /// User presence announcement (for device discovery and mesh health)
    /// Broadcast to global mesh - all nodes see presence, helps with peer discovery
    /// SECURITY: Includes signed profile data so friends can verify identity
    Presence {
        user_id: SqliteUuid,
        public_key: String,
        device_id: String,
        node_addr: iroh::NodeAddr, // Full addressing info for direct peer connection
        timestamp: i64,
        // Profile data for identity verification
        display_name: String,
        bio: String,
        profile_picture: String,
        /// Cryptographic signature of profile data (display_name|bio|profile_picture)
        /// Signed with user's Ed25519 private key, verifiable with public_key
        profile_signature: Option<String>,
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
    /// Direct encrypted message between users (legacy - kept for backwards compatibility)
    DirectMessage {
        message_id: String,
        from_user_id: SqliteUuid,
        to_user_id: SqliteUuid,
        encrypted_content: String,
        timestamp: i64,
        device_id: Option<String>,
    },
    /// Post from a user (legacy - kept for backwards compatibility)
    /// WARNING: Do not use for posts with attachments - use PostWithBlobs instead
    Post {
        user_id: SqliteUuid,
        public_key: String, // Sender's public key for user record creation
        content: String,
        timestamp: i64,
        device_id: Option<String>,
        attachments: Option<Vec<MediaAttachmentWithData>>,
    },
    /// Post with blob-based attachments (for large files like images)
    /// Attachments are stored in iroh-blobs and referenced by hash
    /// Receiver fetches blobs directly from sender's node
    PostWithBlobs {
        user_id: SqliteUuid,
        public_key: String,
        node_id: String,              // Sender's NodeId for blob fetching
        content: String,
        timestamp: i64,
        device_id: Option<String>,
        blob_refs: Vec<BlobReference>, // References to blobs in sender's store
    },
    /// PHASE 2: Sealed envelope containing encrypted content
    /// All nodes receive this, but only friends can decrypt
    SealedEnvelope {
        /// The full GossipEnvelope with sealed boxes
        envelope_json: String,
    },
    /// Friend request sent when scanning QR code
    /// Broadcast to global mesh - only the target can process it
    FriendRequest {
        from_public_key: String,     // Requester's public key
        from_user_id: SqliteUuid,     // Requester's user ID
        from_display_name: String,   // Requester's display name
        from_node_id: String,          // Requester's Iroh NodeId
        from_relay_url: String,        // Requester's relay URL
        to_public_key: String,        // Target's public key (for filtering)
        timestamp: i64,
    },
    /// Friend request acceptance
    /// Broadcast to global mesh - only the requester can process it
    FriendAccepted {
        from_user_id: SqliteUuid,     // Accepter's user ID
        from_public_key: String,     // Accepter's public key
        from_display_name: String,   // Accepter's display name
        from_node_id: String,          // Accepter's Iroh NodeId
        from_relay_url: String,        // Accepter's relay URL
        to_public_key: String,        // Original requester's public key (for filtering)
    },
    /// Heartbeat message to verify peer is still connected
    /// Broadcast to global mesh for accurate peer counting
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
    #[allow(dead_code)] // Kept for future dynamic peer joining
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
    pub display_name: String,
    pub public_key: String,
    pub device_id: Option<String>,
    pub endpoint: Arc<Mutex<Option<iroh::Endpoint>>>,
    pub gossip: Arc<Mutex<Option<iroh_gossip::net::Gossip>>>,
    pub router: Arc<Mutex<Option<Router>>>,
    /// Blob store for large file transfers (images, attachments)
    /// Uses iroh-blobs for efficient P2P transfer over existing NAT-traversed connections
    pub blobs: Arc<Mutex<Option<Blobs<iroh_blobs::store::mem::Store>>>>,
    topics: Arc<Mutex<HashMap<String, TopicSubscription>>>,
    connected_peers: Arc<Mutex<std::collections::HashSet<iroh::NodeId>>>,
    peer_retry_counts: Arc<Mutex<HashMap<iroh::NodeId, PeerRetryState>>>, // Exponential backoff tracking
    peer_heartbeats: Arc<Mutex<HashMap<iroh::NodeId, std::time::Instant>>>, // Last heartbeat time from each peer
    pending_subscriptions: Arc<Mutex<Vec<String>>>, // Topics queued for subscription (processed in background)
    /// Recent message hashes for deduplication (gossip protocols may deliver same message via multiple paths)
    recent_message_hashes: Arc<Mutex<std::collections::HashSet<u64>>>,
    device_seed: [u8; 32], // Device keypair seed for DHT publishing
    app_handle: tauri::AppHandle,
    db: Database,
    /// Health tracking: last time presence was successfully sent
    last_presence_success: Arc<Mutex<Option<std::time::Instant>>>,
    /// Health tracking: last time heartbeat was successfully sent
    last_heartbeat_success: Arc<Mutex<Option<std::time::Instant>>>,
    /// Flag to signal background loops should stop (for clean shutdown)
    /// Public so app lifecycle handlers can signal shutdown
    pub shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl IrohNetwork {
    /// Create new Iroh network instance
    pub async fn new(
        user_id: SqliteUuid,
        display_name: String,
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
            display_name,
            public_key,
            device_id,
            endpoint: Arc::new(Mutex::new(None)),
            gossip: Arc::new(Mutex::new(None)),
            router: Arc::new(Mutex::new(None)),
            blobs: Arc::new(Mutex::new(None)),
            topics: Arc::new(Mutex::new(HashMap::new())),
            connected_peers: Arc::new(Mutex::new(std::collections::HashSet::new())),
            peer_retry_counts: Arc::new(Mutex::new(HashMap::new())),
            peer_heartbeats: Arc::new(Mutex::new(HashMap::new())),
            pending_subscriptions: Arc::new(Mutex::new(Vec::new())),
            recent_message_hashes: Arc::new(Mutex::new(std::collections::HashSet::new())),
            device_seed,
            app_handle,
            db,
            last_presence_success: Arc::new(Mutex::new(None)),
            last_heartbeat_success: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
        println!("[IROH]   [OK] DHT PUBLISHING ENABLED - We are discoverable!");
        println!("[IROH]   [OK] DHT (PRIMARY) - Fully decentralized, no servers required");
        println!("[IROH]   [OK] DNS discovery (n0) - Distributed peer finding");
        println!("[IROH]   [OK] Relay servers (FALLBACK) - NAT traversal helper only");
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

        // Create blob store for large file transfers (images, attachments)
        // Uses in-memory store - blobs are ephemeral and fetched on-demand from peers
        let blobs = Blobs::memory().build(&endpoint);
        println!("[IROH] Blob store initialized (in-memory)");

        // Create Router to accept incoming gossip AND blob connections
        // Both protocols share the same NAT-traversed QUIC connection
        let router = Router::builder(endpoint.clone())
            .accept(ALPN, gossip.clone())
            .accept(iroh_blobs::ALPN, blobs.clone())
            .spawn();

        println!("[IROH] Router created - accepting gossip and blob connections");
        println!("[IROH] Direct RPC acceptor ready for incoming Presence handshakes");

        // Store endpoint, gossip, blobs, and router
        *self.endpoint.lock().await = Some(endpoint.clone());
        *self.gossip.lock().await = Some(gossip);
        *self.blobs.lock().await = Some(blobs);
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
                        match endpoint.connect(peer_id, iroh_gossip::ALPN).await {
                            Ok(conn) => {
                                println!("[IROH] [OK] CONNECTED to peer {}!", peer_id);
                                println!("[IROH]   Connected to peer: {}", peer_id);
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

        // GLOBAL MESH ARCHITECTURE:
        // All nodes join the single cipher/content/v1 topic
        // This creates a global gossip mesh where everyone relays everything
        // Privacy is achieved through encryption, not topology
        println!("[IROH] ========================================");
        println!("[IROH] GLOBAL MESH ARCHITECTURE");
        println!("[IROH] Subscribing to global content topic: {}", CONTENT_TOPIC);
        println!("[IROH] All content will be broadcast to all peers");
        println!("[IROH] Only friends can decrypt your content");
        println!("[IROH] ========================================");

        // Subscribe to the global content topic
        // If we have bootstrap peers from previous sessions, use them
        if !peer_node_ids.is_empty() {
            println!("[IROH] Subscribing with {} bootstrap peers", peer_node_ids.len());
            if let Err(e) = self.subscribe_topic_with_peers(CONTENT_TOPIC, peer_node_ids.clone()).await {
                println!("[IROH] Warning: Failed to subscribe with bootstrap: {}", e);
                println!("[IROH] Falling back to root subscription...");
                self.subscribe_topic(CONTENT_TOPIC).await?;
            }
        } else {
            println!("[IROH] No bootstrap peers - subscribing as root node");
            self.subscribe_topic(CONTENT_TOPIC).await?;
        }
        println!("[IROH] [OK] Subscribed to global content topic");

        // Start presence announcements and heartbeat
        self.start_presence_loop();
        self.start_heartbeat_sender();
        self.start_heartbeat_monitor();

        // Actively try to connect to discovered peers via direct connection attempts
        // This helps bootstrap the gossip mesh using stored peer addresses
        // CENSORSHIP RESISTANCE: Even if infrastructure is blocked, users can exchange
        // peer addresses out-of-band (QR codes, messaging apps) and manually bootstrap
        self.start_active_peer_discovery();

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
                                        println!("[IROH] [OK] Pre-populated friend peer {}", peer_id);
                                        // Try to reconnect with retry logic (3 attempts with exponential backoff)
                                        let max_attempts = 3;
                                        let mut reconnected = false;

                                        for attempt in 1..=max_attempts {
                                            match endpoint.connect(peer_id, iroh_gossip::ALPN).await {
                                                Ok(conn) => {
                                                    println!("[IROH] [OK] RECONNECTED to friend {} on attempt {}!", peer_id, attempt);
                                                    println!(
                                                        "[IROH]   Connected to peer: {}",
                                                        peer_id
                                                    );
                                                    self.add_connected_peer(peer_id).await;

                                                    // Hand off QUIC connection to gossip protocol.
                                                    // Two-step process for OUTGOING connections:
                                                    // 1. handle_connection() - tells gossip about the connection
                                                    // 2. join_peers() - actively joins the peer to our topic mesh
                                                    let gossip_guard = self.gossip.lock().await;
                                                    if let Some(gossip) = gossip_guard.as_ref() {
                                                        match gossip.handle_connection(conn).await {
                                                            Ok(_) => println!("[IROH] [OK] Handed connection to gossip for friend {}", peer_id),
                                                            Err(e) => println!("[IROH] Warning: Failed to hand connection to gossip: {}", e),
                                                        }
                                                    }
                                                    drop(gossip_guard);

                                                    // CRITICAL: Call join_peers() to form gossip mesh
                                                    let topics_guard = self.topics.lock().await;
                                                    if let Some(subscription) = topics_guard.get(CONTENT_TOPIC) {
                                                        match subscription.gossip_sender.join_peers(vec![peer_id]).await {
                                                            Ok(_) => println!("[IROH] [OK] Joined gossip mesh with friend {}", peer_id),
                                                            Err(e) => println!("[IROH] Warning: Failed to join gossip mesh with friend: {}", e),
                                                        }
                                                    }
                                                    drop(topics_guard);

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
        // CRITICAL: Check if already subscribed to prevent duplicate handlers
        // Each subscription spawns a new stream handler task - if we subscribe twice,
        // we get two handlers processing every message, causing duplicates in the UI.
        if self.is_topic_subscribed(topic).await {
            println!("[IROH-GOSSIP] Topic '{}' already subscribed - skipping to prevent duplicate handlers", topic);
            return Ok(());
        }

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
                    println!("[IROH-GOSSIP] [OK] gossip.subscribe_and_join() completed for topic '{}'", topic);
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
                    return Err("Timed out joining gossip topic - gossip protocol did not form neighbor relationship".to_string());
                }
            }
        } else {
            println!("[IROH-GOSSIP] Calling gossip.subscribe() for topic '{}' (root node - empty bootstrap)...", topic);
            println!("[IROH-GOSSIP]   Using subscribe() instead of subscribe_and_join() for root nodes");
            println!("[IROH-GOSSIP]   subscribe() returns immediately, subscribe_and_join() waits for peers");

            // CRITICAL: Use subscribe() (NOT subscribe_and_join()) for root nodes!
            // subscribe_and_join() calls .joined().await which WAITS for at least one connection
            // With empty bootstrap and no peers, it will hang forever!
            // subscribe() returns immediately and creates a proper root node that accepts joins
            match gossip.subscribe(topic_id, bootstrap_peers.clone()) {
                Ok(result) => {
                    println!("[IROH-GOSSIP] [OK] gossip.subscribe() completed for topic '{}' (root node)", topic);
                    println!("[IROH-GOSSIP]   Root node created - ready to accept joins from peers");
                    result
                }
                Err(e) => {
                    println!("[IROH-GOSSIP] ✗ gossip.subscribe() failed for root node: {}", e);
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
                                    // DEDUPLICATION: Hash the message content and check if we've seen it recently
                                    // Gossip protocols may deliver the same message via multiple paths
                                    use std::hash::{Hash, Hasher};
                                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                                    msg.content.hash(&mut hasher);
                                    msg.delivered_from.hash(&mut hasher);
                                    let msg_hash = hasher.finish();

                                    {
                                        let mut seen = self.recent_message_hashes.lock().await;
                                        if seen.contains(&msg_hash) {
                                            // Already processed this message - skip
                                            continue;
                                        }
                                        seen.insert(msg_hash);
                                        // Limit cache size to prevent memory growth (keep last 1000 messages)
                                        if seen.len() > 1000 {
                                            // Clear oldest entries (simple approach - just clear half)
                                            let to_remove: Vec<_> = seen.iter().take(500).cloned().collect();
                                            for hash in to_remove {
                                                seen.remove(&hash);
                                            }
                                        }
                                    }

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
                                                    P2PMessage::PostWithBlobs { .. } => "PostWithBlobs",
                                                    P2PMessage::SealedEnvelope { .. } => "SealedEnvelope",
                                                    P2PMessage::FriendRequest { .. } => "FriendRequest",
                                                    P2PMessage::FriendAccepted { .. } => "FriendAccepted",
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
                                    for peer_id in peers.clone() {
                                        println!("[IROH-STREAM]   Adding peer to connected_peers: {}", peer_id);
                                        self.add_connected_peer(peer_id).await;
                                    }

                                    // CRITICAL: When peers join us, we also need to join them back
                                    // This ensures bidirectional gossip mesh connectivity
                                    let topics_guard = self.topics.lock().await;
                                    if let Some(subscription) = topics_guard.get(&topic) {
                                        match subscription.gossip_sender.join_peers(peers.clone()).await {
                                            Ok(_) => println!("[IROH-STREAM] [OK] Joined gossip mesh back with {} peer(s)", peers.len()),
                                            Err(e) => println!("[IROH-STREAM] Warning: Failed to join back: {}", e),
                                        }
                                    }
                                    drop(topics_guard);
                                }
                                GossipEvent::NeighborUp(peer) => {
                                    println!("[IROH-STREAM] 📡 Neighbor UP on topic '{}': {}", topic, peer);
                                    // Peer became our neighbor - add to connected_peers
                                    println!("[IROH-STREAM]   Adding peer to connected_peers: {}", peer);
                                    self.add_connected_peer(peer).await;
                                }
                                GossipEvent::NeighborDown(peer) => {
                                    println!("[IROH-STREAM] 📴 Neighbor DOWN on topic '{}': {}", topic, peer);
                                    // NeighborDown indicates the gossip neighbor relationship has ended.
                                    // This is a GOSSIP-LAYER event, not a connection loss!
                                    // The underlying QUIC connection may still be fine.
                                    //
                                    // DO NOT remove from connected_peers here!
                                    // The gossip protocol (HyParView) will naturally re-establish
                                    // neighbor relationships through its own mechanism.
                                    // Removing from connected_peers triggers the discovery loop
                                    // to create new subscriptions, causing duplicate handlers
                                    // and an infinite NeighborDown loop.
                                    println!("[IROH-STREAM]   Gossip layer event - connection may still be active");
                                    println!("[IROH-STREAM]   NOT removing from connected set - let gossip protocol handle reconnection");
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
                            println!("[IROH-STREAM] [OK] Successfully broadcast message to topic '{}'", topic);
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
            println!("[IROH-UNSUB] [OK] Unsubscribed from topic '{}'", topic);
            println!("[IROH-UNSUB]   Stream handler will terminate automatically");
            Ok(())
        } else {
            drop(topics_guard);
            Err(format!("Not subscribed to topic: {}", topic))
        }
    }

    /// Join gossip mesh with a specific bootstrap peer (ATOMIC - no message loss)
    ///
    /// This creates a NEW subscription with the peer as bootstrap, then atomically
    /// replaces the old subscription. Messages can still be published during the transition.
    ///
    /// NOTE: This function is currently unused. The gossip protocol naturally forms
    /// neighbor relationships through handle_connection(). Only use this if you need
    /// to force a specific peer as bootstrap (e.g., for initial network setup).
    /// Calling this repeatedly causes duplicate stream handlers and NeighborDown loops.
    #[allow(dead_code)]
    pub async fn join_gossip_mesh_with_peer(&self, bootstrap_peer: iroh::NodeId) -> Result<(), String> {
        println!("[IROH-JOIN] Joining gossip mesh with peer: {} (atomic)", bootstrap_peer);

        let gossip_guard = self.gossip.lock().await;
        let gossip = gossip_guard.as_ref().ok_or("Gossip not initialized")?;

        // Convert topic string to TopicId
        let topic_id = iroh_gossip::proto::TopicId::from(Self::topic_to_id(CONTENT_TOPIC));

        // Step 1: Create NEW subscription with bootstrap peer FIRST (before removing old one)
        // This ensures we always have an active subscription
        println!("[IROH-JOIN] Creating new subscription with peer {} as bootstrap...", bootstrap_peer);

        // Give gossip protocol time to use the QUIC connection we already have
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let gossip_topic = match tokio::time::timeout(
            tokio::time::Duration::from_secs(10),
            gossip.subscribe_and_join(topic_id, vec![bootstrap_peer])
        ).await {
            Ok(Ok(result)) => {
                println!("[IROH-JOIN] [OK] New subscription created successfully!");
                result
            }
            Ok(Err(e)) => {
                println!("[IROH-JOIN] Warning: subscribe_and_join failed: {}", e);
                // Keep old subscription, just return error
                return Err(format!("Failed to join with peer: {}", e));
            }
            Err(_) => {
                println!("[IROH-JOIN] Warning: subscribe_and_join timed out after 10s");
                // Keep old subscription, just return error
                return Err("Timeout joining gossip mesh".to_string());
            }
        };

        drop(gossip_guard);

        // Step 2: Split the new subscription
        let (gossip_sender, gossip_receiver) = gossip_topic.split();
        let (broadcast_tx, broadcast_rx) = tokio::sync::mpsc::unbounded_channel();
        let gossip_sender_arc = Arc::new(gossip_sender);

        // Step 3: ATOMIC swap - replace old subscription with new one
        println!("[IROH-JOIN] Atomically replacing old subscription...");
        {
            let mut topics_guard = self.topics.lock().await;
            // Remove old subscription (if any) - the old stream handler will terminate
            topics_guard.remove(CONTENT_TOPIC);
            // Insert new subscription
            topics_guard.insert(CONTENT_TOPIC.to_string(), TopicSubscription {
                broadcast_tx,
                gossip_sender: gossip_sender_arc.clone(),
            });
        }

        // Step 4: Start new message handler for the new subscription
        let network = Arc::new(self.clone_for_background());
        let topic_str = CONTENT_TOPIC.to_string();
        let gossip_sender_for_handler = gossip_sender_arc.clone();
        tokio::spawn(async move {
            network
                .handle_topic_stream(topic_str, gossip_sender_for_handler, gossip_receiver, broadcast_rx)
                .await;
        });

        self.add_connected_peer(bootstrap_peer).await;
        println!("[IROH-JOIN] [OK] Successfully joined gossip mesh with peer {}!", bootstrap_peer);
        Ok(())
    }

    /// Ensure global content topic is subscribed
    /// This is called by the commands module for backwards compatibility
    #[allow(dead_code)]
    pub async fn ensure_global_topic_subscribed(&self) -> Result<(), String> {
        // Check if already subscribed to global content topic
        if self.is_topic_subscribed(CONTENT_TOPIC).await {
            println!("[IROH] Global content topic already subscribed");
            return Ok(());
        }

        println!("[IROH] Subscribing to global content topic...");
        self.subscribe_topic(CONTENT_TOPIC).await?;
        println!("[IROH] [OK] Subscribed to global content topic");

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
        let data_size = data.len() as i64;
        println!("[IROH] Message serialized, size: {} bytes", data_size);

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

    // ============================================================================
    // BLOB TRANSFER FUNCTIONS
    // For large files (images, attachments) that exceed gossip message size limits
    // ============================================================================

    /// Store a blob locally and return its hash
    /// The hash can then be shared via gossip, and peers can fetch the blob directly
    pub async fn store_blob(&self, data: Vec<u8>) -> Result<iroh_blobs::Hash, String> {
        use iroh_blobs::store::Store;

        let blobs_guard = self.blobs.lock().await;
        let blobs = blobs_guard
            .as_ref()
            .ok_or("Blob store not initialized")?;

        let store = blobs.store();
        let bytes = Bytes::from(data);

        // Import the bytes into the store
        let tag = store
            .import_bytes(bytes, iroh_blobs::BlobFormat::Raw)
            .await
            .map_err(|e| format!("Failed to import blob: {}", e))?;

        let hash = *tag.hash();
        println!("[IROH-BLOBS] Stored blob with hash: {}", hash);

        Ok(hash)
    }

    /// Fetch a blob from a peer by hash
    /// Uses the existing NAT-traversed connection for efficient transfer
    pub async fn fetch_blob(
        &self,
        hash: iroh_blobs::Hash,
        from_node_id: iroh::NodeId,
    ) -> Result<Vec<u8>, String> {
        use iroh_blobs::store::bao_tree::io::fsm::AsyncSliceReader;
        use iroh_blobs::store::{Map, MapEntry};

        let blobs_guard = self.blobs.lock().await;
        let blobs = blobs_guard
            .as_ref()
            .ok_or("Blob store not initialized")?;

        println!("[IROH-BLOBS] Fetching blob {} from peer {}", hash, from_node_id);

        // Queue the download request
        let downloader = blobs.downloader();
        let request = iroh_blobs::downloader::DownloadRequest::new(
            iroh_blobs::HashAndFormat::raw(hash),
            vec![from_node_id],
        );

        // Start the download and wait for completion
        let handle = downloader.queue(request).await;
        handle
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        println!("[IROH-BLOBS] Download complete, reading blob from store");

        // Read the blob from local store
        let store = blobs.store();
        let entry = store
            .get(&hash)
            .await
            .map_err(|e| format!("Failed to get blob entry: {}", e))?
            .ok_or("Blob not found in store after download")?;

        let size = entry.size().value() as usize;

        // Use read_at to read all data at once (returns Send-safe future)
        let mut reader = entry
            .data_reader()
            .await
            .map_err(|e| format!("Failed to get data reader: {}", e))?;

        // Read all bytes using read_at (0 to size)
        let data = reader
            .read_at(0, size)
            .await
            .map_err(|e| format!("Failed to read blob: {}", e))?;

        println!("[IROH-BLOBS] [OK] Fetched blob, size: {} bytes (expected: {})", data.len(), size);
        Ok(data.to_vec())
    }

    /// Check if we have a blob locally
    pub async fn has_blob(&self, hash: iroh_blobs::Hash) -> bool {
        use iroh_blobs::store::Map;

        let blobs_guard = self.blobs.lock().await;
        if let Some(blobs) = blobs_guard.as_ref() {
            let store = blobs.store();
            store.get(&hash).await.ok().flatten().is_some()
        } else {
            false
        }
    }

    /// Get our node ID as a string for blob fetching
    pub async fn get_node_id(&self) -> String {
        let endpoint_guard = self.endpoint.lock().await;
        if let Some(endpoint) = endpoint_guard.as_ref() {
            endpoint.node_id().to_string()
        } else {
            String::new()
        }
    }

    /// Announce presence to the global mesh
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

        // Get current user profile data for signed presence
        let user = self
            .db
            .find_current_user_by_id(self.user_id)
            .map_err(|e| format!("Failed to get user: {}", e))?
            .ok_or_else(|| "User not found".to_string())?;

        let display_name = user.display_name.clone();
        let bio = user.bio.clone().unwrap_or_default();
        let profile_picture = user.profile_picture.clone().unwrap_or_default();

        // Sign profile data with private key for identity verification
        let profile_signature = if let Some(ref private_key) = user.private_key {
            match Database::sign_profile_data(private_key, &display_name, &bio, &profile_picture) {
                Ok(sig) => Some(sig),
                Err(e) => {
                    println!("[IROH] Warning: Failed to sign profile: {}", e);
                    None
                }
            }
        } else {
            println!("[IROH] Warning: No private key available for profile signing");
            None
        };

        let presence = P2PMessage::Presence {
            user_id: self.user_id,
            public_key: self.public_key.clone(),
            device_id: self
                .device_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            node_addr,
            timestamp: chrono::Utc::now().timestamp(),
            display_name,
            bio,
            profile_picture,
            profile_signature,
        };

        // Publish to global content topic - all nodes will see this
        self.publish_message(CONTENT_TOPIC, presence).await
    }

    /// Start background loop for presence announcements
    fn start_presence_loop(&self) {
        let network = Arc::new(self.clone_for_background());
        let shutdown_flag = self.shutdown_flag.clone();
        let last_success = self.last_presence_success.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;

                // Check if we should stop
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("[IROH] Presence loop stopping due to shutdown flag");
                    break;
                }

                match network.announce_presence().await {
                    Ok(_) => {
                        // Track successful presence announcement
                        let mut guard = last_success.lock().await;
                        *guard = Some(std::time::Instant::now());
                    }
                    Err(e) => {
                        println!("[IROH] Failed to announce presence: {}", e);
                    }
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
                    hex::encode(rendezvous_bytes)
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
                                    // CRITICAL: Skip already-connected peers!
                                    // Reconnecting to already-connected peers causes gossip state
                                    // to be reset, breaking message delivery.
                                    if network.is_peer_connected(peer_id).await {
                                        // Peer is already connected and in gossip mesh - skip
                                        continue;
                                    }

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
                                        match endpoint.connect(peer_id, iroh_gossip::ALPN).await {
                                            Ok(conn) => {
                                                println!(
                                                    "[IROH-DISCOVERY] [OK] Connected to peer {}!",
                                                    peer_id
                                                );
                                                println!(
                                                    "[IROH-DISCOVERY]   Connected to: {}",
                                                    peer_id
                                                );
                                                drop(endpoint_guard);

                                                // Check if already connected - skip if so
                                                let was_connected = network.is_peer_connected(peer_id).await;
                                                network.add_connected_peer(peer_id).await;

                                                // RESET backoff counter on successful connection
                                                let mut retry_states = network.peer_retry_counts.lock().await;
                                                if let Some(retry_state) = retry_states.get_mut(&peer_id) {
                                                    println!("[IROH-DISCOVERY] [OK] Backoff reset for peer {} after successful connection", peer_id);
                                                    retry_state.reset();
                                                }
                                                drop(retry_states);

                                                // Hand off the QUIC connection to gossip protocol.
                                                // This allows the gossip layer to use this connection
                                                // for neighbor communication.
                                                //
                                                // Two-step process for OUTGOING connections:
                                                // 1. handle_connection() - tells gossip about the connection
                                                // 2. join_peers() - actively joins the peer to our topic mesh
                                                //
                                                // handle_connection() alone only works for INCOMING connections.
                                                // For OUTGOING connections, we also need join_peers() to form
                                                // the gossip neighbor relationship.
                                                let gossip_guard = network.gossip.lock().await;
                                                if let Some(gossip) = gossip_guard.as_ref() {
                                                    match gossip.handle_connection(conn).await {
                                                        Ok(_) => println!("[IROH-DISCOVERY] [OK] Handed connection to gossip layer for peer {}", peer_id),
                                                        Err(e) => println!("[IROH-DISCOVERY] Warning: Failed to hand connection to gossip: {}", e),
                                                    }
                                                }
                                                drop(gossip_guard);

                                                // CRITICAL: Call join_peers() on the gossip sender to form mesh
                                                // This is required for OUTGOING connections to establish
                                                // a gossip neighbor relationship. Without this, messages
                                                // won't be exchanged even though QUIC is connected.
                                                let topics_guard = network.topics.lock().await;
                                                if let Some(subscription) = topics_guard.get(CONTENT_TOPIC) {
                                                    match subscription.gossip_sender.join_peers(vec![peer_id]).await {
                                                        Ok(_) => println!("[IROH-DISCOVERY] [OK] Joined gossip mesh with peer {}", peer_id),
                                                        Err(e) => println!("[IROH-DISCOVERY] Warning: Failed to join gossip mesh with peer: {}", e),
                                                    }
                                                }
                                                drop(topics_guard);

                                                if !was_connected {
                                                    println!("[IROH-DISCOVERY] NEW peer {} connected!", peer_id);

                                                    // RETRY pending messages when connection restored
                                                    println!("[IROH-DISCOVERY] Connection restored - attempting to resend pending messages...");
                                                    network.retry_pending_messages().await;

                                                    // CRITICAL FIX: Check if this peer initiated a friendship we've accepted
                                                    // If so, resend FriendAccepted to ensure they know we accepted
                                                    network.resend_friend_accepted_if_needed().await;
                                                } else {
                                                    println!("[IROH-DISCOVERY] Peer {} already tracked - refreshed gossip connection", peer_id);
                                                }

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
                display_name,
                bio,
                profile_picture,
                profile_signature,
            } => {
                let peer_node_id = node_addr.node_id;
                println!(
                    "[IROH] Received presence from user {} ({}) device {} (NodeId: {})",
                    display_name, &public_key[..8], device_id, peer_node_id
                );
                println!(
                    "[IROH]   Relay: {:?}, Direct addresses: {}",
                    node_addr.relay_url(),
                    node_addr.direct_addresses().count()
                );

                // SECURITY: Verify profile signature if present
                let signature_valid = if let Some(ref sig) = profile_signature {
                    let valid = Database::verify_profile_signature(
                        &public_key,
                        &display_name,
                        &bio,
                        &profile_picture,
                        sig,
                    );
                    if valid {
                        println!("[IROH] [SECURITY] Profile signature VERIFIED for {}", display_name);
                    } else {
                        println!("[IROH] [SECURITY] WARNING: Profile signature INVALID for {} - possible tampering!", display_name);
                    }
                    valid
                } else {
                    println!("[IROH] [SECURITY] No profile signature provided by {}", display_name);
                    false
                };

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

                // CLEANUP: Remove stale device entries for this user
                // This handles the case where a device was wiped and got a new device_id/node_id
                // The old device entry with the old node_id would otherwise stay in our database forever
                match self.db.cleanup_stale_devices_for_user(&public_key, &device_id, &node_id_str) {
                    Ok(stale_node_ids) if !stale_node_ids.is_empty() => {
                        println!(
                            "[IROH] Cleaned up {} stale device entries for user {}",
                            stale_node_ids.len(),
                            &public_key[..8]
                        );
                        // Also remove stale node_ids from connected_peers
                        for stale_node_id in stale_node_ids {
                            if let Ok(stale_id) = stale_node_id.parse::<iroh::NodeId>() {
                                self.remove_connected_peer(stale_id).await;
                                println!("[IROH] Removed stale peer {} from connected set", stale_node_id);
                            }
                        }
                    }
                    Ok(_) => {} // No stale entries
                    Err(e) => println!("[IROH] Warning: Failed to cleanup stale devices: {}", e),
                }

                // NOTE: We received this Presence message via gossip, which means we ALREADY have
                // a working gossip connection to this peer. Calling endpoint.connect() would be
                // redundant and might interfere with the gossip protocol's connection management.
                //
                // Instead, we just:
                // 1. Add the peer's node address to the endpoint (for better routing info)
                // 2. Track the peer as connected (since we received their presence via gossip)
                // 3. Store their info in the database for future reconnection

                // Add node address for better routing info
                let endpoint_guard = self.endpoint.lock().await;
                if let Some(endpoint) = endpoint_guard.as_ref() {
                    if let Err(e) = endpoint.add_node_addr(node_addr.clone()) {
                        println!("[IROH] Warning: Failed to add node address: {}", e);
                    } else {
                        println!("[IROH] [OK] Added peer's node address to endpoint");
                    }
                }
                drop(endpoint_guard);

                // Track this peer as connected (we received their presence via gossip!)
                self.add_connected_peer(peer_node_id).await;
                println!("[IROH] [OK] Peer {} is connected via gossip mesh (received their presence)", peer_node_id);

                // Create friendship in database (CRITICAL for friends list to work!)
                // Skip if this is the same user (different device)
                if public_key != self.public_key {
                    println!(
                        "[IROH] Creating/updating friendship with peer user {}",
                        peer_user_id
                    );

                    // First, ensure the peer user exists in our database with their profile data
                    if let Err(e) = self.db.conn.lock().unwrap().execute(
                        "INSERT INTO users (id, display_name, public_key, bio, profile_picture, profile_signature, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                         ON CONFLICT(public_key) DO UPDATE SET
                            display_name = excluded.display_name,
                            bio = excluded.bio,
                            profile_picture = excluded.profile_picture,
                            profile_signature = excluded.profile_signature,
                            updated_at = excluded.updated_at",
                        rusqlite::params![
                            peer_user_id,
                            &display_name,
                            &public_key,
                            &bio,
                            &profile_picture,
                            &profile_signature,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to update peer user: {}", e);
                    }

                    // SECURITY: Check for display name changes on existing friends
                    // Store known_display_name when first becoming friends
                    // Warn if display name changes with valid signature (legitimate update)
                    // or especially if signature is invalid (potential impersonation)
                    if let Ok((known_name, _stored_sig)) = self.db.conn.lock().unwrap()
                        .query_row(
                            "SELECT known_display_name, friend_profile_signature FROM p2p_connections
                             WHERE user_id = ?1 AND friend_user_id = ?2 AND status = 'accepted'",
                            rusqlite::params![self.user_id, peer_user_id],
                            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?))
                        )
                    {
                        if let Some(known) = known_name {
                            if known != display_name {
                                if signature_valid {
                                    println!("[IROH] [SECURITY] Friend {} changed name from '{}' to '{}' (signature valid)",
                                        &public_key[..8], known, display_name);
                                    // Emit event to frontend - legitimate name change with valid signature
                                    let _ = self.app_handle.emit("friend-name-changed", serde_json::json!({
                                        "publicKey": &public_key,
                                        "oldName": &known,
                                        "newName": &display_name,
                                        "signatureValid": true,
                                        "warning": false
                                    }));
                                } else {
                                    println!("[IROH] [SECURITY] WARNING: Friend {} changed name from '{}' to '{}' but signature is INVALID!",
                                        &public_key[..8], known, display_name);
                                    // Emit warning event to frontend - potential impersonation attempt!
                                    let _ = self.app_handle.emit("friend-name-changed", serde_json::json!({
                                        "publicKey": &public_key,
                                        "oldName": &known,
                                        "newName": &display_name,
                                        "signatureValid": false,
                                        "warning": true,
                                        "message": "This friend's display name changed but their signature is invalid. This could indicate tampering or impersonation."
                                    }));
                                }
                            }
                        }
                        // Update stored signature if we have a valid new one
                        if signature_valid {
                            let _ = self.db.conn.lock().unwrap().execute(
                                "UPDATE p2p_connections SET friend_profile_signature = ?1
                                 WHERE user_id = ?2 AND friend_user_id = ?3",
                                rusqlite::params![&profile_signature, self.user_id, peer_user_id],
                            );
                        }
                    }

                    // Only update peer address for EXISTING accepted friendships
                    // Do NOT auto-create friendships - that should happen via FriendRequest flow
                    let existing_status: Option<String> = self.db.conn.lock().unwrap()
                        .query_row(
                            "SELECT status FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
                            rusqlite::params![self.user_id, peer_user_id],
                            |row| row.get(0)
                        )
                        .ok();

                    if let Some(status) = existing_status {
                        println!("[IROH] Existing connection with peer {} has status: {}", peer_user_id, status);

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
                                println!("[IROH] [OK] Friend peer address saved for reconnection: NodeId={}, Relay={}", node_id_str, relay_url_str);
                            }
                        }

                        // CRITICAL FIX: If we have accepted this peer's friend request, resend FriendAccepted
                        // This handles the case where our original FriendAccepted was lost due to gossip mesh instability
                        if status == "accepted" {
                            // Check if this peer initiated the connection (they sent the friend request to us)
                            let initiated_by: Option<super::types::SqliteUuid> = self.db.conn.lock().unwrap()
                                .query_row(
                                    "SELECT initiated_by FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
                                    rusqlite::params![self.user_id, peer_user_id],
                                    |row| row.get(0)
                                )
                                .ok();

                            // If peer_user_id initiated the connection, they are waiting for our FriendAccepted
                            if initiated_by == Some(peer_user_id) {
                                println!("[IROH] Peer {} initiated this friendship - resending FriendAccepted to ensure delivery", peer_user_id);

                                // Get our node address for the FriendAccepted message
                                let endpoint_guard = self.endpoint.lock().await;
                                let (our_node_id, our_relay_url) = if let Some(endpoint) = endpoint_guard.as_ref() {
                                    match endpoint.node_addr().await {
                                        Ok(addr) => (
                                            addr.node_id.to_string(),
                                            addr.relay_url().map(|u| u.to_string()).unwrap_or_default()
                                        ),
                                        Err(_) => (String::new(), String::new())
                                    }
                                } else {
                                    (String::new(), String::new())
                                };
                                drop(endpoint_guard);

                                if !our_node_id.is_empty() {
                                    let friend_accepted = P2PMessage::FriendAccepted {
                                        from_user_id: self.user_id,
                                        from_public_key: self.public_key.clone(),
                                        from_display_name: self.display_name.clone(),
                                        from_node_id: our_node_id,
                                        from_relay_url: our_relay_url,
                                        to_public_key: public_key.clone(),
                                    };

                                    if let Err(e) = self.publish_message(CONTENT_TOPIC, friend_accepted).await {
                                        println!("[IROH] Warning: Failed to resend FriendAccepted: {}", e);
                                    } else {
                                        println!("[IROH] [OK] Resent FriendAccepted to {} (ensuring they know we accepted)", public_key);
                                    }
                                }
                            }
                        }

                        // CRITICAL FIX: If we have a pending OUTGOING request to this peer, resend FriendRequest
                        // This handles the case where their app data was cleared and they lost our original request
                        if status == "pending" {
                            // Check if WE initiated the connection (we sent the friend request to them)
                            let initiated_by: Option<super::types::SqliteUuid> = self.db.conn.lock().unwrap()
                                .query_row(
                                    "SELECT initiated_by FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
                                    rusqlite::params![self.user_id, peer_user_id],
                                    |row| row.get(0)
                                )
                                .ok();

                            // If WE initiated the connection, they may have lost our FriendRequest - resend it
                            if initiated_by == Some(self.user_id) {
                                println!("[IROH] We initiated friendship with {} - resending FriendRequest to ensure delivery", peer_user_id);

                                // Get our node address for the FriendRequest message
                                let endpoint_guard = self.endpoint.lock().await;
                                let (our_node_id, our_relay_url) = if let Some(endpoint) = endpoint_guard.as_ref() {
                                    match endpoint.node_addr().await {
                                        Ok(addr) => (
                                            addr.node_id.to_string(),
                                            addr.relay_url().map(|u| u.to_string()).unwrap_or_default()
                                        ),
                                        Err(_) => (String::new(), String::new())
                                    }
                                } else {
                                    (String::new(), String::new())
                                };
                                drop(endpoint_guard);

                                if !our_node_id.is_empty() {
                                    let friend_request = P2PMessage::FriendRequest {
                                        from_user_id: self.user_id,
                                        from_public_key: self.public_key.clone(),
                                        from_display_name: self.display_name.clone(),
                                        from_node_id: our_node_id,
                                        from_relay_url: our_relay_url,
                                        to_public_key: public_key.clone(),
                                        timestamp: chrono::Utc::now().timestamp(),
                                    };

                                    if let Err(e) = self.publish_message(CONTENT_TOPIC, friend_request).await {
                                        println!("[IROH] Warning: Failed to resend FriendRequest: {}", e);
                                    } else {
                                        println!("[IROH] [OK] Resent FriendRequest to {} (in case they lost our original request)", public_key);
                                    }
                                }
                            }
                        }
                    } else {
                        // No existing connection - this peer needs to send a FriendRequest first
                        println!("[IROH] No existing connection with peer {} - waiting for FriendRequest", peer_user_id);
                    }
                }

                // GLOBAL MESH: No topic subscriptions needed in message handler
                // All nodes are on the same cipher/content/v1 topic
                // Presence is just for peer discovery and connection establishment
                println!("[IROH] Presence processed - global mesh handles all routing");

                // Check if this is another device with the same user account
                if public_key == self.public_key
                    && device_id != self.device_id.clone().unwrap_or_default()
                {
                    println!(
                        "[IROH] SAME-USER DEVICE DETECTED: {} with NodeId: {}",
                        device_id, peer_node_id
                    );

                    // Send device sync request via global mesh
                    let sync_request = P2PMessage::DeviceSyncRequest {
                        public_key: self.public_key.clone(),
                        device_id: self
                            .device_id
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string()),
                        last_sync_timestamp: 0, // Get all data for now
                    };

                    if let Err(e) = self.publish_message(CONTENT_TOPIC, sync_request).await {
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

                                    // Publish to global mesh - target device will filter by public_key
                                    if let Err(e) = self.publish_message(CONTENT_TOPIC, response).await {
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
                ref public_key,
                ref content,
                timestamp,
                ref device_id,
                ref attachments,
            } => {
                println!("[IROH] Received post from user {} ({})", user_id, public_key);

                // Skip our own posts (they're already in DB)
                if public_key == &self.public_key {
                    println!("[IROH] Skipping own post");
                    return;
                }

                // CRITICAL: Ensure the sender's user record exists before saving post
                // This prevents foreign key constraint violations in the posts table
                if let Err(e) = self.db.conn.lock().unwrap().execute(
                    "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        user_id,
                        format!("User_{}", &public_key[..8.min(public_key.len())]),
                        public_key,
                        chrono::Utc::now().to_rfc3339(),
                        chrono::Utc::now().to_rfc3339()
                    ],
                ) {
                    println!("[IROH] Warning: Failed to ensure post sender exists: {}", e);
                } else {
                    println!("[IROH] [OK] Ensured user record exists for post sender");
                }

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
                            public_key: public_key.clone(),
                            content: content.clone(),
                            timestamp,
                            device_id: device_id.clone(),
                            attachments: attachments.clone(),
                        },
                    },
                );
            }

            P2PMessage::PostWithBlobs {
                user_id,
                ref public_key,
                ref node_id,
                ref content,
                timestamp,
                ref device_id,
                ref blob_refs,
            } => {
                println!(
                    "[IROH] Received PostWithBlobs from user {} ({}) with {} blob refs",
                    user_id, public_key, blob_refs.len()
                );

                // Skip our own posts (they're already in DB)
                if public_key == &self.public_key {
                    println!("[IROH] Skipping own post with blobs");
                    return;
                }

                // CRITICAL: Ensure the sender's user record exists before saving post
                if let Err(e) = self.db.conn.lock().unwrap().execute(
                    "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        user_id,
                        format!("User_{}", &public_key[..8.min(public_key.len())]),
                        public_key,
                        chrono::Utc::now().to_rfc3339(),
                        chrono::Utc::now().to_rfc3339()
                    ],
                ) {
                    println!("[IROH] Warning: Failed to ensure post sender exists: {}", e);
                } else {
                    println!("[IROH] [OK] Ensured user record exists for post sender");
                }

                // Parse sender's NodeId for blob fetching
                let sender_node_id = match node_id.parse::<iroh::NodeId>() {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IROH] Failed to parse sender NodeId '{}': {}", node_id, e);
                        // Emit post without attachments
                        let _ = self.app_handle.emit(
                            "p2p-message-received",
                            serde_json::json!({
                                "message": {
                                    "type": "Post",
                                    "user_id": user_id,
                                    "public_key": public_key,
                                    "content": content,
                                    "timestamp": timestamp,
                                    "device_id": device_id,
                                    "attachments": []
                                }
                            }),
                        );
                        return;
                    }
                };

                // Download blobs and wait for completion before emitting to UI
                // This ensures the frontend can read the blobs immediately
                let mut downloaded_blob_refs = blob_refs.clone();

                let blobs_guard = self.blobs.lock().await;
                if let Some(blobs) = blobs_guard.as_ref() {
                    let downloader = blobs.downloader();

                    for (idx, blob_ref) in blob_refs.iter().enumerate() {
                        println!(
                            "[IROH] Downloading blob {} ({} bytes) from {}",
                            blob_ref.blob_hash, blob_ref.file_size, node_id
                        );

                        // Parse blob hash
                        let hash = match hex::decode(&blob_ref.blob_hash) {
                            Ok(bytes) if bytes.len() == 32 => {
                                let mut arr = [0u8; 32];
                                arr.copy_from_slice(&bytes);
                                iroh_blobs::Hash::from_bytes(arr)
                            }
                            Ok(_) => {
                                println!("[IROH] Invalid blob hash length for {}", blob_ref.blob_hash);
                                continue;
                            }
                            Err(e) => {
                                println!("[IROH] Failed to decode blob hash: {}", e);
                                continue;
                            }
                        };

                        // Queue the download and WAIT for it to complete
                        let request = iroh_blobs::downloader::DownloadRequest::new(
                            iroh_blobs::HashAndFormat::raw(hash),
                            vec![sender_node_id],
                        );
                        let handle = downloader.queue(request).await;

                        // Wait for download to complete
                        match handle.await {
                            Ok(stats) => {
                                println!(
                                    "[IROH] Blob {} downloaded successfully ({} bytes received)",
                                    blob_ref.blob_hash, stats.bytes_read
                                );
                                // Mark as downloaded
                                downloaded_blob_refs[idx].downloaded = true;
                            }
                            Err(e) => {
                                println!("[IROH] Failed to download blob {}: {}", blob_ref.blob_hash, e);
                                // Keep downloaded = false (default)
                            }
                        }
                    }
                }
                drop(blobs_guard);

                // Emit PostWithBlobs to UI with download status
                // Frontend will only try to read blobs marked as downloaded
                #[derive(serde::Serialize, Clone)]
                struct MessageEvent {
                    message: P2PMessage,
                }

                let _ = self.app_handle.emit(
                    "p2p-message-received",
                    MessageEvent {
                        message: P2PMessage::PostWithBlobs {
                            user_id,
                            public_key: public_key.clone(),
                            node_id: node_id.clone(),
                            content: content.clone(),
                            timestamp,
                            device_id: device_id.clone(),
                            blob_refs: downloaded_blob_refs,
                        },
                    },
                );
            }

            P2PMessage::SealedEnvelope { envelope_json } => {
                // PHASE 2: Sealed box encryption
                // Try to decrypt the envelope using our encryption private key
                println!("[IROH] Received SealedEnvelope - attempting to decrypt...");

                // Get our encryption private key from database
                let our_encryption_private_key = match self.get_user_encryption_private_key() {
                    Some(key) => key,
                    None => {
                        println!("[IROH] No encryption private key found, cannot decrypt");
                        return;
                    }
                };

                // Parse the envelope
                let envelope: super::crypto::GossipEnvelope = match serde_json::from_str(&envelope_json) {
                    Ok(env) => env,
                    Err(e) => {
                        println!("[IROH] Failed to parse envelope: {}", e);
                        return;
                    }
                };

                // Quick check: does this envelope have any boxes for us?
                let our_encryption_public_key = match self.get_user_encryption_public_key() {
                    Some(key) => key,
                    None => {
                        println!("[IROH] No encryption public key found");
                        return;
                    }
                };

                if !envelope.might_be_for_us(&our_encryption_public_key) {
                    // Not for us - but cached for relay
                    println!("[IROH] Envelope not for us, cached for relay");
                    return;
                }

                // Try to decrypt
                match envelope.try_decrypt(&our_encryption_public_key, &our_encryption_private_key) {
                    Some(payload) => {
                        println!("[IROH] [OK] Successfully decrypted envelope from {}", envelope.sender_public_key);

                        // Process the decrypted content
                        match payload {
                            super::crypto::ContentPayload::Post { content, attachments } => {
                                // Get sender's user_id from their public key
                                let sender_user_id = super::types::SqliteUuid::from_public_key(&envelope.sender_public_key);

                                // Ensure sender exists in database
                                if let Err(e) = self.db.conn.lock().unwrap().execute(
                                    "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5)",
                                    rusqlite::params![
                                        sender_user_id,
                                        format!("User_{}", &envelope.sender_public_key[..8.min(envelope.sender_public_key.len())]),
                                        &envelope.sender_public_key,
                                        chrono::Utc::now().to_rfc3339(),
                                        chrono::Utc::now().to_rfc3339()
                                    ],
                                ) {
                                    println!("[IROH] Warning: Failed to ensure post sender exists: {}", e);
                                }

                                // Emit decrypted post to UI
                                #[derive(serde::Serialize, Clone)]
                                struct DecryptedPostEvent {
                                    user_id: super::types::SqliteUuid,
                                    public_key: String,
                                    content: String,
                                    timestamp: i64,
                                    attachments: Option<Vec<super::types::MediaAttachmentWithData>>,
                                }

                                let _ = self.app_handle.emit(
                                    "sealed-post-received",
                                    DecryptedPostEvent {
                                        user_id: sender_user_id,
                                        public_key: envelope.sender_public_key.clone(),
                                        content,
                                        timestamp: envelope.timestamp,
                                        attachments,
                                    },
                                );
                                println!("[IROH] [OK] Emitted decrypted post to UI");
                            }
                            super::crypto::ContentPayload::DirectMessage { content, thread_id } => {
                                let sender_user_id = super::types::SqliteUuid::from_public_key(&envelope.sender_public_key);
                                println!("[IROH] Received encrypted DM from {}: {} chars",
                                    envelope.sender_public_key, content.len());

                                #[derive(serde::Serialize, Clone)]
                                struct DecryptedDMEvent {
                                    from_user_id: super::types::SqliteUuid,
                                    from_public_key: String,
                                    content: String,
                                    thread_id: Option<super::types::SqliteUuid>,
                                    timestamp: i64,
                                }

                                let _ = self.app_handle.emit(
                                    "sealed-dm-received",
                                    DecryptedDMEvent {
                                        from_user_id: sender_user_id,
                                        from_public_key: envelope.sender_public_key.clone(),
                                        content,
                                        thread_id,
                                        timestamp: envelope.timestamp,
                                    },
                                );
                            }
                            super::crypto::ContentPayload::CommunityPost {
                                community_id,
                                community_name,
                                content,
                                attachments,
                                show_in_main_feed,
                            } => {
                                let sender_user_id = super::types::SqliteUuid::from_public_key(&envelope.sender_public_key);
                                println!("[IROH] Received community post in '{}' from {}: {} chars",
                                    community_name, envelope.sender_public_key, content.len());

                                // Parse community_id from string
                                let community_uuid = match super::types::SqliteUuid::parse_str(&community_id) {
                                    Ok(id) => id,
                                    Err(e) => {
                                        println!("[IROH] Failed to parse community_id: {}", e);
                                        return;
                                    }
                                };

                                // Ensure sender exists in database
                                if let Err(e) = self.db.conn.lock().unwrap().execute(
                                    "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5)",
                                    rusqlite::params![
                                        sender_user_id,
                                        format!("User_{}", &envelope.sender_public_key[..8.min(envelope.sender_public_key.len())]),
                                        &envelope.sender_public_key,
                                        chrono::Utc::now().to_rfc3339(),
                                        chrono::Utc::now().to_rfc3339()
                                    ],
                                ) {
                                    println!("[IROH] Warning: Failed to ensure community post sender exists: {}", e);
                                }

                                // Create the post in database
                                match self.db.create_post(sender_user_id, &content, false) {
                                    Ok(post) => {
                                        // Link it to the community
                                        if let Err(e) = self.db.create_community_post(community_uuid, post.id, show_in_main_feed) {
                                            println!("[IROH] Warning: Failed to link post to community: {}", e);
                                        }

                                        // Emit to UI
                                        #[derive(serde::Serialize, Clone)]
                                        struct CommunityPostEvent {
                                            community_id: String,
                                            community_name: String,
                                            post_id: super::types::SqliteUuid,
                                            user_id: super::types::SqliteUuid,
                                            public_key: String,
                                            content: String,
                                            show_in_main_feed: bool,
                                            timestamp: i64,
                                            attachments: Option<Vec<super::types::MediaAttachmentWithData>>,
                                        }

                                        let _ = self.app_handle.emit(
                                            "community-post-received",
                                            CommunityPostEvent {
                                                community_id: community_id.clone(),
                                                community_name: community_name.clone(),
                                                post_id: post.id,
                                                user_id: sender_user_id,
                                                public_key: envelope.sender_public_key.clone(),
                                                content,
                                                show_in_main_feed,
                                                timestamp: envelope.timestamp,
                                                attachments,
                                            },
                                        );
                                        println!("[IROH] [OK] Stored and emitted community post");
                                    }
                                    Err(e) => {
                                        println!("[IROH] Failed to create community post: {}", e);
                                    }
                                }
                            }
                            super::crypto::ContentPayload::CommunityMemberAdded {
                                community_id,
                                community_name,
                                new_member_public_key,
                                new_member_display_name,
                            } => {
                                println!("[IROH] Received community member added: {} joined '{}'",
                                    new_member_display_name, community_name);

                                // Parse community_id from string
                                let community_uuid = match super::types::SqliteUuid::parse_str(&community_id) {
                                    Ok(id) => id,
                                    Err(e) => {
                                        println!("[IROH] Failed to parse community_id: {}", e);
                                        return;
                                    }
                                };

                                // Get or create user_id for the new member
                                let new_member_user_id = super::types::SqliteUuid::from_public_key(&new_member_public_key);

                                // Ensure the new member user exists in database
                                if let Err(e) = self.db.conn.lock().unwrap().execute(
                                    "INSERT OR IGNORE INTO users (id, display_name, public_key, encryption_public_key, created_at, updated_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                    rusqlite::params![
                                        new_member_user_id,
                                        &new_member_display_name,
                                        &new_member_public_key,
                                        &new_member_public_key,
                                        chrono::Utc::now().to_rfc3339(),
                                        chrono::Utc::now().to_rfc3339()
                                    ],
                                ) {
                                    println!("[IROH] Warning: Failed to ensure new member exists: {}", e);
                                }

                                // Add member to community (ignore if already exists)
                                if let Err(e) = self.db.add_community_member(
                                    community_uuid,
                                    new_member_user_id,
                                    &new_member_public_key,
                                    Some(&new_member_display_name),
                                    None, // invited_by not available in this message
                                ) {
                                    // Ignore duplicate errors
                                    if !e.to_string().contains("UNIQUE constraint failed") {
                                        println!("[IROH] Warning: Failed to add community member: {}", e);
                                    }
                                }

                                // Emit to UI
                                #[derive(serde::Serialize, Clone)]
                                struct CommunityMemberAddedEvent {
                                    community_id: String,
                                    community_name: String,
                                    new_member_user_id: super::types::SqliteUuid,
                                    new_member_public_key: String,
                                    new_member_display_name: String,
                                }

                                let _ = self.app_handle.emit(
                                    "community-member-added",
                                    CommunityMemberAddedEvent {
                                        community_id,
                                        community_name,
                                        new_member_user_id,
                                        new_member_public_key,
                                        new_member_display_name,
                                    },
                                );
                                println!("[IROH] [OK] Community member added and emitted");
                            }
                            _ => {
                                println!("[IROH] Received other sealed content type");
                            }
                        }
                    }
                    None => {
                        // Couldn't decrypt - this shouldn't happen if might_be_for_us returned true
                        println!("[IROH] Failed to decrypt envelope (hint matched but decryption failed)");
                    }
                }
            }

            P2PMessage::FriendRequest {
                from_public_key,
                from_user_id,
                from_display_name,
                from_node_id,
                from_relay_url,
                to_public_key,
                timestamp: _,
            } => {
                println!("[IROH] Received FriendRequest from {} ({}) (global mesh)", from_display_name, from_public_key);

                // GLOBAL MESH: Filter messages - only process if intended for us
                if to_public_key != self.public_key {
                    // Not for us - ignore (in global mesh, everyone sees everything)
                    return;
                }

                // Skip if trying to add ourselves
                if from_public_key == self.public_key {
                    println!("[IROH] Cannot add ourselves as friend, ignoring");
                    return;
                }

                println!("[IROH] Processing friend request from {} ({})", from_display_name, from_public_key);

                // 1. Ensure the friend user exists in database with their actual display name
                // Use INSERT OR REPLACE to update display name if user already exists with stub name
                if let Err(e) = self.db.conn.lock().unwrap().execute(
                    "INSERT INTO users (id, display_name, public_key, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET display_name = excluded.display_name, updated_at = excluded.updated_at",
                    rusqlite::params![
                        from_user_id,
                        &from_display_name,
                        &from_public_key,
                        chrono::Utc::now().to_rfc3339(),
                        chrono::Utc::now().to_rfc3339()
                    ],
                ) {
                    println!("[IROH] Warning: Failed to create/update friend user: {}", e);
                }

                // 2. Create incoming friend request in database (status = 'pending')
                // User must accept this request before friendship is established
                if let Err(e) = self.db.conn.lock().unwrap().execute(
                    "INSERT OR IGNORE INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at, iroh_node_id, friend_relay_url)
                     VALUES (?1, ?2, ?3, 'pending', ?3, ?4, ?5, ?6, ?7)",
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
                    println!("[IROH] Warning: Failed to create friend request: {}", e);
                } else {
                    println!("[IROH] [OK] Incoming friend request created from {}", from_public_key);
                }

                // Emit event to UI so it can show the pending request
                let _ = self.app_handle.emit("friend-request-received", serde_json::json!({
                    "from_user_id": from_user_id.to_string(),
                    "from_public_key": from_public_key,
                    "from_node_id": from_node_id,
                }));

                // 3. Add friend's node address to endpoint for direct connection
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
                                println!("[IROH] [OK] Added friend's node address to endpoint");
                            }
                        }
                        drop(endpoint_guard);

                        // NOTE: We do NOT resubscribe here because if we received this message via gossip,
                        // the mesh is already working. Resubscribing would disrupt the mesh and cause
                        // message loss. The sender will resubscribe on their end when they join with us
                        // as bootstrap, which is sufficient for bidirectional connectivity.
                        println!("[IROH] Gossip mesh already functional (received message), skipping resubscription");
                    }
                }

                println!("[IROH] [OK] Friend request received, waiting for user to accept");
            }

            P2PMessage::FriendAccepted {
                from_user_id: _from_user_id, // Unused - we compute deterministically from public key
                from_public_key,
                from_display_name,
                from_node_id,
                from_relay_url,
                to_public_key,
            } => {
                println!("[IROH] Received FriendAccepted from {} ({}) (global mesh)", from_display_name, from_public_key);

                // GLOBAL MESH: Filter messages - only process if intended for us
                if to_public_key != self.public_key {
                    // Not for us - ignore (in global mesh, everyone sees everything)
                    return;
                }

                // Skip if from ourselves
                if from_public_key == self.public_key {
                    println!("[IROH] FriendAccepted from ourselves, ignoring");
                    return;
                }

                // CRITICAL: Compute friend_user_id deterministically from public key
                // This ensures it matches exactly how it was stored when the outgoing request was created
                let friend_user_id = super::types::SqliteUuid::from_public_key(&from_public_key);
                println!("[IROH] Computed friend_user_id from public key: {}", friend_user_id);

                // Update the friend's display name in case they have a real name now (was previously a stub)
                if let Err(e) = self.db.conn.lock().unwrap().execute(
                    "UPDATE users SET display_name = ?1, updated_at = ?2 WHERE id = ?3",
                    rusqlite::params![
                        &from_display_name,
                        chrono::Utc::now().to_rfc3339(),
                        friend_user_id,
                    ],
                ) {
                    println!("[IROH] Warning: Failed to update friend display name: {}", e);
                } else {
                    println!("[IROH] Updated friend display name to {}", from_display_name);
                }

                // Update our pending outgoing request to accepted
                // Also save their node_id and relay_url for reconnection
                // Use a scope block to ensure MutexGuard is dropped before any await calls
                {
                    let now = chrono::Utc::now().to_rfc3339();
                    let conn = self.db.conn.lock().unwrap();
                    println!("[IROH] Updating p2p_connections: user_id={}, friend_user_id={}", self.user_id, friend_user_id);

                    // First try to update existing pending request
                    let rows_affected = match conn.execute(
                        "UPDATE p2p_connections SET status = 'accepted', updated_at = ?1, iroh_node_id = ?4, friend_relay_url = ?5
                         WHERE user_id = ?2 AND friend_user_id = ?3 AND status = 'pending'",
                        rusqlite::params![
                            &now,
                            self.user_id,
                            friend_user_id,
                            &from_node_id,
                            &from_relay_url,
                        ],
                    ) {
                        Ok(n) => n,
                        Err(e) => {
                            println!("[IROH] Warning: Failed to update friend request to accepted: {}", e);
                            0
                        }
                    };

                    if rows_affected > 0 {
                        println!("[IROH] [OK] Friend request accepted by {}, {} row(s) updated", from_public_key, rows_affected);
                    } else {
                        // No pending request found - check if already accepted or need to create
                        let existing_status: Option<String> = conn.query_row(
                            "SELECT status FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
                            rusqlite::params![self.user_id, friend_user_id],
                            |row| row.get(0)
                        ).ok();

                        match existing_status.as_deref() {
                            Some("accepted") => {
                                println!("[IROH] Friendship already accepted, updating node info");
                                // Update node info even if already accepted
                                let _ = conn.execute(
                                    "UPDATE p2p_connections SET iroh_node_id = ?1, friend_relay_url = ?2, updated_at = ?3
                                     WHERE user_id = ?4 AND friend_user_id = ?5",
                                    rusqlite::params![&from_node_id, &from_relay_url, &now, self.user_id, friend_user_id],
                                );
                            }
                            Some(status) => {
                                println!("[IROH] Unexpected status '{}', forcing to accepted", status);
                                let _ = conn.execute(
                                    "UPDATE p2p_connections SET status = 'accepted', iroh_node_id = ?1, friend_relay_url = ?2, updated_at = ?3
                                     WHERE user_id = ?4 AND friend_user_id = ?5",
                                    rusqlite::params![&from_node_id, &from_relay_url, &now, self.user_id, friend_user_id],
                                );
                            }
                            None => {
                                // CRITICAL FIX: No connection exists - create the user and friendship directly
                                // This handles the case where our data was cleared or we never received the original FriendRequest
                                println!("[IROH] No existing connection - creating accepted friendship directly from FriendAccepted");

                                // First ensure the user record exists
                                if let Err(e) = conn.execute(
                                    "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                                     VALUES (?1, ?2, ?3, ?4, ?5)",
                                    rusqlite::params![
                                        friend_user_id,
                                        &from_display_name,
                                        &from_public_key,
                                        &now,
                                        &now,
                                    ],
                                ) {
                                    println!("[IROH] Warning: Failed to create user record: {}", e);
                                }

                                // Create the p2p_connection as already accepted
                                // initiated_by is set to friend_user_id because THEY initiated by accepting (sending FriendAccepted)
                                if let Err(e) = conn.execute(
                                    "INSERT INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, iroh_node_id, friend_relay_url, created_at, updated_at)
                                     VALUES (?1, ?2, ?3, 'accepted', ?4, ?5, ?6, ?7, ?8)",
                                    rusqlite::params![
                                        super::types::SqliteUuid::new(),
                                        self.user_id,
                                        friend_user_id,
                                        friend_user_id, // They initiated (we're receiving their acceptance)
                                        &from_node_id,
                                        &from_relay_url,
                                        &now,
                                        &now,
                                    ],
                                ) {
                                    println!("[IROH] Warning: Failed to create accepted friendship: {}", e);
                                } else {
                                    println!("[IROH] [OK] Created accepted friendship directly from FriendAccepted message");
                                }
                            }
                        }
                    }
                } // conn is dropped here, before any await calls

                // Emit event to UI so it can refresh the friends list
                let _ = self.app_handle.emit("friend-accepted", serde_json::json!({
                    "from_user_id": friend_user_id.to_string(),
                    "from_public_key": from_public_key,
                }));

                // Add their node address to endpoint for direct connection
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
                                println!("[IROH] [OK] Added friend's node address to endpoint");
                            }
                        }
                        drop(endpoint_guard);

                        // NOTE: No resubscription needed! The gossip protocol handles mesh formation
                        // automatically. Since we received this FriendAccepted message via gossip,
                        // we already have a working gossip connection to the peer.
                        // Resubscribing would tear down that connection and cause message loss.
                        println!("[IROH] [OK] Friend's node address added - gossip mesh already established");
                        self.add_connected_peer(peer_node_id).await;
                    }
                }

                println!("[IROH] [OK] Friendship fully established with {}!", from_public_key);
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

                            // Try to resend via global mesh
                            match serde_json::from_str::<P2PMessage>(&pending_msg.content_json) {
                                Ok(message) => {
                                    match self.publish_message(CONTENT_TOPIC, message).await {
                                        Ok(_) => {
                                            println!(
                                                "[QUEUE] [OK] Successfully resent message (ID: {})",
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

    /// Resend FriendAccepted for any accepted connections where the peer initiated
    /// This handles the case where our original FriendAccepted was lost due to gossip mesh instability
    pub async fn resend_friend_accepted_if_needed(&self) {
        println!("[FRIEND-RESEND] Checking for accepted friendships that need FriendAccepted resend...");

        // Query for connections where status='accepted' and initiated_by != our user_id
        // These are connections where someone else sent us a friend request and we accepted
        // Join with users table to get the friend's public_key
        let accepted_connections: Vec<(String, String)> = {
            let conn = self.db.conn.lock().unwrap();
            let stmt_result = conn.prepare(
                "SELECT p.friend_user_id, u.public_key FROM p2p_connections p
                 JOIN users u ON p.friend_user_id = u.id
                 WHERE p.user_id = ?1 AND p.status = 'accepted' AND p.initiated_by != ?1"
            );

            match stmt_result {
                Ok(mut stmt) => {
                    let rows = stmt.query_map(rusqlite::params![self.user_id.to_string()], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    });

                    match rows {
                        Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
                        Err(e) => {
                            println!("[FRIEND-RESEND] Query failed: {}", e);
                            Vec::new()
                        }
                    }
                }
                Err(e) => {
                    println!("[FRIEND-RESEND] Failed to prepare query: {}", e);
                    Vec::new()
                }
            }
        };

        if accepted_connections.is_empty() {
            println!("[FRIEND-RESEND] No accepted connections need FriendAccepted resend");
            return;
        }

        println!("[FRIEND-RESEND] Found {} accepted connections to resend FriendAccepted", accepted_connections.len());

        // Get our node address for the FriendAccepted message
        let endpoint_guard = self.endpoint.lock().await;
        let (our_node_id, our_relay_url) = if let Some(endpoint) = endpoint_guard.as_ref() {
            match endpoint.node_addr().await {
                Ok(addr) => (
                    addr.node_id.to_string(),
                    addr.relay_url().map(|u| u.to_string()).unwrap_or_default()
                ),
                Err(e) => {
                    println!("[FRIEND-RESEND] Failed to get node address: {}", e);
                    return;
                }
            }
        } else {
            println!("[FRIEND-RESEND] No endpoint available");
            return;
        };
        drop(endpoint_guard);

        for (friend_user_id, friend_public_key) in accepted_connections {
            println!("[FRIEND-RESEND] Resending FriendAccepted to {} ({})", friend_user_id, friend_public_key);

            let friend_accepted = P2PMessage::FriendAccepted {
                from_user_id: self.user_id,
                from_public_key: self.public_key.clone(),
                from_display_name: self.display_name.clone(),
                from_node_id: our_node_id.clone(),
                from_relay_url: our_relay_url.clone(),
                to_public_key: friend_public_key.clone(),
            };

            if let Err(e) = self.publish_message(CONTENT_TOPIC, friend_accepted).await {
                println!("[FRIEND-RESEND] Warning: Failed to resend FriendAccepted to {}: {}", friend_public_key, e);
            } else {
                println!("[FRIEND-RESEND] [OK] Resent FriendAccepted to {} (ensuring they know we accepted)", friend_public_key);
            }
        }
    }

    /// Helper to clone fields for background tasks
    fn clone_for_background(&self) -> Self {
        IrohNetwork {
            user_id: self.user_id,
            display_name: self.display_name.clone(),
            public_key: self.public_key.clone(),
            device_id: self.device_id.clone(),
            endpoint: self.endpoint.clone(),
            gossip: self.gossip.clone(),
            router: self.router.clone(),
            blobs: self.blobs.clone(),
            topics: self.topics.clone(),
            connected_peers: self.connected_peers.clone(),
            peer_retry_counts: self.peer_retry_counts.clone(),
            peer_heartbeats: self.peer_heartbeats.clone(),
            pending_subscriptions: self.pending_subscriptions.clone(),
            recent_message_hashes: self.recent_message_hashes.clone(),
            device_seed: self.device_seed,
            app_handle: self.app_handle.clone(),
            db: self.db.clone(),
            last_presence_success: self.last_presence_success.clone(),
            last_heartbeat_success: self.last_heartbeat_success.clone(),
            shutdown_flag: self.shutdown_flag.clone(),
        }
    }

    /// Queue a topic subscription to be processed in background
    /// This is used when we can't call subscribe_topic directly (e.g., from spawned tasks)
    #[allow(dead_code)]
    pub async fn queue_topic_subscription(&self, topic: String) {
        let mut pending = self.pending_subscriptions.lock().await;
        if !pending.contains(&topic) {
            println!("[IROH] Queuing topic subscription: {}", topic);
            pending.push(topic);
        }
    }

    /// Process any pending topic subscriptions
    async fn process_pending_subscriptions(&self) {
        let topics_to_subscribe: Vec<String> = {
            let mut pending = self.pending_subscriptions.lock().await;
            std::mem::take(&mut *pending)
        };

        for topic in topics_to_subscribe {
            println!("[IROH] Processing pending subscription: {}", topic);
            if let Err(e) = self.subscribe_topic(&topic).await {
                println!("[IROH] Warning: Failed to subscribe to queued topic {}: {}", topic, e);
            } else {
                println!("[IROH] [OK] Subscribed to queued topic: {}", topic);
            }
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

    /// Check if a peer is in the connected set
    pub async fn is_peer_connected(&self, node_id: iroh::NodeId) -> bool {
        let peers = self.connected_peers.lock().await;
        peers.contains(&node_id)
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

    /// Get current user's encryption public key from database
    fn get_user_encryption_public_key(&self) -> Option<String> {
        let conn = self.db.conn.lock().unwrap();
        conn.query_row(
            "SELECT encryption_public_key FROM users WHERE id = ?1",
            rusqlite::params![self.user_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
    }

    /// Get current user's encryption private key from database
    fn get_user_encryption_private_key(&self) -> Option<String> {
        let conn = self.db.conn.lock().unwrap();
        conn.query_row(
            "SELECT encryption_private_key FROM users WHERE id = ?1",
            rusqlite::params![self.user_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
    }

    /// Get friend encryption public keys for creating sealed envelopes
    pub fn get_friend_encryption_public_keys(&self) -> Vec<String> {
        let conn = self.db.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT u.encryption_public_key FROM users u
             INNER JOIN p2p_connections p ON u.id = p.friend_user_id
             WHERE p.user_id = ?1 AND p.status = 'accepted' AND u.encryption_public_key IS NOT NULL"
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let result: Vec<String> = match stmt.query_map(rusqlite::params![self.user_id], |row| row.get(0)) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(_) => vec![],
        };
        result
    }

    /// Get connection status
    pub async fn get_connection_status(&self) -> Result<serde_json::Value, String> {
        let endpoint_guard = self.endpoint.lock().await;
        let (has_endpoint, node_id, relay_url) = if let Some(endpoint) = endpoint_guard.as_ref() {
            let node_id = endpoint.node_id().to_string();
            // home_relay() returns Watcher<Option<RelayUrl>>
            // Watcher.get() returns Result<Option<RelayUrl>, Disconnected>
            let relay_url = endpoint
                .home_relay()
                .get()
                .ok()
                .flatten()
                .map(|url| url.to_string())
                .unwrap_or_default();
            (true, node_id, relay_url)
        } else {
            (false, String::new(), String::new())
        };
        drop(endpoint_guard);

        // connected_peers is a HashSet, use .iter() not .keys()
        let connected_peers = self.connected_peers.lock().await;
        let connected_count = connected_peers.len();
        let peer_ids: Vec<String> = connected_peers.iter().map(|k| k.to_string()).collect();
        // Debug: Log the actual peer IDs when count > 1 to help debug phantom peers
        if connected_count > 1 {
            println!("[IROH-DEBUG] connected_peers count={}, ids={:?}", connected_count, peer_ids);
        }
        drop(connected_peers);

        // Get subscribed topics
        let topics_guard = self.topics.lock().await;
        let subscribed_topics: Vec<String> = topics_guard.keys().cloned().collect();
        let topic_count = subscribed_topics.len();
        drop(topics_guard);

        // Get active peers (peers with recent heartbeats) - use peer_heartbeats field
        let heartbeats_guard = self.peer_heartbeats.lock().await;
        let active_peers: Vec<String> = heartbeats_guard.keys().map(|k| k.to_string()).collect();
        drop(heartbeats_guard);

        Ok(serde_json::json!({
            "listening": has_endpoint,
            "connected_peers": connected_count,
            "node_id": node_id,
            "public_key": self.public_key,
            "device_id": self.device_id,
            "relay_url": relay_url,
            "topic_count": topic_count,
            "subscribed_topics": subscribed_topics,
            "peer_ids": peer_ids,
            "active_peers": active_peers,
        }))
    }

    /// Start heartbeat sender - sends heartbeats to cipher/presence every 15 seconds
    fn start_heartbeat_sender(&self) {
        let network = Arc::new(self.clone_for_background());
        let shutdown_flag = self.shutdown_flag.clone();
        let last_success = self.last_heartbeat_success.clone();

        tokio::spawn(async move {
            // Wait a bit for initialization
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));
            loop {
                interval.tick().await;

                // Check if we should stop
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("[HEARTBEAT] Heartbeat sender stopping due to shutdown flag");
                    break;
                }

                // Process any pending topic subscriptions from message handlers
                network.process_pending_subscriptions().await;

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

                    // Send to global content topic
                    match network.publish_message(CONTENT_TOPIC, heartbeat).await {
                        Ok(_) => {
                            println!("[HEARTBEAT] [OK] Sent heartbeat");
                            // Track successful heartbeat
                            let mut guard = last_success.lock().await;
                            *guard = Some(std::time::Instant::now());
                        }
                        Err(e) => {
                            println!("[HEARTBEAT] Failed to send heartbeat: {}", e);
                        }
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
        let shutdown_flag = self.shutdown_flag.clone();

        tokio::spawn(async move {
            // Wait a bit for initialization
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

            let interval = tokio::time::Duration::from_secs(30);
            let heartbeat_timeout = std::time::Duration::from_secs(45);

            loop {
                tokio::time::sleep(interval).await;

                // Check if we should stop
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("[HEARTBEAT] Heartbeat monitor stopping due to shutdown flag");
                    break;
                }

                let now = std::time::Instant::now();
                let heartbeats = network.peer_heartbeats.lock().await;
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

    /// Check network health and return detailed status
    /// Returns: (is_healthy, needs_reconnect, status_details)
    pub async fn health_check(&self) -> (bool, bool, serde_json::Value) {
        let now = std::time::Instant::now();
        let max_stale_time = std::time::Duration::from_secs(60); // Consider unhealthy if no success in 60s

        // Check endpoint exists
        let endpoint_guard = self.endpoint.lock().await;
        let has_endpoint = endpoint_guard.is_some();
        let relay_connected = if let Some(endpoint) = endpoint_guard.as_ref() {
            // Check if we have a relay URL (indicates relay connection)
            endpoint.home_relay().get().ok().flatten().is_some()
        } else {
            false
        };
        drop(endpoint_guard);

        // Check gossip exists
        let gossip_guard = self.gossip.lock().await;
        let has_gossip = gossip_guard.is_some();
        drop(gossip_guard);

        // Check topic subscription
        let topics_guard = self.topics.lock().await;
        let has_content_topic = topics_guard.contains_key(CONTENT_TOPIC);
        drop(topics_guard);

        // Check last successful presence
        let presence_guard = self.last_presence_success.lock().await;
        let presence_stale = match *presence_guard {
            Some(last) => now.duration_since(last) > max_stale_time,
            None => true, // Never succeeded
        };
        let presence_age_secs = presence_guard.map(|t| now.duration_since(t).as_secs());
        drop(presence_guard);

        // Check last successful heartbeat
        let heartbeat_guard = self.last_heartbeat_success.lock().await;
        let heartbeat_stale = match *heartbeat_guard {
            Some(last) => now.duration_since(last) > max_stale_time,
            None => true, // Never succeeded
        };
        let heartbeat_age_secs = heartbeat_guard.map(|t| now.duration_since(t).as_secs());
        drop(heartbeat_guard);

        // Determine health
        let is_healthy = has_endpoint && has_gossip && has_content_topic && relay_connected && !presence_stale && !heartbeat_stale;
        let needs_reconnect = !has_endpoint || !has_gossip || !has_content_topic || !relay_connected;

        let status = serde_json::json!({
            "healthy": is_healthy,
            "needs_reconnect": needs_reconnect,
            "has_endpoint": has_endpoint,
            "relay_connected": relay_connected,
            "has_gossip": has_gossip,
            "has_content_topic": has_content_topic,
            "presence_stale": presence_stale,
            "presence_age_secs": presence_age_secs,
            "heartbeat_stale": heartbeat_stale,
            "heartbeat_age_secs": heartbeat_age_secs,
        });

        println!("[IROH-HEALTH] Health check: healthy={}, needs_reconnect={}", is_healthy, needs_reconnect);
        if !is_healthy {
            println!("[IROH-HEALTH] Details: {:?}", status);
        }

        (is_healthy, needs_reconnect, status)
    }

    /// Recover network connectivity without full reinitialization
    /// This is faster than full shutdown+init and preserves NAT traversal state
    pub async fn recover(&self) -> Result<(), String> {
        println!("[IROH-RECOVER] Starting network recovery...");

        // Reset shutdown flag to allow new background loops
        self.shutdown_flag.store(false, std::sync::atomic::Ordering::Relaxed);

        // Check if we need to re-subscribe to the content topic
        let topics_guard = self.topics.lock().await;
        let has_content_topic = topics_guard.contains_key(CONTENT_TOPIC);
        drop(topics_guard);

        if !has_content_topic {
            println!("[IROH-RECOVER] Content topic missing, re-subscribing...");
            self.subscribe_topic(CONTENT_TOPIC).await?;
        }

        // Restart background loops (they check shutdown_flag and will start fresh)
        println!("[IROH-RECOVER] Restarting background loops...");
        self.start_presence_loop();
        self.start_heartbeat_sender();
        self.start_heartbeat_monitor();

        // Announce presence immediately
        if let Err(e) = self.announce_presence().await {
            println!("[IROH-RECOVER] Warning: Initial presence announcement failed: {}", e);
            // Don't fail recovery for this - background loop will retry
        } else {
            println!("[IROH-RECOVER] [OK] Initial presence announced");
        }

        // Try to reconnect to known peers
        self.start_active_peer_discovery();

        println!("[IROH-RECOVER] Recovery complete");
        Ok(())
    }

    /// Shutdown the network
    pub async fn shutdown(&self) -> Result<(), String> {
        println!("[IROH] Shutting down Iroh network...");

        // Signal background loops to stop
        self.shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);

        // Give background loops a moment to notice the flag
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

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
