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
use iroh::address_lookup::memory::MemoryLookup;
use iroh::protocol::Router;
use iroh_blobs::api::downloader::Downloader;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::BlobsProtocol;
use iroh_gossip::api::{Event as GossipEvent, GossipReceiver, GossipSender};
use iroh_gossip::ALPN;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

use super::types::SqliteUuid;
use super::Database;

/// Global topic for all encrypted content
pub const CONTENT_TOPIC: &str = "cipher/content/v1";

/// Maximum gossip message size. iroh-gossip 0.101 defaults to 4096 bytes,
/// which real sealed envelopes easily exceed - we raise it explicitly and
/// enforce the same limit on the publish path so oversize messages get a
/// distinct, diagnosable error instead of a generic broadcast failure.
pub const GOSSIP_MAX_MESSAGE_SIZE: usize = 256 * 1024;

/// Pre-sealing size budget for one device-sync chunk. Sealing adds base64
/// (~1.33x), padding and per-recipient key boxes, so 96KB of serialized
/// SyncData stays safely under the 256KB gossip cap.
const SYNC_CHUNK_BUDGET_BYTES: usize = 96 * 1024;

/// P2P message types for the social network.
/// ALL application content (posts, comments, reactions, DMs, presence, friend
/// handshake, device sync data) travels as SealedEnvelope - signed inside,
/// encrypted to its recipients, metadata-free on the wire. The only other
/// messages are the signed sync request and neighbor-scoped heartbeats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum P2PMessage {
    /// Sealed envelope containing encrypted, sender-signed content.
    /// All nodes relay these; only intended recipients can decrypt.
    SealedEnvelope {
        /// The full GossipEnvelope with key boxes and encrypted payload
        envelope_json: String,
    },
    /// Device sync request (same user, different device).
    /// SECURITY: signed with the user's Ed25519 key - without the signature,
    /// anyone could broadcast a request claiming our public key and trigger a
    /// sync response.
    DeviceSyncRequest {
        public_key: String,
        device_id: String,
        last_sync_timestamp: i64,
        #[serde(default)]
        timestamp: i64,
        #[serde(default)]
        signature: String,
    },
    /// Heartbeat to confirm a live connection. Sent to direct gossip
    /// neighbors only (broadcast_neighbors) - neighbors already know our
    /// EndpointId from the QUIC connection, so this reveals nothing new,
    /// and it is never relayed mesh-wide.
    Heartbeat {
        node_id: String, // Sender's Iroh NodeId
        timestamp: i64,
    },
}

/// Topic subscription with channel for sending messages and gossip sender for peer management
/// The GossipSender allows dynamic peer joining without re-subscription
/// The actual GossipReceiver is owned by the stream handler task
struct TopicSubscription {
    broadcast_tx: tokio::sync::mpsc::UnboundedSender<OutgoingMessage>,
    #[allow(dead_code)] // Kept for future dynamic peer joining
    gossip_sender: Arc<iroh_gossip::api::GossipSender>,
}

/// A message queued for gossip broadcast.
/// `neighbors_only` uses broadcast_neighbors() - delivered to direct gossip
/// neighbors without mesh-wide relay. Used for heartbeats, which only need to
/// confirm live connections; flooding them lets every mesh node track every
/// device's online times.
struct OutgoingMessage {
    data: Bytes,
    neighbors_only: bool,
    /// Optional ack channel: the stream handler sends the REAL broadcast
    /// result back through this so publishers can distinguish "queued into a
    /// channel" from "actually handed to the gossip mesh".
    ack: Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
}

/// Insertion-ordered dedup cache for recently seen gossip message hashes.
/// The VecDeque preserves insertion order so eviction always removes the
/// OLDEST entries - the previous HashSet-only version evicted arbitrary
/// entries, sometimes throwing away fresh hashes and re-processing duplicates.
struct RecentMessageCache {
    order: std::collections::VecDeque<u64>,
    seen: std::collections::HashSet<u64>,
}

impl RecentMessageCache {
    const MAX_ENTRIES: usize = 1000;

    fn new() -> Self {
        RecentMessageCache {
            order: std::collections::VecDeque::new(),
            seen: std::collections::HashSet::new(),
        }
    }

    /// Record a hash. Returns false if it was already present (duplicate).
    fn insert(&mut self, hash: u64) -> bool {
        if !self.seen.insert(hash) {
            return false;
        }
        self.order.push_back(hash);
        while self.order.len() > Self::MAX_ENTRIES {
            if let Some(oldest) = self.order.pop_front() {
                self.seen.remove(&oldest);
            }
        }
        true
    }

    /// Forget a hash so a redelivered copy of the message is processed again.
    /// Used when a sealed envelope's handler fails transiently: leaving the
    /// hash in place would dedup the gossip redelivery before it ever reached
    /// the handler, permanently discarding the envelope.
    fn remove(&mut self, hash: u64) {
        if self.seen.remove(&hash) {
            self.order.retain(|h| *h != hash);
        }
    }
}

/// Removes a topic's subscription entry from the shared map when its stream
/// handler task exits for ANY reason - graceful stream end, error, or a panic
/// while handling an event. Without this, a panicked handler left a zombie
/// entry: publish_message kept queuing into a channel nobody drained, and
/// recover() skipped re-subscribing because the topic looked present.
struct TopicCleanupGuard {
    topics: Arc<Mutex<HashMap<String, TopicSubscription>>>,
    topic: String,
    /// Identity of the subscription this handler serves - only remove the map
    /// entry if it is still OURS (an atomic-swap replacement may have already
    /// installed a new subscription under the same topic name).
    broadcast_tx: tokio::sync::mpsc::UnboundedSender<OutgoingMessage>,
}

impl Drop for TopicCleanupGuard {
    fn drop(&mut self) {
        let topics = self.topics.clone();
        let topic = std::mem::take(&mut self.topic);
        let our_tx = self.broadcast_tx.clone();
        // Drop can't await the async mutex - spawn the removal (we are always
        // dropped on a runtime thread, including during panic unwind)
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut topics_guard = topics.lock().await;
                let is_ours = topics_guard
                    .get(&topic)
                    .map(|sub| sub.broadcast_tx.same_channel(&our_tx))
                    .unwrap_or(false);
                if is_ours {
                    topics_guard.remove(&topic);
                    println!(
                        "[IROH-STREAM] Cleanup guard removed topic '{}' from map (handler exited)",
                        topic
                    );
                }
            });
        }
    }
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

/// Canonical string a DeviceSyncRequest signature covers - binds all request
/// fields so none can be altered without invalidating the signature
fn sync_request_context(
    public_key: &str,
    device_id: &str,
    last_sync_timestamp: i64,
    timestamp: i64,
) -> String {
    format!(
        "syncreq_v1|{}|{}|{}|{}",
        public_key, device_id, last_sync_timestamp, timestamp
    )
}

/// The reaction emojis the app actually offers (mirrors
/// `PostInteractions.EMOJIS` in src/js/main.js). Peer-supplied reaction strings
/// are rendered into post markup by the frontend, so anything outside this set
/// is rejected at the receive boundary rather than trusted to be escaped later.
/// Both the emoji-presentation form (with U+FE0F) and the bare code point are
/// accepted, since senders on different platforms normalise differently.
const SUPPORTED_REACTION_EMOJIS: &[&str] = &[
    "\u{1F44D}",         // 👍
    "\u{2764}\u{FE0F}",  // ❤️
    "\u{2764}",          // ❤ (no variation selector)
    "\u{1F602}",         // 😂
    "\u{1F62E}",         // 😮
    "\u{1F622}",         // 😢
    "\u{1F621}",         // 😡
    "\u{1F44D}\u{FE0F}", // 👍️
];

/// True if `emoji` is one of the reactions this app supports.
fn is_supported_reaction_emoji(emoji: &str) -> bool {
    SUPPORTED_REACTION_EMOJIS.contains(&emoji)
}

/// MIME types we accept for peer-supplied attachments. The frontend renders
/// `fileType` into markup and uses it to decide whether to build an <img>/
/// <video> element, so an unconstrained string from a peer is both a rendering
/// and an injection hazard. SVG is deliberately absent: it is an active
/// document and would execute script when rendered as an image.
const SUPPORTED_MEDIA_TYPES: &[&str] = &[
    "image/jpeg",
    "image/jpg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/heic",
    "image/heif",
    "image/avif",
    "video/mp4",
    "video/quicktime",
    "video/webm",
    "video/mpeg",
    "video/x-matroska",
    "audio/mpeg",
    "audio/mp4",
    "audio/aac",
    "audio/wav",
    "audio/x-wav",
    "audio/ogg",
    "audio/webm",
    "application/pdf",
    "text/plain",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
];

/// True if a peer-supplied attachment MIME type is on the allowlist. Any
/// parameters (e.g. `; codecs=...`) are stripped and matching is
/// case-insensitive, per RFC 2045.
fn is_supported_media_type(file_type: &str) -> bool {
    let base = file_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    SUPPORTED_MEDIA_TYPES.contains(&base.as_str())
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
    /// Uses iroh-blobs for efficient P2P transfer over existing NAT-traversed connections.
    /// Filesystem-backed (app data dir) so attachments survive restarts.
    pub store: Arc<Mutex<Option<FsStore>>>,
    /// Downloader for fetching blobs from peers (created once the endpoint is bound)
    pub downloader: Arc<Mutex<Option<Downloader>>>,
    /// Runtime registry of known peer addresses (replaces iroh 0.35 `add_node_addr`).
    /// Seeded from stored relay URLs and Presence messages so gossip can dial peers
    /// by EndpointId even before DHT/DNS lookups resolve them. Arc-based, cheap to clone.
    address_book: MemoryLookup,
    topics: Arc<Mutex<HashMap<String, TopicSubscription>>>,
    connected_peers: Arc<Mutex<std::collections::HashSet<iroh::EndpointId>>>,
    peer_retry_counts: Arc<Mutex<HashMap<iroh::EndpointId, PeerRetryState>>>, // Exponential backoff tracking
    peer_heartbeats: Arc<Mutex<HashMap<iroh::EndpointId, std::time::Instant>>>, // Last heartbeat time from each peer
    pending_subscriptions: Arc<Mutex<Vec<String>>>, // Topics queued for subscription (processed in background)
    /// Recent message hashes for deduplication (gossip protocols may deliver same message via multiple paths)
    recent_message_hashes: Arc<Mutex<RecentMessageCache>>,
    device_seed: [u8; 32], // Device keypair seed for DHT publishing
    app_handle: tauri::AppHandle,
    db: Database,
    /// Health tracking: last time presence was successfully sent
    last_presence_success: Arc<Mutex<Option<std::time::Instant>>>,
    /// Health tracking: last time heartbeat was successfully sent
    last_heartbeat_success: Arc<Mutex<Option<std::time::Instant>>>,
    /// Health tracking: last time a message was ACTUALLY broadcast to the gossip mesh.
    /// The presence/heartbeat timestamps above only prove the message was queued into an
    /// in-memory channel (publish_message always succeeds), so they stay fresh even when
    /// the network is dead. This one is set by the stream handler on real broadcast success.
    last_broadcast_success: Arc<Mutex<Option<std::time::Instant>>>,
    /// Flag to signal background loops should stop (for clean shutdown)
    /// Public so app lifecycle handlers can signal shutdown
    pub shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    /// Handles of spawned background loops so recover()/shutdown() can abort them.
    /// The loops only observe shutdown_flag at their next tick (15-30s, never while
    /// the process is suspended on mobile), so flag-based stopping alone leaks a full
    /// set of duplicate loops on every recover().
    background_tasks: Arc<std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// Guards against concurrent recover() calls (foreground event racing the
    /// frontend health-check loop) each spawning their own set of loops
    recovering: Arc<std::sync::atomic::AtomicBool>,
    /// Last time we sent a backfill (FriendSyncRequest) to each friend, so a
    /// burst of presence messages doesn't trigger a request storm. Keyed by
    /// friend user_id.
    friend_sync_sent: Arc<Mutex<HashMap<SqliteUuid, std::time::Instant>>>,
    /// Guards against overlapping retry_pending_messages() runs - friends are
    /// now dialed concurrently, and each successful dial triggers a retry pass
    retrying_pending: Arc<std::sync::atomic::AtomicBool>,
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
            store: Arc::new(Mutex::new(None)),
            downloader: Arc::new(Mutex::new(None)),
            address_book: MemoryLookup::new(),
            topics: Arc::new(Mutex::new(HashMap::new())),
            connected_peers: Arc::new(Mutex::new(std::collections::HashSet::new())),
            peer_retry_counts: Arc::new(Mutex::new(HashMap::new())),
            peer_heartbeats: Arc::new(Mutex::new(HashMap::new())),
            pending_subscriptions: Arc::new(Mutex::new(Vec::new())),
            recent_message_hashes: Arc::new(Mutex::new(RecentMessageCache::new())),
            device_seed,
            app_handle,
            db,
            last_presence_success: Arc::new(Mutex::new(None)),
            last_heartbeat_success: Arc::new(Mutex::new(None)),
            last_broadcast_success: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            background_tasks: Arc::new(std::sync::Mutex::new(Vec::new())),
            recovering: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            friend_sync_sent: Arc::new(Mutex::new(HashMap::new())),
            retrying_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

        // Configure BitTorrent Mainline DHT address lookup (PRIMARY, censorship-resistant).
        // CRITICAL: Provide secret_key to enable DHT PUBLISHING - without this we only
        // consume the DHT but never publish our own address, so peers can't find us.
        let dht_lookup = iroh_mainline_address_lookup::DhtAddressLookup::builder()
            .secret_key(secret_key.clone())
            .build()
            .map_err(|e| format!("Failed to create DHT address lookup: {}", e))?;

        // Build endpoint with the N0 preset (n0 DNS discovery + relays for NAT traversal),
        // adding our DHT lookup and the MemoryLookup address book on top. Multiple
        // address_lookup() calls compose and fail over, matching our multi-path design.
        let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::N0)
            .secret_key(secret_key.clone())
            .address_lookup(dht_lookup) // BitTorrent DHT - fully decentralized
            .address_lookup(self.address_book.clone()) // Out-of-band peer addresses
            .bind()
            .await
            .map_err(|e| format!("Failed to create Iroh endpoint: {}", e))?;

        // mDNS local-network discovery is registered at runtime because it needs our
        // EndpointId, which only exists after the endpoint is bound.
        #[cfg(feature = "mdns")]
        {
            match iroh_mdns_address_lookup::MdnsAddressLookup::builder().build(endpoint.id()) {
                Ok(mdns) => {
                    if let Some(lookup) = endpoint.address_lookup() {
                        lookup.add(mdns);
                        println!("[IROH] mDNS local-network address lookup enabled");
                    }
                }
                Err(e) => println!("[IROH] Warning: Failed to create mDNS lookup: {}", e),
            }
        }

        let node_id = endpoint.id();
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
            let our_node_addr = endpoint.addr();
            let relay_url_owned = our_node_addr.relay_urls().next().map(|u| u.to_string());
            if let Some(relay_url_str) = relay_url_owned {
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

        // Create gossip protocol (spawn is synchronous in iroh-gossip 0.101)
        // EXPLICIT max message size: the library default is 4096 bytes, which
        // real sealed envelopes exceed - oversize messages were silently
        // failing to broadcast.
        let gossip = iroh_gossip::net::Gossip::builder()
            .max_message_size(GOSSIP_MAX_MESSAGE_SIZE)
            .spawn(endpoint.clone());

        println!("[IROH] Gossip protocol initialized");

        // Create PERSISTENT blob store for large file transfers (images, attachments).
        // Lives next to the sqlite DB in the app data dir - the old in-memory store
        // lost every attachment on restart (sender AND receiver side).
        let blobs_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {}", e))?
            .join("blobs");
        std::fs::create_dir_all(&blobs_dir)
            .map_err(|e| format!("Failed to create blobs directory: {}", e))?;
        let store = FsStore::load(&blobs_dir)
            .await
            .map_err(|e| format!("Failed to load blob store: {}", e))?;
        let downloader = store.downloader(&endpoint);
        let blobs = BlobsProtocol::new(&store, None);
        println!(
            "[IROH] Blob store initialized (persistent: {:?})",
            blobs_dir
        );

        // Create Router to accept incoming gossip AND blob connections
        // Both protocols share the same NAT-traversed QUIC connection
        let router = Router::builder(endpoint.clone())
            .accept(ALPN, gossip.clone())
            .accept(iroh_blobs::ALPN, blobs)
            .spawn();

        println!("[IROH] Router created - accepting gossip and blob connections");
        println!("[IROH] Direct RPC acceptor ready for incoming Presence handshakes");

        // Store endpoint, gossip, blob store, downloader, and router
        *self.endpoint.lock().await = Some(endpoint.clone());
        *self.gossip.lock().await = Some(gossip);
        *self.store.lock().await = Some(store);
        *self.downloader.lock().await = Some(downloader);
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
        if let Ok(peer_addrs) = self.db.get_all_friend_peer_addresses(self.user_id) {
            println!(
                "[IROH] Found {} friend peer addresses in database",
                peer_addrs.len()
            );
            // Build one dial future per stored peer, then run them CONCURRENTLY.
            // Serial dialing made startup latency scale linearly with the number
            // of offline friends (5s timeout each).
            let mut dial_futures = Vec::new();
            for (node_id_str, relay_url_str) in &peer_addrs {
                println!(
                    "[IROH] Processing peer address: node_id={}, relay={}",
                    node_id_str, relay_url_str
                );
                let peer_id = match node_id_str.parse::<iroh::EndpointId>() {
                    Ok(peer_id) => peer_id,
                    Err(e) => {
                        println!(
                            "[IROH] Failed to parse peer NodeId '{}': {}",
                            node_id_str, e
                        );
                        continue;
                    }
                };
                if peer_id == node_id {
                    println!("[IROH] Skipping self (peer_id matches our node_id)");
                    continue;
                }

                // Construct full EndpointAddr with relay URL for reliable connection
                let mut peer_node_addr = iroh::EndpointAddr::new(peer_id);
                if let Ok(url) = relay_url_str.parse() {
                    peer_node_addr = peer_node_addr.with_relay_url(url);
                    println!(
                        "[IROH] Connecting to peer {} with relay: {}",
                        peer_id, relay_url_str
                    );
                } else {
                    println!("[IROH] Warning: Invalid relay URL for peer {}", peer_id);
                    continue;
                }

                // Register the peer's address in the address book so gossip (which
                // dials by EndpointId) can resolve it. Replaces 0.35 add_node_addr.
                self.address_book.add_endpoint_info(peer_node_addr.clone());

                // Bound the attempt - an unreachable peer otherwise stalls app
                // startup for the full QUIC handshake timeout.
                let endpoint = endpoint.clone();
                let node_id_str = node_id_str.clone();
                dial_futures.push(async move {
                    println!("[IROH] Attempting to connect to peer: {}", peer_id);
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(5),
                        endpoint.connect(peer_node_addr, iroh_gossip::ALPN),
                    )
                    .await
                    {
                        Ok(Ok(_conn)) => {
                            println!("[IROH] [OK] CONNECTED to peer {}!", peer_id);
                            Some((peer_id, node_id_str))
                        }
                        Ok(Err(e)) => {
                            println!("[IROH] Failed to connect to {}: {}", peer_id, e);
                            None
                        }
                        Err(_) => {
                            println!("[IROH] Connect to {} timed out after 5s", peer_id);
                            None
                        }
                    }
                });
            }

            for (peer_id, node_id_str) in futures::future::join_all(dial_futures)
                .await
                .into_iter()
                .flatten()
            {
                // Track this connection and keep the peer for gossip bootstrap
                self.add_connected_peer(peer_id).await;
                peer_node_ids.push(node_id_str);
            }
        }

        // GLOBAL MESH ARCHITECTURE:
        // All nodes join the single cipher/content/v1 topic
        // This creates a global gossip mesh where everyone relays everything
        // Privacy is achieved through encryption, not topology
        println!("[IROH] ========================================");
        println!("[IROH] GLOBAL MESH ARCHITECTURE");
        println!(
            "[IROH] Subscribing to global content topic: {}",
            CONTENT_TOPIC
        );
        println!("[IROH] All content will be broadcast to all peers");
        println!("[IROH] Only friends can decrypt your content");
        println!("[IROH] ========================================");

        // Subscribe to the global content topic
        // If we have bootstrap peers from previous sessions, use them
        if !peer_node_ids.is_empty() {
            println!(
                "[IROH] Subscribing with {} bootstrap peers",
                peer_node_ids.len()
            );
            if let Err(e) = self
                .subscribe_topic_with_peers(CONTENT_TOPIC, peer_node_ids.clone())
                .await
            {
                println!("[IROH] Warning: Failed to subscribe with bootstrap: {}", e);
                println!("[IROH] Falling back to root subscription...");
                self.subscribe_topic(CONTENT_TOPIC).await?;
            }
        } else {
            println!("[IROH] No bootstrap peers - subscribing as root node");
            self.subscribe_topic(CONTENT_TOPIC).await?;
        }
        println!("[IROH] [OK] Subscribed to global content topic");

        // Start presence announcements, heartbeat, and periodic sync
        self.start_presence_loop();
        self.start_heartbeat_sender();
        self.start_heartbeat_monitor();
        self.start_periodic_device_sync();

        // Actively try to connect to discovered peers via direct connection attempts
        // This helps bootstrap the gossip mesh using stored peer addresses
        // CENSORSHIP RESISTANCE: Even if infrastructure is blocked, users can exchange
        // peer addresses out-of-band (QR codes, messaging apps) and manually bootstrap
        self.start_active_peer_discovery();

        // Friend reconnection is handled by the active peer discovery loop started above:
        // it pre-populates node addresses, connects with exponential backoff, and performs
        // the handle_connection() + join_peers() handshake on success. Its first pass runs
        // ~1s after spawn. Doing the same work inline here (serial, 3 attempts per friend,
        // no per-connect timeout) used to block app startup for minutes when friends were
        // offline.

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

    /// Subscribe to a gossip topic with bootstrap peers (takes iroh::EndpointId directly)
    #[allow(dead_code)]
    pub async fn subscribe_with_bootstrap(
        &self,
        topic: &str,
        bootstrap_peers: Vec<iroh::EndpointId>,
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

        // Clone the Gossip handle out of the guard - holding the mutex across
        // the 500ms settle sleep and subscribe().await below stalled every
        // other gossip user (discovery loop, publish path) for the duration.
        let gossip = {
            let gossip_guard = self.gossip.lock().await;
            gossip_guard
                .as_ref()
                .ok_or("Gossip not initialized")?
                .clone()
        };

        // Convert topic string to TopicId
        let topic_id = iroh_gossip::proto::TopicId::from(Self::topic_to_id(topic));

        // Convert String NodeIds to iroh::EndpointId types
        let bootstrap_peers: Vec<iroh::EndpointId> = peer_node_ids
            .iter()
            .filter_map(|id_str| match id_str.parse::<iroh::EndpointId>() {
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

        // ALWAYS use subscribe_and_join() for proper gossip mesh formation.
        // With empty bootstrap, it returns immediately as a root node.
        // With bootstrap peers, it waits for NeighborUp to confirm mesh formation.
        //
        // Key: subscribe_and_join() enables both sending AND receiving.
        // The old subscribe() only enabled sending reliably.
        println!("[IROH-GOSSIP] Calling gossip.subscribe_and_join() for topic '{}' with {} bootstrap peers...", topic, bootstrap_peers.len());

        // Give the gossip protocol layer time to establish after QUIC connection
        if !bootstrap_peers.is_empty() {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // All nodes use subscribe() - mesh forms dynamically via join_peers() calls
        // No "root" node concept - every peer is equal
        println!("[IROH-GOSSIP] Creating gossip subscription...");
        let gossip_topic = match gossip.subscribe(topic_id, bootstrap_peers.clone()).await {
            Ok(result) => {
                println!(
                    "[IROH-GOSSIP] [OK] Subscription created for topic '{}'",
                    topic
                );
                result
            }
            Err(e) => {
                println!("[IROH-GOSSIP] ✗ Failed to subscribe: {}", e);
                return Err(format!("Failed to subscribe to gossip topic: {}", e));
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
        let broadcast_tx_for_handler = broadcast_tx.clone();
        self.topics.lock().await.insert(
            topic.to_string(),
            TopicSubscription {
                broadcast_tx,
                gossip_sender: gossip_sender_arc.clone(),
            },
        );

        // Start listening to the receiver stream AND handle broadcast requests
        let network = Arc::new(self.clone_for_background());
        let topic_str = topic.to_string();
        let gossip_sender_for_handler = gossip_sender_arc.clone();
        let bootstrap_peers_for_handler = bootstrap_peers.clone();
        tokio::spawn(async move {
            network
                .handle_topic_stream(
                    topic_str,
                    gossip_sender_for_handler,
                    gossip_receiver,
                    broadcast_rx,
                    bootstrap_peers_for_handler,
                    broadcast_tx_for_handler,
                )
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
        mut broadcast_rx: tokio::sync::mpsc::UnboundedReceiver<OutgoingMessage>,
        bootstrap_peers: Vec<iroh::EndpointId>,
        broadcast_tx: tokio::sync::mpsc::UnboundedSender<OutgoingMessage>,
    ) {
        use futures::StreamExt;

        // Remove our topic entry from the map on ANY exit of this task -
        // including panics - so publish_message fails fast and recover()
        // re-subscribes instead of seeing a zombie entry forever.
        let _cleanup_guard = TopicCleanupGuard {
            topics: self.topics.clone(),
            topic: topic.clone(),
            broadcast_tx,
        };

        println!(
            "[IROH-STREAM] Starting message handler for topic: {}",
            topic
        );
        println!("[IROH-STREAM] Listening for gossip events and broadcast requests...");

        // CRITICAL: Join bootstrap peers INSIDE the handler, right before the loop
        // This ensures we're actively listening when join_peers() triggers events
        // Bounded: join_peers can hang on unreachable bootstrap peers; the
        // discovery loop retries joins later, so just proceed to the loop.
        if !bootstrap_peers.is_empty() {
            println!(
                "[IROH-STREAM] Joining {} bootstrap peers for bidirectional mesh...",
                bootstrap_peers.len()
            );
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(10),
                gossip_sender.join_peers(bootstrap_peers),
            )
            .await
            {
                Ok(Ok(_)) => {
                    println!("[IROH-STREAM] [OK] Joined bootstrap peers - ready to receive")
                }
                Ok(Err(e)) => println!(
                    "[IROH-STREAM] Warning: Failed to join bootstrap peers: {}",
                    e
                ),
                Err(_) => println!(
                    "[IROH-STREAM] Warning: join_peers timed out after 10s - discovery loop will retry"
                ),
            }
        }

        // Listen on both the gossip receiver stream (incoming) and broadcast channel (outgoing)
        loop {
            tokio::select! {
                // Incoming gossip messages from the receiver
                event = gossip_receiver.next() => {
                    match event {
                        Some(Ok(GossipEvent::Received(msg))) => {
                                    // DEDUPLICATION: Hash the message content and check if we've seen it recently
                                    // Gossip protocols may deliver the same message via multiple paths
                                    use std::hash::{Hash, Hasher};
                                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                                    // Hash CONTENT ONLY - including delivered_from defeated the
                                    // purpose, since the same message arriving via two paths has
                                    // two different delivery peers and was processed twice
                                    msg.content.hash(&mut hasher);
                                    let msg_hash = hasher.finish();

                                    {
                                        // Insertion-ordered cache: evicts the OLDEST hashes when
                                        // over capacity (keeps last 1000 messages)
                                        let mut seen = self.recent_message_hashes.lock().await;
                                        if !seen.insert(msg_hash) {
                                            // Already processed this message - skip
                                            continue;
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
                                                    P2PMessage::SealedEnvelope { .. } => "SealedEnvelope",
                                                    P2PMessage::DeviceSyncRequest { .. } => "DeviceSyncRequest",
                                                    P2PMessage::Heartbeat { .. } => "Heartbeat",
                                                }
                                            );
                                            // Handle the message OFF the select loop: handlers do
                                            // blocking DB work (and can panic on a poisoned mutex),
                                            // which stalled gossip receive AND outgoing broadcast
                                            // drain, causing Lagged drops. A panic in the spawned
                                            // task cannot kill this stream loop.
                                            let network = Arc::new(self.clone_for_background());
                                            let delivered_from = msg.delivered_from;
                                            tokio::spawn(async move {
                                                network.handle_message(p2p_msg, delivered_from, msg_hash).await;
                                            });
                                        }
                                        Err(e) => {
                                            println!("[IROH-STREAM] ✗ Failed to deserialize message: {}", e);
                                        }
                                    }
                                }
                        Some(Ok(GossipEvent::NeighborUp(peer))) => {
                                    println!("[IROH-STREAM] 📡 Neighbor UP on topic '{}': {}", topic, peer);
                                    // Peer became our neighbor - add to connected_peers
                                    println!("[IROH-STREAM]   Adding peer to connected_peers: {}", peer);
                                    self.add_connected_peer(peer).await;
                                }
                        Some(Ok(GossipEvent::NeighborDown(peer))) => {
                                    println!("[IROH-STREAM] 📴 Neighbor DOWN on topic '{}': {}", topic, peer);
                                    // NeighborDown indicates the gossip neighbor relationship has ended.
                                    // This typically happens when:
                                    // - Mobile app goes to background
                                    // - Network changes (WiFi <-> cellular)
                                    // - Connection timeout
                                    //
                                    // We MUST remove from connected_peers so the discovery loop
                                    // can attempt reconnection. The exponential backoff in the
                                    // discovery loop prevents hammering failed connections.
                                    println!("[IROH-STREAM]   Removing peer from connected set to enable reconnection");
                                    self.remove_connected_peer(peer).await;

                                    // Also reset the peer's backoff counter to allow immediate reconnection attempt
                                    // This is important for mobile resume - the peer is likely coming back online
                                    let mut retry_states = self.peer_retry_counts.lock().await;
                                    if let Some(retry_state) = retry_states.get_mut(&peer) {
                                        println!("[IROH-STREAM]   Resetting backoff for peer {} to enable quick reconnection", peer);
                                        retry_state.reset();
                                    }
                                    drop(retry_states);
                                }
                        Some(Ok(GossipEvent::Lagged)) => {
                            println!("[IROH-STREAM] ⚠️  Stream lagged on topic: {} - messages were dropped", topic);
                            // We provably missed gossip messages: trigger the existing
                            // catch-up paths without blocking this loop. Clearing the
                            // friend_sync_sent rate-limits lets the next presence from
                            // each friend trigger a backfill request immediately.
                            let network = Arc::new(self.clone_for_background());
                            tokio::spawn(async move {
                                network.friend_sync_sent.lock().await.clear();
                                println!("[IROH-STREAM] Lagged recovery: cleared friend backfill rate-limits");
                                if network.has_other_own_devices() {
                                    network.request_device_sync().await;
                                }
                            });
                        }
                        Some(Err(e)) => {
                            println!("[IROH-STREAM] ✗ Error on topic {} stream: {}", topic, e);
                            println!("[IROH-STREAM]   Cleanup guard removes topic from map so recover() will re-subscribe");
                            break;
                        }
                        None => {
                            println!("[IROH-STREAM] Stream ended for topic: {}", topic);
                            println!("[IROH-STREAM]   Cleanup guard removes topic from map so recover() will re-subscribe");
                            break;
                        }
                    }
                }

                // Outgoing broadcast messages via the sender
                Some(outgoing) = broadcast_rx.recv() => {
                    let OutgoingMessage { data, neighbors_only, ack } = outgoing;
                    println!("[IROH-STREAM] 📤 Broadcasting message to topic '{}' ({} bytes{})", topic, data.len(), if neighbors_only { ", neighbors only" } else { "" });
                    let result = if neighbors_only {
                        gossip_sender.broadcast_neighbors(data).await
                    } else {
                        gossip_sender.broadcast(data).await
                    };
                    let result = match result {
                        Ok(_) => {
                            println!("[IROH-STREAM] [OK] Successfully broadcast message to topic '{}'", topic);
                            // Record REAL broadcast success for health_check()
                            *self.last_broadcast_success.lock().await = Some(std::time::Instant::now());
                            Ok(())
                        }
                        Err(e) => {
                            println!("[IROH-STREAM] ✗ Failed to broadcast message to topic '{}': {}", topic, e);
                            Err(format!("Gossip broadcast failed: {}", e))
                        }
                    };
                    // Report the REAL broadcast result to the publisher (if it asked)
                    if let Some(ack) = ack {
                        let _ = ack.send(result);
                    }
                }
            }
        }
    }

    /// Check if a topic is subscribed with a LIVE stream handler.
    /// A closed broadcast channel means the handler task exited (or panicked);
    /// such a zombie entry is removed and reported as not-subscribed so
    /// recover()/subscribe paths re-subscribe instead of skipping forever.
    pub async fn is_topic_subscribed(&self, topic: &str) -> bool {
        let mut topics_guard = self.topics.lock().await;
        match topics_guard.get(topic) {
            Some(sub) if !sub.broadcast_tx.is_closed() => true,
            Some(_) => {
                println!(
                    "[IROH] Topic '{}' has a dead stream handler - removing zombie entry",
                    topic
                );
                topics_guard.remove(topic);
                false
            }
            None => false,
        }
    }

    /// Unsubscribe from a gossip topic
    /// This cleanly removes the subscription and stops the message handler
    #[allow(dead_code)]
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

    /// Join a peer into the EXISTING content-topic mesh by calling join_peers()
    /// on the current subscription's gossip sender - the same mechanism the
    /// stream handler startup and dial_and_join_friend() use. Unlike the old
    /// join_gossip_mesh_with_peer(), this does NOT drop and replace the
    /// CONTENT_TOPIC subscription, which left the old stream handler racing
    /// the new one (duplicate handlers, NeighborDown loops).
    pub async fn add_peer_to_content_topic(&self, peer_id: iroh::EndpointId) -> Result<(), String> {
        println!(
            "[IROH-JOIN] Joining peer {} into the content-topic mesh...",
            peer_id
        );

        // Clone the sender out of the topics mutex, then drop the guard BEFORE
        // awaiting - publish_message needs this mutex, so holding it across
        // join_peers would stall all publishing.
        let sender = {
            let topics_guard = self.topics.lock().await;
            topics_guard
                .get(CONTENT_TOPIC)
                .map(|subscription| subscription.gossip_sender.clone())
        };
        let sender = sender.ok_or_else(|| format!("Not subscribed to topic: {}", CONTENT_TOPIC))?;

        match tokio::time::timeout(
            tokio::time::Duration::from_secs(10),
            sender.join_peers(vec![peer_id]),
        )
        .await
        {
            Ok(Ok(_)) => {
                self.add_connected_peer(peer_id).await;
                println!("[IROH-JOIN] [OK] Joined gossip mesh with peer {}", peer_id);
                Ok(())
            }
            Ok(Err(e)) => Err(format!("Failed to join gossip mesh with peer: {}", e)),
            Err(_) => Err("Timed out joining gossip mesh after 10s".to_string()),
        }
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

    /// Publish a message to a gossip topic.
    /// Waits for the REAL broadcast result from the stream handler; on failure
    /// or timeout the message is persisted to the pending_messages outbox so
    /// retry_pending_messages() can re-send it once connectivity returns.
    pub async fn publish_message(&self, topic: &str, message: P2PMessage) -> Result<(), String> {
        self.publish_message_inner(topic, message, false, true)
            .await
    }

    /// Publish a message to direct gossip neighbors only (not relayed mesh-wide).
    /// Use for connection-liveness traffic like heartbeats that has no business
    /// being flooded to - and recorded by - every node in the mesh.
    /// Never queued for retry: this traffic is periodic and replaying stale
    /// copies later is pointless.
    pub async fn publish_message_to_neighbors(
        &self,
        topic: &str,
        message: P2PMessage,
    ) -> Result<(), String> {
        self.publish_message_inner(topic, message, true, false)
            .await
    }

    async fn publish_message_inner(
        &self,
        topic: &str,
        message: P2PMessage,
        neighbors_only: bool,
        queue_on_failure: bool,
    ) -> Result<(), String> {
        println!("[IROH] publish_message() called for topic: {}", topic);

        // Serialize message
        println!("[IROH] Serializing message...");
        let content_json = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        let data_size = content_json.len();
        println!("[IROH] Message serialized, size: {} bytes", data_size);

        // DISTINCT oversize error: an oversize message can never broadcast, so
        // fail it here (and never queue it) instead of reporting a generic
        // broadcast failure.
        if data_size > GOSSIP_MAX_MESSAGE_SIZE {
            let err = format!(
                "message exceeds gossip max size ({} > {})",
                data_size, GOSSIP_MAX_MESSAGE_SIZE
            );
            eprintln!("[IROH] ✗ {}", err);
            return Err(err);
        }

        match self
            .send_and_confirm_broadcast(
                topic,
                Bytes::from(content_json.clone().into_bytes()),
                neighbors_only,
            )
            .await
        {
            Ok(()) => Ok(()),
            Err(e) if !queue_on_failure => Err(e),
            Err(e) => {
                // Persist to the outbox so retry_pending_messages() can re-send.
                // Only SealedEnvelope carries real content; DeviceSyncRequest and
                // Heartbeat are periodic and freshness-checked by receivers, so
                // replaying stale copies would just be rejected.
                let queued = match &message {
                    P2PMessage::SealedEnvelope { .. } => {
                        // Cap the outbox so a long outage (e.g. periodic sealed
                        // presence failing every cycle) can't grow it unboundedly
                        const MAX_QUEUED_MESSAGES: i64 = 500;
                        let pending = self.db.get_pending_message_count(self.user_id).unwrap_or(0);
                        if pending >= MAX_QUEUED_MESSAGES {
                            println!(
                                "[QUEUE] Outbox full ({} pending) - not queueing failed message",
                                pending
                            );
                            false
                        } else {
                            match self.db.queue_pending_message(
                                self.user_id,
                                "sealed_envelope",
                                &content_json,
                                10,
                            ) {
                                Ok(_) => true,
                                Err(qe) => {
                                    eprintln!("[QUEUE] Failed to queue message for retry: {}", qe);
                                    false
                                }
                            }
                        }
                    }
                    _ => false,
                };
                if queued {
                    Err(format!("broadcast failed; message queued for retry: {}", e))
                } else {
                    Err(format!("broadcast failed: {}", e))
                }
            }
        }
    }

    /// Send a serialized message to the topic's stream handler and wait for
    /// the REAL broadcast result. Returns Err on broadcast failure, on a dead
    /// handler, or after a 10s confirmation timeout.
    async fn send_and_confirm_broadcast(
        &self,
        topic: &str,
        data: Bytes,
        neighbors_only: bool,
    ) -> Result<(), String> {
        let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();

        // Hold the topics lock only long enough to enqueue - never across the await
        {
            let topics_guard = self.topics.lock().await;
            let subscription = topics_guard
                .get(topic)
                .ok_or_else(|| format!("Not subscribed to topic: {}", topic))?;
            subscription
                .broadcast_tx
                .send(OutgoingMessage {
                    data,
                    neighbors_only,
                    ack: Some(ack_tx),
                })
                .map_err(|e| format!("Failed to send to broadcast channel: {}", e))?;
        }
        println!(
            "[IROH] Message queued for broadcast to topic: {} - awaiting confirmation",
            topic
        );

        match tokio::time::timeout(tokio::time::Duration::from_secs(10), ack_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(
                "broadcast not confirmed: stream handler exited before broadcasting".to_string(),
            ),
            Err(_) => Err("broadcast confirmation timed out after 10s".to_string()),
        }
    }

    // ============================================================================
    // BLOB TRANSFER FUNCTIONS
    // For large files (images, attachments) that exceed gossip message size limits
    // ============================================================================

    /// Register a known peer address so gossip and direct dials can resolve it by
    /// EndpointId. Replaces iroh 0.35's `endpoint.add_node_addr()`.
    pub fn register_peer_address(&self, addr: iroh::EndpointAddr) {
        self.address_book.add_endpoint_info(addr);
    }

    /// Store a blob locally and return its hash
    /// The hash can then be shared via gossip, and peers can fetch the blob directly
    pub async fn store_blob(&self, data: Vec<u8>) -> Result<iroh_blobs::Hash, String> {
        let store_guard = self.store.lock().await;
        let store = store_guard.as_ref().ok_or("Blob store not initialized")?;

        // Add the bytes to the store (raw format). AddProgress resolves to TagInfo.
        let tag = store
            .blobs()
            .add_bytes(data)
            .await
            .map_err(|e| format!("Failed to import blob: {}", e))?;

        let hash = tag.hash;
        println!("[IROH-BLOBS] Stored blob with hash: {}", hash);

        Ok(hash)
    }

    /// Fetch a blob from a peer by hash
    /// Uses the existing NAT-traversed connection for efficient transfer
    pub async fn fetch_blob(
        &self,
        hash: iroh_blobs::Hash,
        from_node_id: iroh::EndpointId,
    ) -> Result<Vec<u8>, String> {
        println!(
            "[IROH-BLOBS] Fetching blob {} from peer {}",
            hash, from_node_id
        );

        // Clone the downloader so we don't hold the lock across the download.
        let downloader = self
            .downloader
            .lock()
            .await
            .clone()
            .ok_or("Downloader not initialized")?;

        // Download the blob from the given provider. The downloader establishes
        // (or reuses) a connection to the peer via the endpoint and address lookups.
        downloader
            .download(hash, vec![from_node_id])
            .await
            .map_err(|e| format!("Download failed: {}", e))?;

        println!("[IROH-BLOBS] Download complete, reading blob from store");

        // Read the blob back out of the local store.
        let store_guard = self.store.lock().await;
        let store = store_guard.as_ref().ok_or("Blob store not initialized")?;
        let data = store
            .blobs()
            .get_bytes(hash)
            .await
            .map_err(|e| format!("Failed to read blob: {}", e))?;

        println!("[IROH-BLOBS] [OK] Fetched blob, size: {} bytes", data.len());
        Ok(data.to_vec())
    }

    /// Decrypt a blob stored in the standard attachment wire format:
    /// [24-byte XChaCha20Poly1305 nonce][ciphertext], key base64-encoded.
    /// Inverse of the encryption in build_blob_refs_for_post().
    pub(crate) fn decrypt_blob_data(
        encrypted_data: &[u8],
        key_b64: &str,
    ) -> Result<Vec<u8>, String> {
        use base64::Engine as _;
        use chacha20poly1305::aead::{Aead, KeyInit};
        use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(key_b64)
            .map_err(|e| format!("Invalid encryption key: {}", e))?;
        if key_bytes.len() != 32 {
            return Err("Encryption key must be 32 bytes".to_string());
        }
        if encrypted_data.len() < 24 {
            return Err("Encrypted blob too short (missing nonce)".to_string());
        }
        let (nonce_bytes, ciphertext) = encrypted_data.split_at(24);
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
        cipher
            .decrypt(XNonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| "Blob decryption failed - invalid key or corrupted data".to_string())
    }

    /// Download a received post's encrypted attachment blobs from the sender,
    /// decrypt them, and persist them as media_attachments so the feed can
    /// render them. Best-effort per attachment: a failed blob drops that
    /// attachment (with a log), never the post.
    async fn fetch_and_store_post_attachments(
        &self,
        post_id: super::types::SqliteUuid,
        owner_id: super::types::SqliteUuid,
        node_id: &str,
        blob_refs: &[super::types::BlobReference],
    ) {
        let sender_node_id = match node_id.parse::<iroh::EndpointId>() {
            Ok(id) => id,
            Err(e) => {
                println!(
                    "[IROH-BLOBS] Cannot fetch {} attachment(s) for post {}: invalid sender node_id: {}",
                    blob_refs.len(),
                    post_id,
                    e
                );
                return;
            }
        };

        for blob_ref in blob_refs {
            // VALIDATION: file_type is a peer-supplied string that ends up in
            // the media row the UI renders from. Only fetch/store types we
            // actually support.
            if !is_supported_media_type(&blob_ref.file_type) {
                println!(
                    "[IROH-BLOBS] Skipping attachment {}: unsupported file type '{}'",
                    blob_ref.id, blob_ref.file_type
                );
                continue;
            }
            let hash_bytes = match hex::decode(&blob_ref.blob_hash) {
                Ok(b) if b.len() == 32 => b,
                Ok(_) => {
                    println!(
                        "[IROH-BLOBS] Skipping attachment {}: blob hash must be 32 bytes",
                        blob_ref.id
                    );
                    continue;
                }
                Err(e) => {
                    println!(
                        "[IROH-BLOBS] Skipping attachment {}: invalid blob hash: {}",
                        blob_ref.id, e
                    );
                    continue;
                }
            };
            let mut hash_arr = [0u8; 32];
            hash_arr.copy_from_slice(&hash_bytes);
            let hash = iroh_blobs::Hash::from_bytes(hash_arr);

            let encrypted_data = match tokio::time::timeout(
                tokio::time::Duration::from_secs(30),
                self.fetch_blob(hash, sender_node_id),
            )
            .await
            {
                Ok(Ok(data)) => data,
                Ok(Err(e)) => {
                    println!(
                        "[IROH-BLOBS] Failed to fetch attachment {} for post {}: {}",
                        blob_ref.id, post_id, e
                    );
                    continue;
                }
                Err(_) => {
                    println!(
                        "[IROH-BLOBS] Timed out fetching attachment {} for post {}",
                        blob_ref.id, post_id
                    );
                    continue;
                }
            };

            let data = match &blob_ref.encryption_key {
                Some(key_b64) => match Self::decrypt_blob_data(&encrypted_data, key_b64) {
                    Ok(plaintext) => plaintext,
                    Err(e) => {
                        println!(
                            "[IROH-BLOBS] Failed to decrypt attachment {} for post {}: {}",
                            blob_ref.id, post_id, e
                        );
                        continue;
                    }
                },
                // Legacy or unencrypted blob
                None => encrypted_data,
            };

            match self.db.upload_media(
                owner_id,
                post_id,
                "synced_file",
                &blob_ref.file_type,
                &data,
                blob_ref.file_size,
            ) {
                Ok(_) => println!(
                    "[IROH-BLOBS] [OK] Saved attachment {} for post {}",
                    blob_ref.id, post_id
                ),
                Err(e) => println!(
                    "[IROH-BLOBS] Failed to save attachment {} for post {}: {}",
                    blob_ref.id, post_id, e
                ),
            }
        }
    }

    /// Check if we have a blob locally
    #[allow(dead_code)]
    pub async fn has_blob(&self, hash: iroh_blobs::Hash) -> bool {
        // Clone the store handle to avoid holding the lock during await
        let store_clone = self.store.lock().await.clone();
        if let Some(store) = store_clone.as_ref() {
            store.blobs().has(hash).await.unwrap_or(false)
        } else {
            false
        }
    }

    /// Get our node ID as a string for blob fetching
    pub async fn get_node_id(&self) -> String {
        // Clone endpoint to avoid holding lock - prevents potential deadlocks
        let endpoint_clone = self.endpoint.lock().await.clone();
        if let Some(endpoint) = endpoint_clone.as_ref() {
            endpoint.id().to_string()
        } else {
            String::new()
        }
    }

    /// Announce presence to the global mesh
    pub async fn announce_presence(&self) -> Result<(), String> {
        // Get our full NodeAddr from the endpoint
        // Clone endpoint to avoid holding lock during await
        let endpoint_clone = self.endpoint.lock().await.clone();
        let node_addr = if let Some(endpoint) = endpoint_clone.as_ref() {
            endpoint.addr()
        } else {
            return Err("Endpoint not initialized".to_string());
        };

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

        // Get encryption public key for sealed envelopes (comments, reactions, etc.)
        let encryption_public_key = self.get_user_encryption_public_key();

        // Presence carries our profile and network addresses (IPs), so it is
        // SEALED to friends plus our own devices - it used to be broadcast in
        // plaintext to the entire mesh, handing every node a live directory of
        // users, their IPs, and their online times.
        let mut recipients = self.get_friend_encryption_public_keys();
        if let Some(own_key) = encryption_public_key.clone() {
            if !recipients.contains(&own_key) {
                recipients.push(own_key);
            }
        }
        if recipients.is_empty() {
            println!("[IROH] No presence recipients (no encryption key yet) - skipping");
            return Ok(());
        }
        let signing_key = user
            .private_key
            .clone()
            .ok_or_else(|| "No signing private key for presence".to_string())?;

        let node_addr_json = serde_json::to_string(&node_addr)
            .map_err(|e| format!("Failed to serialize node address: {}", e))?;

        // Rotate the pre-key if it's due, then piggyback our current pre-key so
        // friends stay current even if a KeyRotation envelope was lost
        self.ensure_prekey_and_maybe_rotate(&signing_key).await;
        let (prekey_public, prekey_signature, prekey_created_at) =
            match self.db.get_current_prekey(self.user_id) {
                Some(pk) => (Some(pk.public_key), Some(pk.signature), pk.created_at),
                None => (None, None, 0),
            };

        let payload = super::crypto::ContentPayload::Presence {
            device_id: self
                .device_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            node_addr_json,
            encryption_public_key,
            display_name,
            bio,
            profile_picture,
            profile_signature,
            prekey_public,
            prekey_signature,
            prekey_created_at,
            sent_at: chrono::Utc::now().timestamp(),
        };

        let envelope = super::crypto::GossipEnvelope::seal(
            &payload,
            &recipients,
            &self.public_key,
            &signing_key,
        )?;
        let envelope_json = serde_json::to_string(&envelope)
            .map_err(|e| format!("Failed to serialize presence envelope: {}", e))?;

        self.publish_message(CONTENT_TOPIC, P2PMessage::SealedEnvelope { envelope_json })
            .await
    }

    /// Start background loop for presence announcements
    fn start_presence_loop(&self) {
        let network = Arc::new(self.clone_for_background());
        let shutdown_flag = self.shutdown_flag.clone();
        let last_success = self.last_presence_success.clone();

        let handle = tokio::spawn(async move {
            // Adaptive cadence: announce every 30s during a warm-up window (when
            // peers are still discovering each other and the mesh is settling),
            // then back off to every 2 minutes once stable. Presence is now
            // sealed, so the steady-state cost is bandwidth/battery, not privacy;
            // the warm-up keeps reconnection and pre-key propagation snappy.
            const WARMUP_SECS: u64 = 30;
            const STEADY_SECS: u64 = 120;
            const WARMUP_CYCLES: u32 = 10; // ~5 minutes of fast announcements
            let mut cycle: u32 = 0;
            loop {
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

                // CRITICAL: Also retry FriendAccepted messages every cycle
                // This ensures FriendAccepted is delivered even if mesh was unstable initially
                network.resend_friend_accepted_if_needed().await;

                // Announce immediately on the first pass (like the old
                // interval), then wait before the next one
                let delay = if cycle < WARMUP_CYCLES {
                    WARMUP_SECS
                } else {
                    STEADY_SECS
                };
                cycle = cycle.saturating_add(1);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
            }
        });
        self.track_background_task(handle);

        println!("[IROH] Started presence announcement loop (includes FriendAccepted retry)");
    }

    /// Start periodic device sync loop
    /// Regularly requests sync from other devices with same user account
    /// Ensures posts, comments, reactions stay in sync even if presence detection is delayed
    fn start_periodic_device_sync(&self) {
        let network = Arc::new(self.clone_for_background());
        let shutdown_flag = self.shutdown_flag.clone();

        let handle = tokio::spawn(async move {
            // Initial delay before first sync request
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

            // Sync every 30 seconds
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Check if we should stop
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("[IROH-SYNC] Periodic sync loop stopping due to shutdown flag");
                    break;
                }

                // Request sync from other same-user devices - but only when we
                // actually know of another own device. Single-device users were
                // broadcasting a DeviceSyncRequest mesh-wide every 30s for nothing.
                if network.has_other_own_devices() {
                    network.request_device_sync().await;
                } else {
                    println!(
                        "[IROH-SYNC] No other devices known for this account - skipping periodic sync request"
                    );
                }
            }
        });
        self.track_background_task(handle);

        println!("[IROH] Started periodic device sync loop (every 30s)");
    }

    /// Actively discover peers using Pkarr rendezvous DHT key
    /// All Cipher nodes publish to and query from a well-known rendezvous point
    fn start_active_peer_discovery(&self) {
        let network = Arc::new(self.clone_for_background());
        let shutdown_flag = self.shutdown_flag.clone();

        let handle = tokio::spawn(async move {
            // Brief delay to allow endpoint initialization (reduced from 5s for faster reconnection)
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            println!("[IROH-RENDEZVOUS] Starting Pkarr-based peer discovery...");

            // Clone the endpoint so the lock isn't held across awaits
            let endpoint_clone = network.endpoint.lock().await.clone();
            if let Some(endpoint) = endpoint_clone.as_ref() {
                let our_node_id = endpoint.id();
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

                // Get our EndpointAddr to publish
                let our_node_addr = endpoint.addr();
                println!("[IROH-RENDEZVOUS] Our EndpointAddr: {:?}", our_node_addr);
                println!(
                    "[IROH-RENDEZVOUS]   Relay: {:?}",
                    our_node_addr.relay_urls().next()
                );
                println!(
                    "[IROH-RENDEZVOUS]   Direct addrs: {}",
                    our_node_addr.ip_addrs().count()
                );
            }

            // Continuously try database peers with EXPONENTIAL BACKOFF
            // Reduces battery drain and network spam for unreachable peers
            // Work-first ordering: the first reconnect pass runs ~1s after start
            // (important after resume) instead of waiting a full interval.
            let interval = tokio::time::Duration::from_secs(10);
            loop {
                // Check if we should stop (recover()/shutdown() also abort this task directly)
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("[IROH-DISCOVERY] Discovery loop stopping due to shutdown flag");
                    break;
                }

                // Check all friend peer addresses (NodeId + relay URL) in database and try to connect
                if let Ok(peer_addrs) = network.db.get_all_friend_peer_addresses(network.user_id) {
                    // Prune retry bookkeeping for peers no longer in the friends
                    // list so removed friends don't accumulate stale entries
                    {
                        let current_peers: std::collections::HashSet<iroh::EndpointId> = peer_addrs
                            .iter()
                            .filter_map(|(id_str, _)| id_str.parse::<iroh::EndpointId>().ok())
                            .collect();
                        let mut retry_states = network.peer_retry_counts.lock().await;
                        let before = retry_states.len();
                        retry_states.retain(|peer_id, _| current_peers.contains(peer_id));
                        let pruned = before - retry_states.len();
                        if pruned > 0 {
                            println!(
                                "[IROH-DISCOVERY] Pruned {} retry entries for peers no longer in friends list",
                                pruned
                            );
                        }
                    }

                    if !peer_addrs.is_empty() {
                        println!("[IROH-DISCOVERY] Found {} peer addresses in database, checking for reconnection candidates...", peer_addrs.len());

                        // Clone the endpoint and release the lock immediately.
                        // Holding it across the connect() await stalled presence,
                        // heartbeats and the gossip stream handler for the entire
                        // attempt (tens of seconds for unreachable peers).
                        let endpoint_clone = network.endpoint.lock().await.clone();
                        if let Some(endpoint) = endpoint_clone {
                            let our_node_id = endpoint.id();

                            // First pass: gather dial candidates, respecting the
                            // per-peer exponential backoff bookkeeping
                            let mut candidates: Vec<(iroh::EndpointId, String)> = Vec::new();
                            for (node_id_str, relay_url) in peer_addrs {
                                match node_id_str.parse::<iroh::EndpointId>() {
                                    Ok(peer_id) => {
                                        // CRITICAL: Skip already-connected peers!
                                        // Reconnecting to already-connected peers causes gossip state
                                        // to be reset, breaking message delivery.
                                        if network.is_peer_connected(peer_id).await {
                                            // Peer is already connected and in gossip mesh - skip
                                            continue;
                                        }

                                        // Skip if it's our own NodeId
                                        if peer_id == our_node_id {
                                            continue;
                                        }

                                        // CHECK EXPONENTIAL BACKOFF before attempting connection
                                        let mut retry_states =
                                            network.peer_retry_counts.lock().await;
                                        let retry_state = retry_states
                                            .entry(peer_id)
                                            .or_insert_with(PeerRetryState::new);

                                        if !retry_state.should_retry() {
                                            let backoff_delay = retry_state.backoff_delay_secs();
                                            println!(
                                                "[IROH-DISCOVERY] Skipping peer {} - in backoff (attempt {}, next retry in {}s)",
                                                peer_id, retry_state.attempt_count + 1, backoff_delay
                                            );
                                            drop(retry_states);
                                            continue;
                                        }

                                        // Time to retry - record attempt
                                        retry_state.record_attempt();
                                        println!(
                                            "[IROH-DISCOVERY] Attempting to connect to peer {} (attempt {})...",
                                            peer_id, retry_state.attempt_count
                                        );
                                        drop(retry_states);

                                        candidates.push((peer_id, relay_url));
                                    }
                                    Err(e) => {
                                        println!(
                                            "[IROH-DISCOVERY] Failed to parse NodeId '{}': {}",
                                            node_id_str, e
                                        );
                                    }
                                }
                            }

                            // Second pass: dial all candidates CONCURRENTLY.
                            // Serial dialing (15s timeout each) made reconnect
                            // latency scale linearly with offline-friend count.
                            let mut dials = tokio::task::JoinSet::new();
                            for (peer_id, relay_url) in candidates {
                                let network = network.clone();
                                let endpoint = endpoint.clone();
                                dials.spawn(async move {
                                    network
                                        .dial_and_join_friend(&endpoint, peer_id, &relay_url)
                                        .await;
                                });
                            }
                            while dials.join_next().await.is_some() {}
                        }
                    }
                }

                tokio::time::sleep(interval).await;
            }
        });
        self.track_background_task(handle);

        println!("[IROH] Started Pkarr rendezvous + database peer discovery");
    }

    /// Dial a disconnected friend, hand the QUIC connection to the gossip
    /// layer, and join the content-topic mesh. Called concurrently per peer by
    /// the discovery loop.
    async fn dial_and_join_friend(
        &self,
        endpoint: &iroh::Endpoint,
        peer_id: iroh::EndpointId,
        relay_url: &str,
    ) {
        // Construct full EndpointAddr with relay URL for reliable connection
        let mut peer_node_addr = iroh::EndpointAddr::new(peer_id);
        if let Ok(url) = relay_url.parse() {
            peer_node_addr = peer_node_addr.with_relay_url(url);
            println!(
                "[IROH-DISCOVERY] Connecting to peer {} with relay: {}",
                peer_id, relay_url
            );
        } else {
            println!(
                "[IROH-DISCOVERY] Warning: Invalid relay URL for peer {}",
                peer_id
            );
            return;
        }

        // Register the peer's address so gossip can resolve
        // it by EndpointId. Replaces 0.35 add_node_addr.
        self.address_book.add_endpoint_info(peer_node_addr.clone());

        // Try to connect with full addressing info.
        // Bound the attempt so one unreachable peer can't hang forever.
        println!(
            "[IROH-DISCOVERY] Discovering peer {} via DHT/DNS/Relay...",
            peer_id
        );
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(15),
            endpoint.connect(peer_node_addr.clone(), iroh_gossip::ALPN),
        )
        .await
        {
            Ok(Ok(conn)) => {
                println!("[IROH-DISCOVERY] [OK] Connected to peer {}!", peer_id);

                // Check if already connected - skip if so
                let was_connected = self.is_peer_connected(peer_id).await;
                self.add_connected_peer(peer_id).await;

                // RESET backoff counter on successful connection
                let mut retry_states = self.peer_retry_counts.lock().await;
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
                //
                // Clone the Gossip handle OUT of the mutex before awaiting -
                // holding the guard across the await blocked every other
                // gossip user (including subscribe/publish) for its duration.
                let gossip_clone = self.gossip.lock().await.clone();
                if let Some(gossip) = gossip_clone {
                    match gossip.handle_connection(conn).await {
                        Ok(_) => println!(
                            "[IROH-DISCOVERY] [OK] Handed connection to gossip layer for peer {}",
                            peer_id
                        ),
                        Err(e) => println!(
                            "[IROH-DISCOVERY] Warning: Failed to hand connection to gossip: {}",
                            e
                        ),
                    }
                }

                // CRITICAL: Call join_peers() on the gossip sender to form mesh
                // This is required for OUTGOING connections to establish
                // a gossip neighbor relationship. Without this, messages
                // won't be exchanged even though QUIC is connected.
                //
                // Clone the sender out of the topics mutex, then drop the guard
                // BEFORE awaiting - publish_message needs this mutex, so holding
                // it across join_peers stalled all publishing.
                let sender = {
                    let topics_guard = self.topics.lock().await;
                    topics_guard
                        .get(CONTENT_TOPIC)
                        .map(|subscription| subscription.gossip_sender.clone())
                };
                if let Some(sender) = sender {
                    match sender.join_peers(vec![peer_id]).await {
                        Ok(_) => println!(
                            "[IROH-DISCOVERY] [OK] Joined gossip mesh with peer {}",
                            peer_id
                        ),
                        Err(e) => println!(
                            "[IROH-DISCOVERY] Warning: Failed to join gossip mesh with peer: {}",
                            e
                        ),
                    }
                }

                if !was_connected {
                    println!("[IROH-DISCOVERY] NEW peer {} connected!", peer_id);

                    // RETRY pending messages when connection restored
                    println!("[IROH-DISCOVERY] Connection restored - attempting to resend pending messages...");
                    self.retry_pending_messages().await;

                    // CRITICAL FIX: Check if this peer initiated a friendship we've accepted
                    // If so, resend FriendAccepted to ensure they know we accepted
                    self.resend_friend_accepted_if_needed().await;
                } else {
                    println!(
                        "[IROH-DISCOVERY] Peer {} already tracked - refreshed gossip connection",
                        peer_id
                    );
                }
            }
            Ok(Err(e)) => {
                println!(
                    "[IROH-DISCOVERY] Failed to connect to {} (attempt {}): {}",
                    peer_id,
                    {
                        let retry_states = self.peer_retry_counts.lock().await;
                        retry_states
                            .get(&peer_id)
                            .map(|s| s.attempt_count)
                            .unwrap_or(0)
                    },
                    e
                );
            }
            Err(_) => {
                println!(
                    "[IROH-DISCOVERY] Connect to {} timed out after 15s",
                    peer_id
                );
            }
        }
    }

    /// Handle received P2P message.
    /// `delivered_from` is the AUTHENTICATED gossip neighbor the message
    /// arrived from (from the QUIC connection) - unlike any node id embedded
    /// in the payload, it cannot be spoofed.
    /// `msg_hash` is the raw-message dedup hash the stream loop recorded for
    /// this delivery; a failed envelope handler removes it so a redelivered
    /// copy gets reprocessed instead of being deduped.
    async fn handle_message(
        &self,
        message: P2PMessage,
        delivered_from: iroh::EndpointId,
        msg_hash: u64,
    ) {
        match message {
            P2PMessage::DeviceSyncRequest {
                public_key,
                device_id,
                last_sync_timestamp,
                timestamp,
                signature,
            } => {
                // Check if this is from another device with same user
                if public_key == self.public_key
                    && device_id != self.device_id.clone().unwrap_or_default()
                {
                    // AUTHENTICITY: public keys are broadcast in Presence, so a
                    // matching public_key proves nothing - anyone could send
                    // this request. Only our own devices hold the signing key.
                    let context = sync_request_context(
                        &public_key,
                        &device_id,
                        last_sync_timestamp,
                        timestamp,
                    );
                    if !Database::verify_signature(&context, &signature, &self.public_key) {
                        println!(
                            "[IROH] Rejecting device sync request from {}: invalid signature",
                            device_id
                        );
                        return;
                    }
                    // Freshness window limits replaying a captured request
                    let now = chrono::Utc::now().timestamp();
                    if (now - timestamp).abs() > 300 {
                        println!(
                            "[IROH] Rejecting device sync request from {}: stale timestamp",
                            device_id
                        );
                        return;
                    }

                    println!(
                        "[IROH] Received verified device sync request from device {}",
                        device_id
                    );

                    // STATELESS RESPONDER: send everything newer than the
                    // REQUESTER's watermark. The old code ignored the request
                    // timestamp and gated on our own per-device sync_state
                    // cursors, which applying an incoming sync advanced - so
                    // pre-existing rows were never sent back.
                    let our_device_id = self
                        .device_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let since_rfc3339 =
                        chrono::DateTime::from_timestamp(last_sync_timestamp.max(0), 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_else(|| "1970-01-01T00:00:00+00:00".to_string());
                    match self.db.get_sync_data(self.user_id, &since_rfc3339) {
                        Ok(sync_data) => {
                            println!(
                                "[IROH] Got sync data since {}: {} posts, {} messages, {} friends, {} comments, {} reactions",
                                since_rfc3339,
                                sync_data.posts.len(),
                                sync_data.messages.len(),
                                sync_data.friends.len(),
                                sync_data.comments.len(),
                                sync_data.reactions.len()
                            );
                            if sync_data.is_empty() {
                                println!(
                                    "[IROH] Nothing newer than requester's watermark - no sync response needed"
                                );
                                return;
                            }

                            // Seal the sync data to our OWN encryption key: all
                            // devices of this user share the keypair, so only
                            // they can read it. The old plaintext response
                            // broadcast the entire database to the whole mesh.
                            let Some(our_encryption_public_key) =
                                self.get_user_encryption_public_key()
                            else {
                                println!("[IROH] No encryption public key - cannot send sync");
                                return;
                            };
                            let Some(our_signing_private_key) = self.get_user_signing_private_key()
                            else {
                                println!("[IROH] No signing private key - cannot send sync");
                                return;
                            };

                            // CHUNKED RESPONSE: one envelope with the whole
                            // delta silently exceeded the gossip message cap
                            // and was undeliverable. Each chunk stays under a
                            // pre-sealing budget (sealing adds base64, padding
                            // and key boxes) and is a self-contained SyncData
                            // that can be applied independently.
                            let chunks = sync_data.into_chunks(SYNC_CHUNK_BUDGET_BYTES);
                            let chunk_count = chunks.len();
                            for (i, chunk) in chunks.into_iter().enumerate() {
                                let data_json = match serde_json::to_string(&chunk) {
                                    Ok(j) => j,
                                    Err(e) => {
                                        println!("[IROH] Failed to serialize sync chunk: {}", e);
                                        continue;
                                    }
                                };

                                match super::crypto::GossipEnvelope::new_device_sync(
                                    &self.public_key,
                                    &our_device_id,
                                    &data_json,
                                    &our_encryption_public_key,
                                    &our_signing_private_key,
                                ) {
                                    Ok(envelope) => match serde_json::to_string(&envelope) {
                                        Ok(envelope_json) => {
                                            let response =
                                                P2PMessage::SealedEnvelope { envelope_json };
                                            if let Err(e) =
                                                self.publish_message(CONTENT_TOPIC, response).await
                                            {
                                                println!(
                                                    "[IROH] Failed to send device sync chunk {}/{}: {}",
                                                    i + 1,
                                                    chunk_count,
                                                    e
                                                );
                                            } else {
                                                println!(
                                                    "[IROH] Sent encrypted device sync chunk {}/{} to device {}",
                                                    i + 1,
                                                    chunk_count,
                                                    device_id
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            println!("[IROH] Failed to serialize envelope: {}", e)
                                        }
                                    },
                                    Err(e) => {
                                        println!("[IROH] Failed to create sync envelope: {}", e)
                                    }
                                }
                            }
                        }
                        Err(e) => println!("[IROH] Failed to get sync data: {}", e),
                    }
                }
            }

            // PostWithBlobs removed - all posts use SealedEnvelope (encrypted)
            P2PMessage::SealedEnvelope { envelope_json } => {
                // PHASE 2: Sealed box encryption
                // Try to decrypt the envelope using our encryption private key
                println!("[IROH] Received SealedEnvelope - attempting to decrypt...");

                // Candidate decryption keys: our static identity key plus our
                // current + previous rotating pre-keys. Senders seal to whichever
                // of our keys they know is freshest, so we must try them all.
                let mut candidate_keys = self.db.get_prekey_private_keys(self.user_id);
                match self.get_user_encryption_private_key() {
                    Some(key) => candidate_keys.push(key),
                    None if candidate_keys.is_empty() => {
                        println!("[IROH] No decryption keys found, cannot decrypt");
                        return;
                    }
                    None => {}
                }

                // Parse the envelope
                let envelope: super::crypto::GossipEnvelope =
                    match serde_json::from_str(&envelope_json) {
                        Ok(env) => env,
                        Err(e) => {
                            println!("[IROH] Failed to parse envelope: {}", e);
                            return;
                        }
                    };

                // STALENESS: envelopes are only valid within the purge window;
                // anything older is a replay or long-delayed duplicate
                let now = chrono::Utc::now().timestamp();
                if envelope.timestamp < now - 7 * 24 * 3600 {
                    println!("[IROH] Ignoring stale envelope (older than 7 days)");
                    return;
                }

                // REPLAY PROTECTION: persistently track processed message_ids -
                // the in-memory gossip dedup doesn't survive restarts, so an
                // attacker could otherwise replay recorded envelopes (e.g. undo
                // a reaction removal) after we restart.
                // READ-ONLY check: the id is recorded as seen only after the
                // payload handler SUCCEEDS (or the envelope isn't for us) -
                // marking it before processing meant a transient handler
                // failure permanently discarded the envelope.
                match self.db.is_envelope_seen(&envelope.message_id) {
                    Ok(false) => {} // first time we see this envelope
                    Ok(true) => {
                        println!(
                            "[IROH] Ignoring already-processed envelope {}",
                            &envelope.message_id[..8.min(envelope.message_id.len())]
                        );
                        return;
                    }
                    // Fail open: a broken replay table shouldn't stop content
                    Err(e) => println!("[IROH] Warning: replay check failed: {}", e),
                }

                // Trial-decrypt the key boxes (no recipient hints on the wire)
                // with each candidate key. The sender's identity comes back
                // AUTHENTICATED from inside the ciphertext - the wire carries
                // no sender metadata.
                let decrypted_opt = candidate_keys.iter().find_map(|k| envelope.try_decrypt(k));
                match decrypted_opt {
                    Some(decrypted) => {
                        let sender_public_key = decrypted.sender_public_key;
                        println!(
                            "[IROH] [OK] Successfully decrypted envelope from {}",
                            sender_public_key
                        );

                        // Process the decrypted content. The envelope is only
                        // marked replay-seen AFTER the handler succeeds:
                        // marking it up front meant any transient handler
                        // failure (lock contention, DB error, mid-processing
                        // crash) permanently discarded the envelope, because
                        // gossip redelivery was deduped from then on.
                        match self
                            .handle_content_payload(
                                sender_public_key,
                                envelope.timestamp,
                                decrypted.payload,
                            )
                            .await
                        {
                            Ok(()) => {
                                if let Err(e) = self.db.mark_envelope_seen(&envelope.message_id) {
                                    println!(
                                        "[IROH] Warning: failed to record envelope as seen: {}",
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "[IROH] Handler failed for envelope {}: {} - leaving it unmarked so a redelivered copy is reprocessed",
                                    &envelope.message_id[..8.min(envelope.message_id.len())],
                                    e
                                );
                                // Also forget the raw-message dedup hash, or the
                                // in-memory cache would drop the redelivered copy
                                // before it ever reached this handler again
                                self.recent_message_hashes.lock().await.remove(msg_hash);
                            }
                        }
                    }
                    None => {
                        // No key box decrypted with our key - envelope is for
                        // other recipients; we just relayed it. Safe to mark
                        // seen NOW: we can never process it, and marking skips
                        // the trial-decryption cost on a redelivery.
                        println!("[IROH] Envelope not for us - ignored");
                        if let Err(e) = self.db.mark_envelope_seen(&envelope.message_id) {
                            println!("[IROH] Warning: failed to record envelope as seen: {}", e);
                        }
                    }
                }
            }

            P2PMessage::Heartbeat {
                node_id,
                timestamp: _,
            } => {
                // SECURITY: key liveness bookkeeping on the AUTHENTICATED
                // gossip sender (delivered_from), NOT the embedded node_id.
                // Heartbeats are neighbors-only (never relayed), so the
                // delivering neighbor IS the originator. Trusting the payload
                // let any neighbor refresh liveness for an arbitrary peer and
                // suppress reconnection to it.
                match node_id.parse::<iroh::EndpointId>() {
                    Ok(claimed) if claimed != delivered_from => {
                        println!(
                            "[HEARTBEAT] Heartbeat claims node_id {} but was delivered by {} - using the authenticated sender",
                            claimed, delivered_from
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        println!("[HEARTBEAT] Failed to parse node_id from heartbeat: {}", e);
                    }
                }
                println!(
                    "[HEARTBEAT] Received heartbeat from peer: {}",
                    delivered_from
                );

                // Update last heartbeat time for this peer
                let mut heartbeats = self.peer_heartbeats.lock().await;
                heartbeats.insert(delivered_from, std::time::Instant::now());
                drop(heartbeats);

                // Ensure peer is in connected set (heartbeat confirms they're alive)
                self.add_connected_peer(delivered_from).await;
            }
        }
    }

    /// Process a decrypted sealed-envelope payload.
    ///
    /// Returns Err ONLY for TRANSIENT failures (DB errors, failed sync
    /// applies) - the caller then leaves the envelope unmarked so a gossip
    /// redelivery gets reprocessed. Permanent rejects (unparseable ids,
    /// foreign DeviceSync senders, stale presence) return Ok(()) so the
    /// envelope is marked seen and never retried.
    async fn handle_content_payload(
        &self,
        sender_public_key: String,
        envelope_timestamp: i64,
        payload: super::crypto::ContentPayload,
    ) -> Result<(), String> {
        match payload {
            super::crypto::ContentPayload::Post {
                post_id,
                content,
                node_id,
                blob_refs,
                sent_at,
                is_backfill,
            } => {
                // AUTHORIZATION: a decryptable envelope is not consent - only
                // accepted, non-blocked friends may put content in our feed.
                // Ok(()) so the envelope is marked seen and never retried.
                if !self
                    .is_authorized_content_sender(&sender_public_key, "post")
                    .await
                {
                    return Ok(());
                }
                // Precise time is inside the ciphertext; envelope
                // timestamp (hour-coarse) is the legacy fallback
                let timestamp = if sent_at > 0 {
                    sent_at
                } else {
                    envelope_timestamp
                };
                // Get sender's user_id from their public key
                let sender_user_id = super::types::SqliteUuid::from_public_key(&sender_public_key);

                // Ensure sender exists in database
                // NOTE: Using lock() instead of try_lock() for data integrity
                {
                    let conn = self.db.conn.lock().unwrap();
                    if let Err(e) = conn.execute(
                        "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            sender_user_id,
                            format!("User_{}", &sender_public_key[..8.min(sender_public_key.len())]),
                            &sender_public_key,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to ensure post sender exists: {}", e);
                    }
                }

                // Emit decrypted post to UI via sealed-post-received event
                #[derive(serde::Serialize, Clone)]
                struct DecryptedPostEvent {
                    post_id: String,
                    user_id: super::types::SqliteUuid,
                    public_key: String,
                    node_id: String,
                    content: String,
                    timestamp: i64,
                    blob_refs: Vec<super::types::BlobReference>,
                    is_backfill: bool,
                }

                let _ = self.app_handle.emit(
                    "sealed-post-received",
                    DecryptedPostEvent {
                        post_id,
                        user_id: sender_user_id,
                        public_key: sender_public_key.clone(),
                        node_id,
                        content,
                        timestamp,
                        blob_refs,
                        is_backfill,
                    },
                );
                println!(
                    "[IROH] [OK] Emitted decrypted post to UI (backfill={})",
                    is_backfill
                );
            }
            super::crypto::ContentPayload::DirectMessage {
                message_id,
                content,
                thread_id,
                sent_at,
            } => {
                // AUTHORIZATION: only accepted, non-blocked friends may drop a
                // DM into our conversation list
                if !self
                    .is_authorized_content_sender(&sender_public_key, "direct message")
                    .await
                {
                    return Ok(());
                }
                let timestamp = if sent_at > 0 {
                    sent_at
                } else {
                    envelope_timestamp
                };
                let sender_user_id = super::types::SqliteUuid::from_public_key(&sender_public_key);
                println!(
                    "[IROH] Received encrypted DM from {}: {} chars",
                    sender_public_key,
                    content.len()
                );

                // Ensure sender exists in database
                // NOTE: Using lock() instead of try_lock() for data integrity
                {
                    let conn = self.db.conn.lock().unwrap();
                    if let Err(e) = conn.execute(
                        "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            sender_user_id,
                            format!("User_{}", &sender_public_key[..8.min(sender_public_key.len())]),
                            &sender_public_key,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to ensure DM sender exists: {}", e);
                    }
                }

                // PERSIST the message BEFORE emitting: the event only
                // refreshes the UI - the DB row is what the conversation
                // view actually renders. The old handler only emitted the
                // event, so the message was lost the moment it fired.
                // Dedup by message id (gossip can redeliver).
                let db_message_id = super::types::SqliteUuid::parse_str(&message_id)
                    .unwrap_or_else(|_| super::types::SqliteUuid::new());
                let created_at = chrono::DateTime::from_timestamp(timestamp, 0)
                    .unwrap_or_else(chrono::Utc::now)
                    .to_rfc3339();
                match self.db.store_received_message(
                    db_message_id,
                    sender_user_id,
                    self.user_id,
                    &content,
                    thread_id,
                    &created_at,
                ) {
                    Ok(true) => {
                        println!("[IROH] [OK] Stored incoming DM {}", db_message_id)
                    }
                    Ok(false) => {
                        // Redelivered duplicate: nothing new to show the UI
                        println!(
                            "[IROH] Incoming DM {} already stored - skipping insert",
                            db_message_id
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        println!("[IROH] Failed to store incoming DM: {}", e);
                        // Transient (DB) failure: propagate so the envelope
                        // stays unmarked and a redelivery retries the save
                        return Err(format!("failed to store incoming DM: {}", e));
                    }
                }

                // Same event shape the frontend has always
                // consumed for incoming DMs
                let _ = self.app_handle.emit(
                    "p2p-message-received",
                    serde_json::json!({
                        "message": {
                            "DirectMessage": {
                                "message_id": message_id,
                                "from_user_id": sender_user_id,
                                "from_public_key": sender_public_key,
                                "to_user_id": self.user_id,
                                "encrypted_content": content,
                                "timestamp": timestamp,
                                "device_id": null,
                            }
                        }
                    }),
                );
            }
            super::crypto::ContentPayload::CommunityPost {
                community_id,
                community_name,
                content,
                attachments,
                node_id,
                blob_refs,
                show_in_main_feed,
                sent_at,
            } => {
                let timestamp = if sent_at > 0 {
                    sent_at
                } else {
                    envelope_timestamp
                };
                let sender_user_id = super::types::SqliteUuid::from_public_key(&sender_public_key);
                println!(
                    "[IROH] Received community post in '{}' from {}: {} chars",
                    community_name,
                    sender_public_key,
                    content.len()
                );

                // Parse community_id from string
                let community_uuid = match super::types::SqliteUuid::parse_str(&community_id) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IROH] Failed to parse community_id: {}", e);
                        return Ok(());
                    }
                };

                // AUTHORIZATION: the community_id is a sender-supplied claim.
                // Require that the authenticated sender really is a member of
                // that community before storing anything under it.
                if !self.is_authorized_community_sender(&sender_public_key, community_uuid) {
                    return Ok(());
                }

                // Ensure sender exists in database
                // NOTE: Using lock() instead of try_lock() for data integrity
                {
                    let conn = self.lock_db_conn_blocking();
                    if let Err(e) = conn.execute(
                        "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            sender_user_id,
                            format!("User_{}", &sender_public_key[..8.min(sender_public_key.len())]),
                            &sender_public_key,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to ensure community post sender exists: {}", e);
                    }
                }

                // Create the post in database
                match self.db.create_post(sender_user_id, &content, false) {
                    Ok(post) => {
                        // Link it to the community
                        if let Err(e) = self.db.create_community_post(
                            community_uuid,
                            post.id,
                            show_in_main_feed,
                        ) {
                            println!("[IROH] Warning: Failed to link post to community: {}", e);
                        }

                        // Attachments: prefer blob refs (fetched from the
                        // sender's blob store, same as sealed posts); fall
                        // back to the legacy embedded bytes from old senders
                        if !blob_refs.is_empty() {
                            self.fetch_and_store_post_attachments(
                                post.id,
                                sender_user_id,
                                &node_id,
                                &blob_refs,
                            )
                            .await;
                        } else if let Some(ref atts) = attachments {
                            use base64::Engine as _;
                            for att in atts {
                                if !is_supported_media_type(&att.file_type) {
                                    println!(
                                        "[IROH] Skipping embedded community attachment {}: unsupported file type '{}'",
                                        att.id, att.file_type
                                    );
                                    continue;
                                }
                                match base64::engine::general_purpose::STANDARD.decode(&att.data)
                                {
                                    Ok(bytes) => {
                                        if let Err(e) = self.db.upload_media(
                                            sender_user_id,
                                            post.id,
                                            "synced_file",
                                            &att.file_type,
                                            &bytes,
                                            att.file_size,
                                        ) {
                                            println!(
                                                "[IROH] Warning: Failed to save embedded community attachment {}: {}",
                                                att.id, e
                                            );
                                        }
                                    }
                                    Err(e) => println!(
                                        "[IROH] Warning: Failed to decode embedded community attachment {}: {}",
                                        att.id, e
                                    ),
                                }
                            }
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
                                public_key: sender_public_key.clone(),
                                content,
                                show_in_main_feed,
                                timestamp,
                                attachments,
                            },
                        );
                        println!("[IROH] [OK] Stored and emitted community post");
                    }
                    Err(e) => {
                        println!("[IROH] Failed to create community post: {}", e);
                        // Transient DB failure (the insert didn't happen):
                        // leave the envelope unmarked so a redelivery retries
                        return Err(format!("failed to create community post: {}", e));
                    }
                }
            }
            super::crypto::ContentPayload::CommunityMemberAdded {
                community_id,
                community_name,
                new_member_public_key,
                new_member_display_name,
            } => {
                println!(
                    "[IROH] Received community member added: {} joined '{}'",
                    new_member_display_name, community_name
                );

                // Parse community_id from string
                let community_uuid = match super::types::SqliteUuid::parse_str(&community_id) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IROH] Failed to parse community_id: {}", e);
                        return Ok(());
                    }
                };

                // Get or create user_id for the new member
                let new_member_user_id =
                    super::types::SqliteUuid::from_public_key(&new_member_public_key);

                // Ensure the new member user exists in database
                // NOTE: Using lock() instead of try_lock() for data integrity
                {
                    let conn = self.db.conn.lock().unwrap();
                    if let Err(e) = conn.execute(
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
            super::crypto::ContentPayload::PostComment {
                comment_id,
                post_id,
                content,
                parent_comment_id,
                sent_at,
            } => {
                // AUTHORIZATION: comments are friend-scoped content
                if !self
                    .is_authorized_content_sender(&sender_public_key, "post comment")
                    .await
                {
                    return Ok(());
                }
                let timestamp = if sent_at > 0 {
                    sent_at
                } else {
                    envelope_timestamp
                };
                let sender_user_id = super::types::SqliteUuid::from_public_key(&sender_public_key);
                println!(
                    "[IROH] Received post comment from {} on post {}",
                    sender_public_key, post_id
                );

                // Parse IDs
                let comment_uuid = match super::types::SqliteUuid::parse_str(&comment_id) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IROH] Failed to parse comment_id: {}", e);
                        return Ok(());
                    }
                };
                let post_uuid = match super::types::SqliteUuid::parse_str(&post_id) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IROH] Failed to parse post_id: {}", e);
                        return Ok(());
                    }
                };
                let parent_uuid = parent_comment_id
                    .as_ref()
                    .and_then(|id| super::types::SqliteUuid::parse_str(id).ok());

                // Ensure sender exists and save comment
                // NOTE: Using lock() instead of try_lock() - CRITICAL for comment persistence
                {
                    let conn = self.db.conn.lock().unwrap();
                    let now = chrono::Utc::now().to_rfc3339();

                    // Ensure sender exists
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            sender_user_id,
                            format!("User_{}", &sender_public_key[..8.min(sender_public_key.len())]),
                            &sender_public_key,
                            &now,
                            &now
                        ],
                    );

                    // Save comment (OR IGNORE for duplicates)
                    if let Err(e) = conn.execute(
                        "INSERT OR IGNORE INTO post_comments (id, post_id, user_id, content, parent_comment_id, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        rusqlite::params![
                            comment_uuid,
                            post_uuid,
                            sender_user_id,
                            &content,
                            parent_uuid,
                            &now,
                            &now
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to save comment: {}", e);
                        // Transient DB failure: leave the envelope unmarked so
                        // a redelivery retries the save
                        return Err(format!("failed to save comment: {}", e));
                    } else {
                        println!("[IROH] ✓ Saved comment to database");
                    }
                }

                // Emit to UI
                let _ = self.app_handle.emit(
                    "p2p-post-comment",
                    serde_json::json!({
                        "commentId": comment_id,
                        "postId": post_id,
                        "userId": sender_user_id.to_string(),
                        "publicKey": sender_public_key,
                        "content": content,
                        "parentCommentId": parent_comment_id,
                        "timestamp": timestamp
                    }),
                );
                println!("[IROH] ✓ Emitted p2p-post-comment event");
            }
            super::crypto::ContentPayload::PostReaction {
                post_id,
                emoji,
                action,
                sent_at,
            } => {
                // AUTHORIZATION: reactions are friend-scoped content
                if !self
                    .is_authorized_content_sender(&sender_public_key, "post reaction")
                    .await
                {
                    return Ok(());
                }
                // VALIDATION: the app only offers a fixed reaction set, and the
                // frontend renders this string into post markup. Anything else
                // is either a bug or an injection attempt - drop it.
                if !is_supported_reaction_emoji(&emoji) {
                    println!(
                        "[SECURITY] Rejecting reaction with unsupported emoji ({} bytes) from {}",
                        emoji.len(),
                        &sender_public_key[..8.min(sender_public_key.len())]
                    );
                    return Ok(());
                }
                if action != "add" && action != "remove" {
                    println!(
                        "[SECURITY] Rejecting reaction with unknown action '{}'",
                        action
                    );
                    return Ok(());
                }
                let timestamp = if sent_at > 0 {
                    sent_at
                } else {
                    envelope_timestamp
                };
                let sender_user_id = super::types::SqliteUuid::from_public_key(&sender_public_key);
                println!(
                    "[IROH] Received post reaction from {}: {} {} on post {}",
                    sender_public_key, action, emoji, post_id
                );

                // Parse post ID
                let post_uuid = match super::types::SqliteUuid::parse_str(&post_id) {
                    Ok(id) => id,
                    Err(e) => {
                        println!("[IROH] Failed to parse post_id: {}", e);
                        return Ok(());
                    }
                };

                // Save or remove reaction
                // NOTE: Using lock() instead of try_lock() - CRITICAL for reaction persistence
                {
                    let conn = self.db.conn.lock().unwrap();
                    let now = chrono::Utc::now().to_rfc3339();

                    // Ensure sender exists
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO users (id, display_name, public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            sender_user_id,
                            format!("User_{}", &sender_public_key[..8.min(sender_public_key.len())]),
                            &sender_public_key,
                            &now,
                            &now
                        ],
                    );

                    if action == "add" {
                        // Use REPLACE to handle existing reactions
                        if let Err(e) = conn.execute(
                            "INSERT OR REPLACE INTO post_reactions (id, post_id, user_id, emoji, created_at)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            rusqlite::params![
                                super::types::SqliteUuid::new(),
                                post_uuid,
                                sender_user_id,
                                &emoji,
                                &now
                            ],
                        ) {
                            println!("[IROH] Warning: Failed to save reaction: {}", e);
                            // Transient DB failure: leave the envelope unmarked
                            // so a redelivery retries the save
                            return Err(format!("failed to save reaction: {}", e));
                        } else {
                            println!("[IROH] ✓ Saved reaction to database");
                        }
                    } else if action == "remove" {
                        if let Err(e) = conn.execute(
                            "DELETE FROM post_reactions WHERE post_id = ?1 AND user_id = ?2",
                            rusqlite::params![post_uuid, sender_user_id],
                        ) {
                            println!("[IROH] Warning: Failed to remove reaction: {}", e);
                            return Err(format!("failed to remove reaction: {}", e));
                        } else {
                            println!("[IROH] ✓ Removed reaction from database");
                        }
                    }
                }

                // Emit to UI
                let _ = self.app_handle.emit(
                    "p2p-post-reaction",
                    serde_json::json!({
                        "postId": post_id,
                        "userId": sender_user_id.to_string(),
                        "publicKey": sender_public_key,
                        "emoji": emoji,
                        "action": action,
                        "timestamp": timestamp
                    }),
                );
                println!("[IROH] ✓ Emitted p2p-post-reaction event");
            }
            super::crypto::ContentPayload::DeviceSync {
                device_id,
                data_json,
                ..
            } => {
                // try_decrypt already verified the payload is
                // signed by sender_public_key; only our
                // own devices (same recovery phrase) can both
                // sign as us and encrypt to our key. Guard
                // anyway: sync must come from our own identity.
                if sender_public_key != self.public_key {
                    println!("[IROH-SYNC] Rejecting DeviceSync from foreign sender");
                    return Ok(());
                }
                if device_id == self.device_id.clone().unwrap_or_default() {
                    // Our own broadcast echoed back
                    return Ok(());
                }

                println!(
                    "[IROH-SYNC] Received encrypted device sync from device {}",
                    device_id
                );
                match serde_json::from_str::<crate::app::database::sync::SyncData>(&data_json) {
                    Ok(sync_data) => {
                        println!(
                            "[IROH-SYNC] Applying sync data: {} posts, {} messages, {} friends",
                            sync_data.posts.len(),
                            sync_data.messages.len(),
                            sync_data.friends.len()
                        );
                        match self.db.apply_sync_data(&sync_data) {
                            Ok(()) => {
                                // NOTE: applying an incoming sync must NOT
                                // advance any outgoing cursor. The old
                                // update_all_sync_timestamps call here moved
                                // the cursor that gated what we SENT, so after
                                // applying B's data our pre-existing rows were
                                // never sent to B. Responses are now gated on
                                // the REQUESTER's watermark instead.
                                let _ = self.app_handle.emit("device-sync-completed", device_id);
                                println!("[IROH-SYNC] Applied encrypted device sync");
                            }
                            Err(e) => {
                                println!("[IROH-SYNC] Failed to apply sync data: {}", e);
                                // Transient failure (the apply is transactional,
                                // nothing was committed): leave the envelope
                                // unmarked so a redelivery - or the next 30s
                                // request cycle - retries it
                                return Err(format!("failed to apply device sync: {}", e));
                            }
                        }
                    }
                    Err(e) => println!("[IROH-SYNC] Failed to deserialize sync data: {}", e),
                }
            }
            super::crypto::ContentPayload::FriendRequest {
                display_name,
                encryption_public_key,
                node_id,
                relay_url,
                ..
            } => {
                self.handle_friend_request(
                    sender_public_key.clone(),
                    encryption_public_key,
                    display_name,
                    node_id,
                    relay_url,
                )
                .await;
            }
            super::crypto::ContentPayload::FriendAccepted {
                display_name,
                encryption_public_key,
                node_id,
                relay_url,
                ..
            } => {
                self.handle_friend_accepted(
                    sender_public_key.clone(),
                    encryption_public_key,
                    display_name,
                    node_id,
                    relay_url,
                )
                .await;
            }
            super::crypto::ContentPayload::Presence {
                device_id,
                node_addr_json,
                encryption_public_key,
                display_name,
                bio,
                profile_picture,
                profile_signature,
                prekey_public,
                prekey_signature,
                prekey_created_at,
                sent_at,
            } => {
                // Replayed presence could poison our address
                // book with stale endpoints - only accept
                // reasonably fresh announcements
                let now = chrono::Utc::now().timestamp();
                if sent_at > 0 && now - sent_at > 3600 {
                    println!("[IROH] Ignoring stale presence (>1h old)");
                    return Ok(());
                }
                let node_addr: iroh::EndpointAddr = match serde_json::from_str(&node_addr_json) {
                    Ok(a) => a,
                    Err(e) => {
                        println!("[IROH] Invalid node address in presence: {}", e);
                        return Ok(());
                    }
                };
                // Piggybacked pre-key: store it (catch-up path if
                // a KeyRotation envelope was lost)
                self.store_friend_prekey(
                    &sender_public_key,
                    prekey_public.as_deref(),
                    prekey_signature.as_deref(),
                    prekey_created_at,
                );
                // Derive the peer's user_id from the AUTHENTICATED
                // sender key rather than trusting a claimed id
                let peer_user_id = super::types::SqliteUuid::from_public_key(&sender_public_key);
                self.handle_presence(
                    peer_user_id,
                    sender_public_key.clone(),
                    encryption_public_key,
                    device_id,
                    node_addr,
                    display_name,
                    bio,
                    profile_picture,
                    profile_signature,
                )
                .await;
            }
            super::crypto::ContentPayload::KeyRotation {
                prekey_public,
                signature,
                prekey_created_at,
                ..
            } => {
                // Explicit pre-key rotation announcement
                self.store_friend_prekey(
                    &sender_public_key,
                    Some(&prekey_public),
                    Some(&signature),
                    prekey_created_at,
                );
            }
            super::crypto::ContentPayload::FriendSyncRequest { since, .. } => {
                // A friend came back online and wants the posts
                // they missed. sender_public_key is authenticated;
                // resend_posts_to_friend re-checks the friendship
                // is accepted before sending anything.
                self.resend_posts_to_friend(&sender_public_key, since).await;
            }
        }
        Ok(())
    }

    /// Retry all pending messages when connection is restored
    pub async fn retry_pending_messages(&self) {
        // Single-flight: concurrent friend dials each trigger a retry pass;
        // overlapping passes would re-broadcast the same queued messages
        if self
            .retrying_pending
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        struct ClearOnDrop(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for ClearOnDrop {
            fn drop(&mut self) {
                self.0.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _retry_guard = ClearOnDrop(self.retrying_pending.clone());

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

                            // Try to resend via global mesh. Use the no-requeue
                            // publish path (queue_on_failure = false) so a failed
                            // retry doesn't insert a DUPLICATE row - the message
                            // is already queued; we just bump its retry count.
                            // Success below means the broadcast was CONFIRMED by
                            // the stream handler, not merely channel-queued.
                            match serde_json::from_str::<P2PMessage>(&pending_msg.content_json) {
                                Ok(message) => {
                                    match self
                                        .publish_message_inner(CONTENT_TOPIC, message, false, false)
                                        .await
                                    {
                                        Ok(_) => {
                                            println!(
                                                "[QUEUE] [OK] Successfully resent message (ID: {})",
                                                pending_msg.id
                                            );
                                            // Remove from queue
                                            if let Err(e) =
                                                self.db.mark_message_sent(pending_msg.id)
                                            {
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

                                            if pending_msg.retry_count + 1
                                                >= pending_msg.max_retries
                                            {
                                                println!(
                                                    "[QUEUE] Max retries reached, removing message"
                                                );
                                                if let Err(e) =
                                                    self.db.remove_pending_message(pending_msg.id)
                                                {
                                                    println!(
                                                        "[QUEUE] Warning: Failed to remove: {}",
                                                        e
                                                    );
                                                }
                                            } else if let Err(e) =
                                                self.db.increment_retry_count(pending_msg.id)
                                            {
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
        println!(
            "[FRIEND-RESEND] Checking for accepted friendships that need FriendAccepted resend..."
        );

        // Query for connections where status='accepted' and initiated_by != our user_id
        // These are connections where someone else sent us a friend request and we accepted
        // Join with users table to get the friend's public_key
        // NOTE: Using lock() instead of try_lock() for reliable resend behavior
        let accepted_connections: Vec<(String, String)> = {
            let conn = self.db.conn.lock().unwrap();
            let stmt_result = conn.prepare(
                "SELECT p.friend_user_id, u.public_key FROM p2p_connections p
                 JOIN users u ON p.friend_user_id = u.id
                 WHERE p.user_id = ?1 AND p.status = 'accepted' AND p.initiated_by != ?1",
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

        println!(
            "[FRIEND-RESEND] Found {} accepted connections to resend FriendAccepted",
            accepted_connections.len()
        );

        for (friend_user_id, friend_public_key) in accepted_connections {
            println!(
                "[FRIEND-RESEND] Resending FriendAccepted to {} ({})",
                friend_user_id, friend_public_key
            );

            // Seal to the friend's encryption key - without it we can't (and
            // shouldn't) tell them anything
            let friend_enc_key = match super::types::SqliteUuid::parse_str(&friend_user_id) {
                Ok(id) => self.get_encryption_key_for_user(id).await,
                Err(_) => None,
            };
            let Some(friend_enc_key) = friend_enc_key else {
                println!(
                    "[FRIEND-RESEND] No encryption key for {} - skipping",
                    friend_public_key
                );
                continue;
            };

            if let Err(e) = self.send_friend_accepted_sealed(&friend_enc_key).await {
                println!(
                    "[FRIEND-RESEND] Warning: Failed to resend FriendAccepted to {}: {}",
                    friend_public_key, e
                );
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
            store: self.store.clone(),
            downloader: self.downloader.clone(),
            address_book: self.address_book.clone(),
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
            last_broadcast_success: self.last_broadcast_success.clone(),
            shutdown_flag: self.shutdown_flag.clone(),
            background_tasks: self.background_tasks.clone(),
            recovering: self.recovering.clone(),
            friend_sync_sent: self.friend_sync_sent.clone(),
            retrying_pending: self.retrying_pending.clone(),
        }
    }

    /// Track a background loop so recover()/shutdown() can abort it
    fn track_background_task(&self, handle: tokio::task::JoinHandle<()>) {
        self.background_tasks.lock().unwrap().push(handle);
    }

    /// Abort all tracked background loops immediately.
    /// Must be called before spawning a replacement generation of loops.
    fn stop_background_tasks(&self) {
        let mut tasks = self.background_tasks.lock().unwrap();
        let count = tasks.len();
        for handle in tasks.drain(..) {
            handle.abort();
        }
        if count > 0 {
            println!("[IROH] Aborted {} background loops", count);
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
                println!(
                    "[IROH] Warning: Failed to subscribe to queued topic {}: {}",
                    topic, e
                );
            } else {
                println!("[IROH] [OK] Subscribed to queued topic: {}", topic);
            }
        }
    }

    /// Add a peer to the connected set
    pub async fn add_connected_peer(&self, node_id: iroh::EndpointId) {
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
    pub async fn remove_connected_peer(&self, node_id: iroh::EndpointId) {
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
    pub async fn is_peer_connected(&self, node_id: iroh::EndpointId) -> bool {
        let peers = self.connected_peers.lock().await;
        peers.contains(&node_id)
    }

    /// Clear all connected peers (used on initialization)
    pub async fn clear_connected_peers(&self) {
        let mut peers = self.connected_peers.lock().await;
        let count = peers.len();
        peers.clear();
        if count > 0 {
            println!(
                "[IROH] Cleared {} stale peer connections on initialization",
                count
            );
        }
    }

    /// Acquire the DB connection lock (blocking variant for sync helpers).
    /// Handlers now run in their own spawned tasks, so briefly blocking here
    /// matches every other `db.conn.lock().unwrap()` in this file - but unlike
    /// unwrap(), a poisoned mutex is recovered (loudly) instead of cascading
    /// the panic through the network layer.
    fn lock_db_conn_blocking(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.db.conn.lock().unwrap_or_else(|poisoned| {
            eprintln!("[DB] DB mutex poisoned - recovering (a previous holder panicked)");
            poisoned.into_inner()
        })
    }

    /// Acquire the DB connection lock from async code with a bounded retry
    /// loop (25ms steps, ~500ms total) before falling back to a blocking
    /// lock. Always returns a guard: callers get a DEFINITIVE answer instead
    /// of the old single-try_lock-then-give-up pattern that misreported mere
    /// lock contention as "not a friend" / "no key".
    async fn lock_db_conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        for _ in 0..20 {
            // NOTE: the try_lock result must be fully dropped before the sleep
            // below, or the non-Send guard would be held across the await
            match self.db.conn.try_lock() {
                Ok(guard) => return guard,
                Err(std::sync::TryLockError::WouldBlock) => {}
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    eprintln!("[DB] DB mutex poisoned - recovering (a previous holder panicked)");
                    return poisoned.into_inner();
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
        }
        println!(
            "[DB] DB lock still contended after ~500ms of retries - falling back to blocking lock"
        );
        self.lock_db_conn_blocking()
    }

    /// Get current user's encryption public key from database
    pub fn get_user_encryption_public_key(&self) -> Option<String> {
        let conn = self.lock_db_conn_blocking();
        conn.query_row(
            "SELECT encryption_public_key FROM users WHERE id = ?1",
            rusqlite::params![self.user_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
    }

    /// Get current user's encryption private key from database
    /// Process an authenticated presence announcement. Presence arrives inside
    /// sealed envelopes (ContentPayload::Presence); `public_key` is the sender
    /// key whose signature was verified during envelope decryption, and
    /// `peer_user_id` is derived from it rather than trusted from the payload.
    #[allow(clippy::too_many_arguments)]
    async fn handle_presence(
        &self,
        peer_user_id: super::types::SqliteUuid,
        public_key: String,
        encryption_public_key: Option<String>,
        device_id: String,
        node_addr: iroh::EndpointAddr,
        display_name: String,
        bio: String,
        profile_picture: String,
        profile_signature: Option<String>,
    ) {
        let peer_node_id = node_addr.id;
        println!(
            "[IROH] Received presence from user {} ({}) device {} (NodeId: {})",
            display_name,
            &public_key[..8],
            device_id,
            peer_node_id
        );
        println!(
            "[IROH]   Relay: {:?}, Direct addresses: {}",
            node_addr.relay_urls().next(),
            node_addr.ip_addrs().count()
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
                println!(
                    "[IROH] [SECURITY] Profile signature VERIFIED for {}",
                    display_name
                );
            } else {
                println!("[IROH] [SECURITY] WARNING: Profile signature INVALID for {} - possible tampering!", display_name);
            }
            valid
        } else {
            println!(
                "[IROH] [SECURITY] No profile signature provided by {}",
                display_name
            );
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

        // Store peer NodeId in database for future bootstrap. This builds a
        // network of known peers for gossip bootstrapping.
        //
        // SECURITY: device_id is an UNAUTHENTICATED claim inside the payload -
        // only the sender's identity key is authenticated. The unconstrained
        // `update_device_node_id(device_id, ...)` therefore let any peer
        // repoint ANY device row (including our own, or a friend's) at a node
        // they control. The update is bound to rows owned by the authenticated
        // sender's public key instead.
        let node_id_str = peer_node_id.to_string();
        {
            let conn = self.lock_db_conn_blocking();
            match conn.execute(
                "UPDATE devices SET iroh_node_id = ?1 WHERE id = ?2 AND user_public_key = ?3",
                rusqlite::params![&node_id_str, &device_id, &public_key],
            ) {
                Ok(0) => {
                    // Either we have never seen this device, or the row belongs
                    // to a DIFFERENT user - in which case the sender just tried
                    // to hijack someone else's device entry.
                    let owner: Option<String> = conn
                        .query_row(
                            "SELECT user_public_key FROM devices WHERE id = ?1",
                            rusqlite::params![&device_id],
                            |row| row.get(0),
                        )
                        .ok();
                    match owner {
                        Some(owner) if owner != public_key => println!(
                            "[IROH] [SECURITY] Ignoring presence claiming device {} - that device belongs to {}, not the sender",
                            device_id,
                            &owner[..8.min(owner.len())]
                        ),
                        _ => println!(
                            "[IROH] No device row for {} owned by sender - NodeId not stored (presence still processed)",
                            device_id
                        ),
                    }
                }
                Ok(_) => println!(
                    "[IROH] Stored peer NodeId {} for device {}",
                    node_id_str, device_id
                ),
                Err(e) => println!("[IROH] Warning: Failed to store peer NodeId: {}", e),
            }
        }

        // CLEANUP: Remove stale device entries for OTHER users (not our own user)
        // This handles the case where a peer user wiped their device and got a new device_id/node_id
        // The old device entry with the old node_id would otherwise stay in our database forever
        //
        // CRITICAL: Only run cleanup for OTHER users, not our own user!
        // If peer is another device of the SAME user (public_key == self.public_key),
        // running cleanup would delete OUR OWN device entry since it would match the
        // "different device_id, different node_id" criteria.
        if public_key != self.public_key {
            match self
                .db
                .cleanup_stale_devices_for_user(&public_key, &device_id, &node_id_str)
            {
                Ok(stale_node_ids) if !stale_node_ids.is_empty() => {
                    println!(
                        "[IROH] Cleaned up {} stale device entries for user {}",
                        stale_node_ids.len(),
                        &public_key[..8]
                    );
                    // Also remove stale node_ids from connected_peers
                    for stale_node_id in stale_node_ids {
                        if let Ok(stale_id) = stale_node_id.parse::<iroh::EndpointId>() {
                            self.remove_connected_peer(stale_id).await;
                            println!(
                                "[IROH] Removed stale peer {} from connected set",
                                stale_node_id
                            );
                        }
                    }
                }
                Ok(_) => {} // No stale entries
                Err(e) => {
                    println!("[IROH] Warning: Failed to cleanup stale devices: {}", e)
                }
            }
        }

        // NOTE: We received this Presence message via gossip, which means we ALREADY have
        // a working gossip connection to this peer. Calling endpoint.connect() would be
        // redundant and might interfere with the gossip protocol's connection management.
        //
        // Instead, we just:
        // 1. Add the peer's node address to the endpoint (for better routing info)
        // 2. Track the peer as connected (since we received their presence via gossip)
        // 3. Store their info in the database for future reconnection

        // Register the peer's address in the address book for better routing /
        // gossip resolution. Replaces 0.35 endpoint.add_node_addr().
        self.address_book.add_endpoint_info(node_addr.clone());
        println!("[IROH] [OK] Added peer's address to the address book");

        // Track this peer as connected (we received their presence via gossip!)
        self.add_connected_peer(peer_node_id).await;
        println!(
            "[IROH] [OK] Peer {} is connected via gossip mesh (received their presence)",
            peer_node_id
        );

        // Create friendship in database (CRITICAL for friends list to work!)
        // Skip if this is the same user (different device)
        if public_key != self.public_key {
            println!(
                "[IROH] Creating/updating friendship with peer user {}",
                peer_user_id
            );

            // First, ensure the peer user exists in our database with their profile data
            // CRITICAL: Include encryption_public_key for sealed envelope encryption (comments, reactions, etc.)
            // NOTE: Using lock() because encryption_public_key is ESSENTIAL for comments/reactions to work.
            // Without this data, sealed envelope encryption will fail and messages won't be delivered.
            {
                let conn = self.lock_db_conn_blocking();
                // SECURITY: profile fields (display_name / bio / profile_picture)
                // are only allowed to OVERWRITE what we already hold when the
                // accompanying profile signature verifies. Previously the
                // signature result was merely logged, so an unsigned (or
                // badly-signed) presence could rewrite a friend's displayed
                // identity - exactly the impersonation the signature exists to
                // prevent. A first-contact INSERT still takes the claimed
                // values (there is nothing to overwrite); the network address
                // and encryption key are accepted either way, since those are
                // bound to the authenticated sender key.
                let result = if signature_valid {
                    conn.execute(
                            "INSERT INTO users (id, display_name, public_key, encryption_public_key, bio, profile_picture, profile_signature, created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                             ON CONFLICT(public_key) DO UPDATE SET
                                display_name = excluded.display_name,
                                encryption_public_key = COALESCE(excluded.encryption_public_key, encryption_public_key),
                                bio = excluded.bio,
                                profile_picture = excluded.profile_picture,
                                profile_signature = excluded.profile_signature,
                                updated_at = excluded.updated_at",
                            rusqlite::params![
                                peer_user_id,
                                &display_name,
                                &public_key,
                                &encryption_public_key,
                                &bio,
                                &profile_picture,
                                &profile_signature,
                                chrono::Utc::now().to_rfc3339(),
                                chrono::Utc::now().to_rfc3339()
                            ],
                        )
                } else {
                    println!(
                        "[IROH] [SECURITY] Presence from {} has no valid profile signature - keeping stored profile fields",
                        &public_key[..8]
                    );
                    conn.execute(
                            "INSERT INTO users (id, display_name, public_key, encryption_public_key, bio, profile_picture, created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                             ON CONFLICT(public_key) DO UPDATE SET
                                encryption_public_key = COALESCE(excluded.encryption_public_key, encryption_public_key),
                                updated_at = excluded.updated_at",
                            rusqlite::params![
                                peer_user_id,
                                &display_name,
                                &public_key,
                                &encryption_public_key,
                                &bio,
                                &profile_picture,
                                chrono::Utc::now().to_rfc3339(),
                                chrono::Utc::now().to_rfc3339()
                            ],
                        )
                };
                if let Err(e) = result {
                    println!("[IROH] Warning: Failed to update peer user: {}", e);
                } else if encryption_public_key.is_some() {
                    println!(
                        "[IROH] [OK] Stored encryption_public_key for peer {}",
                        &public_key[..8]
                    );
                }
            }

            // SECURITY: Check for display name changes on existing friends
            // Store known_display_name when first becoming friends
            // Warn if display name changes with valid signature (legitimate update)
            // or especially if signature is invalid (potential impersonation)
            // Check both directions since connection could be stored either way
            // NOTE: Using lock() instead of try_lock() - security checks must not be skipped
            {
                let conn = self.db.conn.lock().unwrap();
                if let Ok((known_name, _stored_sig)) = conn
                            .query_row(
                                "SELECT known_display_name, friend_profile_signature FROM p2p_connections
                                 WHERE ((user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1))
                                 AND status = 'accepted'",
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
                                let _ = conn.execute(
                                    "UPDATE p2p_connections SET friend_profile_signature = ?1
                                     WHERE (user_id = ?2 AND friend_user_id = ?3) OR (user_id = ?3 AND friend_user_id = ?2)",
                                    rusqlite::params![&profile_signature, self.user_id, peer_user_id],
                                );
                            }
                        }
            }

            // Only update peer address for EXISTING friendships (pending or accepted)
            // Do NOT auto-create friendships - that should happen via FriendRequest flow
            // Check both directions since connection could be stored either way
            // NOTE: Using lock() instead of try_lock() - connection status check must not be skipped
            println!(
                "[IROH] Checking existing connection status for peer {}",
                peer_user_id
            );
            let existing_status: Option<String> = {
                let conn = self.db.conn.lock().unwrap();
                conn.query_row(
                            "SELECT status FROM p2p_connections
                             WHERE (user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1)",
                            rusqlite::params![self.user_id, peer_user_id],
                            |row| row.get(0)
                        ).ok()
            };
            println!(
                "[IROH] Existing status query returned: {:?}",
                existing_status
            );

            if let Some(status) = existing_status {
                println!(
                    "[IROH] Existing connection with peer {} has status: {}",
                    peer_user_id, status
                );

                // CRITICAL: Always update NodeId for reconnection - even if relay_url is missing
                // This ensures the discovery loop has the correct NodeId after peer wipes/restarts
                if let Some(relay_url) = node_addr.relay_urls().next() {
                    // Have relay URL: update both NodeId and relay URL
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
                } else {
                    // No relay URL: update just the NodeId, preserve existing relay URL
                    if let Err(e) =
                        self.db
                            .update_friend_node_id(self.user_id, peer_user_id, &node_id_str)
                    {
                        println!("[IROH] Warning: Failed to update friend NodeId: {}", e);
                    } else {
                        println!(
                            "[IROH] [OK] Friend NodeId updated for reconnection: {}",
                            node_id_str
                        );
                    }
                }

                // CRITICAL FIX: If we have accepted this peer's friend request, resend FriendAccepted
                // This handles the case where our original FriendAccepted was lost due to gossip mesh instability
                if status == "accepted" {
                    // This accepted friend is online. Ask them (rate-limited) to
                    // re-send posts we missed while offline - gossip is
                    // fire-and-forget, so anything posted while we were away is
                    // otherwise lost. Seal to whatever encryption key their
                    // presence carried (falls back handled inside).
                    if let Some(ref friend_enc_key) = encryption_public_key {
                        if !friend_enc_key.is_empty() {
                            self.maybe_request_friend_sync(peer_user_id, friend_enc_key)
                                .await;
                        }
                    }

                    // Check if this peer initiated the connection (they sent the friend request to us)
                    // NOTE: Using lock() instead of try_lock() - this check is important for FriendAccepted resend
                    let initiated_by: Option<super::types::SqliteUuid> = {
                        let conn = self.db.conn.lock().unwrap();
                        conn.query_row(
                                    "SELECT initiated_by FROM p2p_connections
                                     WHERE (user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1)",
                                    rusqlite::params![self.user_id, peer_user_id],
                                    |row| row.get(0)
                                ).ok()
                    };

                    // If peer_user_id initiated the connection, they are waiting for our FriendAccepted
                    if initiated_by == Some(peer_user_id) {
                        println!("[IROH] Peer {} initiated this friendship - resending FriendAccepted to ensure delivery", peer_user_id);

                        // Seal the FriendAccepted to the friend's encryption
                        // key (we have it - their presence just delivered it,
                        // and the handshake stored it)
                        match self.get_encryption_key_for_user(peer_user_id).await {
                            Some(friend_enc_key) => {
                                if let Err(e) =
                                    self.send_friend_accepted_sealed(&friend_enc_key).await
                                {
                                    println!(
                                        "[IROH] Warning: Failed to resend FriendAccepted: {}",
                                        e
                                    );
                                } else {
                                    println!("[IROH] [OK] Resent FriendAccepted to {} (ensuring they know we accepted)", public_key);
                                }
                            }
                            None => println!(
                                "[IROH] No encryption key for {} - cannot resend FriendAccepted",
                                public_key
                            ),
                        }
                    }
                }

                // CRITICAL FIX: If we have a pending OUTGOING request to this peer, resend FriendRequest
                // This handles the case where their app data was cleared and they lost our original request
                if status == "pending" {
                    // Check if WE initiated the connection (we sent the friend request to them)
                    // NOTE: Using lock() instead of try_lock() - this check is important for FriendRequest resend
                    let initiated_by: Option<super::types::SqliteUuid> = {
                        let conn = self.db.conn.lock().unwrap();
                        conn.query_row(
                                    "SELECT initiated_by FROM p2p_connections
                                     WHERE (user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1)",
                                    rusqlite::params![self.user_id, peer_user_id],
                                    |row| row.get(0)
                                ).ok()
                    };

                    // If WE initiated the connection, they may have lost our FriendRequest - resend it
                    if initiated_by == Some(self.user_id) {
                        println!("[IROH] We initiated friendship with {} - resending FriendRequest to ensure delivery", peer_user_id);

                        // Seal the FriendRequest to the friend's encryption
                        // key (stored when we scanned their invite; their
                        // presence just refreshed it)
                        match self.get_encryption_key_for_user(peer_user_id).await {
                            Some(friend_enc_key) => {
                                if let Err(e) =
                                    self.send_friend_request_sealed(&friend_enc_key).await
                                {
                                    println!(
                                        "[IROH] Warning: Failed to resend FriendRequest: {}",
                                        e
                                    );
                                } else {
                                    println!("[IROH] [OK] Resent FriendRequest to {} (in case they lost our original request)", public_key);
                                }
                            }
                            None => println!(
                                "[IROH] No encryption key for {} - cannot resend FriendRequest",
                                public_key
                            ),
                        }
                    }
                }
            } else {
                // No existing connection - this peer needs to send a FriendRequest first
                println!(
                    "[IROH] No existing connection with peer {} - waiting for FriendRequest",
                    peer_user_id
                );
            }
        }

        // GLOBAL MESH: No topic subscriptions needed in message handler
        // All nodes are on the same cipher/content/v1 topic
        // Presence is just for peer discovery and connection establishment
        println!("[IROH] Presence processed - global mesh handles all routing");

        // Check if this is another device with the same user account
        if public_key == self.public_key && device_id != self.device_id.clone().unwrap_or_default()
        {
            println!(
                "[IROH] SAME-USER DEVICE DETECTED: {} with NodeId: {}",
                device_id, peer_node_id
            );

            // Send device sync request via global mesh
            match self.build_signed_sync_request() {
                Some(sync_request) => {
                    if let Err(e) = self.publish_message(CONTENT_TOPIC, sync_request).await {
                        println!("[IROH] Failed to send device sync request: {}", e);
                    } else {
                        println!(
                            "[IROH] Sent device sync request to same-user device {}",
                            device_id
                        );
                    }
                }
                None => {
                    println!("[IROH] Cannot sign device sync request (no signing key) - skipping")
                }
            }
        }
    }

    /// Process an authenticated friend request. Arrives sealed to our
    /// encryption key; `from_public_key` is the envelope's verified sender, so
    /// friend requests can no longer be forged or observed by third parties.
    async fn handle_friend_request(
        &self,
        from_public_key: String,
        from_encryption_public_key: String,
        from_display_name: String,
        from_node_id: String,
        from_relay_url: String,
    ) {
        println!(
            "[IROH] Received FriendRequest from {} ({}) (global mesh)",
            from_display_name, from_public_key
        );

        // Skip if trying to add ourselves
        if from_public_key == self.public_key {
            println!("[IROH] Cannot add ourselves as friend, ignoring");
            return;
        }

        println!(
            "[IROH] Processing friend request from {} ({})",
            from_display_name, from_public_key
        );

        // CRITICAL: Compute friend's user_id from their public key to ensure consistency
        // Don't trust from_user_id from the message - derive it deterministically
        let friend_user_id = super::types::SqliteUuid::from_public_key(&from_public_key);
        println!(
            "[IROH] Computed friend_user_id {} from public_key",
            friend_user_id
        );

        // 1. Ensure the friend user exists in database with their actual display name and encryption key
        // Use ON CONFLICT(public_key) since that's the reliable unique identifier
        // CRITICAL: Store encryption_public_key so we can encrypt comments/reactions to them
        // NOTE: Using lock() instead of try_lock() because this MUST succeed - data loss is unacceptable
        {
            let conn = self.db.conn.lock().unwrap();
            // Use empty string check since encryption_public_key might be "" for old clients
            let enc_key = if from_encryption_public_key.is_empty() {
                None
            } else {
                Some(&from_encryption_public_key)
            };
            if let Err(e) = conn.execute(
                        "INSERT INTO users (id, display_name, public_key, encryption_public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                         ON CONFLICT(public_key) DO UPDATE SET
                            display_name = excluded.display_name,
                            encryption_public_key = COALESCE(excluded.encryption_public_key, encryption_public_key),
                            updated_at = excluded.updated_at",
                        rusqlite::params![
                            friend_user_id,  // Use computed ID, not from message
                            &from_display_name,
                            &from_public_key,
                            enc_key,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to create/update friend user: {}", e);
                    } else {
                        println!("[IROH] [OK] Created/updated friend user {} with display name: {} (enc_key: {})",
                            friend_user_id, from_display_name, if enc_key.is_some() { "yes" } else { "no" });
                    }
        }

        // 2. Create incoming friend request in database (status = 'pending')
        // User must accept this request before friendship is established
        // Convention: user_id = sender (initiator), friend_user_id = receiver
        // IMPORTANT: Check both directions to avoid duplicate rows when both users send requests
        // NOTE: Using lock() instead of try_lock() because this MUST succeed - data loss is unacceptable
        let mut auto_accepted = false;
        {
            let conn = self.db.conn.lock().unwrap();
            let now = chrono::Utc::now().to_rfc3339();

            // Check if any connection exists in either direction
            let existing: Option<(String, super::types::SqliteUuid)> = conn.query_row(
                        "SELECT status, initiated_by FROM p2p_connections
                         WHERE (user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1)",
                        rusqlite::params![self.user_id, friend_user_id],
                        |row| Ok((row.get(0)?, row.get(1)?))
                    ).ok();

            match existing {
                Some((status, _)) if status == "accepted" => {
                    // Already friends - just update their node info
                    println!(
                        "[IROH] Already friends with {}, updating node info",
                        from_public_key
                    );
                    let _ = conn.execute(
                                "UPDATE p2p_connections SET iroh_node_id = ?1, friend_relay_url = ?2, updated_at = ?3
                                 WHERE (user_id = ?4 AND friend_user_id = ?5) OR (user_id = ?5 AND friend_user_id = ?4)",
                                rusqlite::params![&from_node_id, &from_relay_url, &now, self.user_id, friend_user_id],
                            );
                }
                Some((status, initiated_by))
                    if status == "pending" && initiated_by == self.user_id =>
                {
                    // We already sent THEM a request and they're sending us one - auto-accept!
                    println!(
                        "[IROH] Mutual friend request detected! Auto-accepting with {}",
                        from_public_key
                    );
                    let _ = conn.execute(
                                "UPDATE p2p_connections SET status = 'accepted', iroh_node_id = ?1, friend_relay_url = ?2, updated_at = ?3
                                 WHERE (user_id = ?4 AND friend_user_id = ?5) OR (user_id = ?5 AND friend_user_id = ?4)",
                                rusqlite::params![&from_node_id, &from_relay_url, &now, self.user_id, friend_user_id],
                            );
                    auto_accepted = true;
                }
                Some((status, _)) if status == "pending" => {
                    // They already sent us a request (we haven't accepted) - update their node info
                    println!(
                        "[IROH] Already have pending request from {}, updating node info",
                        from_public_key
                    );
                    let _ = conn.execute(
                                "UPDATE p2p_connections SET iroh_node_id = ?1, friend_relay_url = ?2, updated_at = ?3
                                 WHERE (user_id = ?4 AND friend_user_id = ?5) OR (user_id = ?5 AND friend_user_id = ?4)",
                                rusqlite::params![&from_node_id, &from_relay_url, &now, self.user_id, friend_user_id],
                            );
                }
                _ => {
                    // No existing connection - create new incoming request
                    // CRITICAL: user_id must ALWAYS be the local user (self) for queries to work
                    // friend_user_id is the friend we're connecting to (computed from public key)
                    // initiated_by tracks who sent the original request
                    if let Err(e) = conn.execute(
                                "INSERT INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, created_at, updated_at, iroh_node_id, friend_relay_url)
                                 VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, ?8)",
                                rusqlite::params![
                                    super::types::SqliteUuid::new(),
                                    self.user_id,     // user_id = ALWAYS the local user (us)
                                    friend_user_id,   // friend_user_id = computed from sender's public key
                                    friend_user_id,   // initiated_by = sender (they sent the request)
                                    &now,
                                    &now,
                                    &from_node_id,
                                    &from_relay_url,
                                ],
                            ) {
                                println!("[IROH] Warning: Failed to create friend request: {}", e);
                            } else {
                                println!("[IROH] [OK] Incoming friend request created from {} (our user_id={}, friend_user_id={})",
                                    from_public_key, self.user_id, friend_user_id);
                            }
                }
            }
        }

        // Emit acceptance event AFTER releasing the database lock
        if auto_accepted {
            let _ = self.app_handle.emit(
                "friend-request-accepted",
                serde_json::json!({
                    "friend_user_id": friend_user_id.to_string(),
                    "friend_public_key": from_public_key,
                }),
            );
        }

        // Emit event to UI so it can show the pending request
        let _ = self.app_handle.emit(
            "friend-request-received",
            serde_json::json!({
                "from_user_id": friend_user_id.to_string(),
                "from_public_key": from_public_key,
                "from_node_id": from_node_id,
            }),
        );

        // 3. Add friend's node address to the address book for direct connection
        if let Ok(peer_node_id) = from_node_id.parse::<iroh::EndpointId>() {
            if let Ok(relay_url_parsed) = from_relay_url.parse::<url::Url>() {
                let node_addr =
                    iroh::EndpointAddr::new(peer_node_id).with_relay_url(relay_url_parsed.into());

                self.address_book.add_endpoint_info(node_addr);
                println!("[IROH] [OK] Added friend's address to the address book");

                // NOTE: We do NOT resubscribe here because if we received this message via gossip,
                // the mesh is already working. Resubscribing would disrupt the mesh and cause
                // message loss. The sender will resubscribe on their end when they join with us
                // as bootstrap, which is sufficient for bidirectional connectivity.
                println!("[IROH] Gossip mesh already functional (received message), skipping resubscription");
            }
        }

        println!("[IROH] [OK] Friend request received, waiting for user to accept");
    }

    /// Process an authenticated friend acceptance (sealed + signed, like
    /// handle_friend_request)
    async fn handle_friend_accepted(
        &self,
        from_public_key: String,
        from_encryption_public_key: String,
        from_display_name: String,
        from_node_id: String,
        from_relay_url: String,
    ) {
        println!(
            "[IROH] Received FriendAccepted from {} ({}) (global mesh)",
            from_display_name, from_public_key
        );

        // Skip if from ourselves
        if from_public_key == self.public_key {
            println!("[IROH] FriendAccepted from ourselves, ignoring");
            return;
        }

        // CRITICAL: Compute friend_user_id deterministically from public key
        // This ensures it matches exactly how it was stored when the outgoing request was created
        let friend_user_id = super::types::SqliteUuid::from_public_key(&from_public_key);
        println!(
            "[IROH] Computed friend_user_id from public key: {}",
            friend_user_id
        );

        // Ensure the friend user exists with their proper display name and encryption key
        // CRITICAL: Store encryption_public_key so we can encrypt comments/reactions to them
        // Use INSERT ON CONFLICT to handle both new users and stub-named users
        // NOTE: Using lock() instead of try_lock() because this MUST succeed - data loss is unacceptable
        {
            let conn = self.db.conn.lock().unwrap();
            // Use empty string check since encryption_public_key might be "" for old clients
            let enc_key = if from_encryption_public_key.is_empty() {
                None
            } else {
                Some(&from_encryption_public_key)
            };
            if let Err(e) = conn.execute(
                        "INSERT INTO users (id, display_name, public_key, encryption_public_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                         ON CONFLICT(public_key) DO UPDATE SET
                            display_name = excluded.display_name,
                            encryption_public_key = COALESCE(excluded.encryption_public_key, encryption_public_key),
                            updated_at = excluded.updated_at",
                        rusqlite::params![
                            friend_user_id,
                            &from_display_name,
                            &from_public_key,
                            enc_key,
                            chrono::Utc::now().to_rfc3339(),
                            chrono::Utc::now().to_rfc3339()
                        ],
                    ) {
                        println!("[IROH] Warning: Failed to create/update friend user: {}", e);
                    } else {
                        println!("[IROH] [OK] Created/updated friend user with display name: {} (enc_key: {})",
                            from_display_name, if enc_key.is_some() { "yes" } else { "no" });
                    }
        }

        // AUTHORIZATION: a FriendAccepted is only meaningful as the second half
        // of a handshake WE started. Our X25519 key is published in every invite
        // QR, so anyone who has seen an invite can seal us a well-formed
        // FriendAccepted; promoting on that alone handed them friend-level
        // access. We therefore only flip to 'accepted' when a pending row
        // exists that WE initiated.
        let we_initiated_pending = match self
            .db
            .has_locally_initiated_pending_request(self.user_id, friend_user_id)
        {
            Ok(v) => v,
            Err(e) => {
                println!(
                    "[IROH] Warning: could not check pending friend request ({}) - treating as unsolicited",
                    e
                );
                false
            }
        };

        // Did we end up with an accepted friendship? Drives which UI event fires.
        let mut now_accepted = false;

        {
            let conn = self.lock_db_conn_blocking();
            let now = chrono::Utc::now().to_rfc3339();
            println!(
                "[IROH] Updating p2p_connections: user_id={}, friend_user_id={} (we_initiated_pending={})",
                self.user_id, friend_user_id, we_initiated_pending
            );

            // Promote the pending request we sent - and ONLY that one. The
            // initiated_by clause keeps an INCOMING pending request (one they
            // sent us, awaiting the local user's approval) from being
            // self-accepted by the sender.
            let rows_affected = if we_initiated_pending {
                match conn.execute(
                        "UPDATE p2p_connections SET status = 'accepted', updated_at = ?1, iroh_node_id = ?4, friend_relay_url = ?5
                         WHERE ((user_id = ?2 AND friend_user_id = ?3) OR (user_id = ?3 AND friend_user_id = ?2))
                           AND status = 'pending' AND initiated_by = ?2",
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
                    }
            } else {
                0
            };

            if rows_affected > 0 {
                now_accepted = true;
                println!(
                    "[IROH] [OK] Friend request accepted by {}, {} row(s) updated",
                    from_public_key, rows_affected
                );
            } else {
                // No request of ours to promote - check what we already have
                // Check both directions since connection could be stored either way
                let existing_status: Option<String> = conn.query_row(
                            "SELECT status FROM p2p_connections
                             WHERE (user_id = ?1 AND friend_user_id = ?2) OR (user_id = ?2 AND friend_user_id = ?1)",
                            rusqlite::params![self.user_id, friend_user_id],
                            |row| row.get(0)
                        ).ok();

                match existing_status.as_deref() {
                    Some("accepted") => {
                        now_accepted = true;
                        println!("[IROH] Friendship already accepted, updating node info");
                        // Update node info even if already accepted
                        let _ = conn.execute(
                                    "UPDATE p2p_connections SET iroh_node_id = ?1, friend_relay_url = ?2, updated_at = ?3
                                     WHERE (user_id = ?4 AND friend_user_id = ?5) OR (user_id = ?5 AND friend_user_id = ?4)",
                                    rusqlite::params![&from_node_id, &from_relay_url, &now, self.user_id, friend_user_id],
                                );
                    }
                    Some(status) => {
                        // e.g. an incoming 'pending' request, or a rejected /
                        // blocked row. NEVER force these to accepted - only the
                        // local user may do that. Record the reachability info
                        // so the address survives, and leave status alone.
                        println!(
                            "[IROH] FriendAccepted from {} but connection status is '{}' - refusing to promote; updating node info only",
                            from_public_key, status
                        );
                        let _ = conn.execute(
                                    "UPDATE p2p_connections SET iroh_node_id = ?1, friend_relay_url = ?2, updated_at = ?3
                                     WHERE (user_id = ?4 AND friend_user_id = ?5) OR (user_id = ?5 AND friend_user_id = ?4)",
                                    rusqlite::params![&from_node_id, &from_relay_url, &now, self.user_id, friend_user_id],
                                );
                    }
                    None => {
                        // Unsolicited FriendAccepted with no connection at all
                        // (their acceptance arrived after our data was wiped, or
                        // it is simply a stranger). Record it as an INCOMING
                        // PENDING request so the local user can approve it -
                        // never as an accepted friendship.
                        println!("[IROH] Unsolicited FriendAccepted from {} with no existing connection - recording as a pending incoming request", from_public_key);

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

                        // initiated_by = them: this is an incoming request
                        // awaiting the local user's approval
                        if let Err(e) = conn.execute(
                                    "INSERT INTO p2p_connections (id, user_id, friend_user_id, status, initiated_by, iroh_node_id, friend_relay_url, created_at, updated_at)
                                     VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7, ?8)",
                                    rusqlite::params![
                                        super::types::SqliteUuid::new(),
                                        self.user_id,
                                        friend_user_id,
                                        friend_user_id, // They initiated
                                        &from_node_id,
                                        &from_relay_url,
                                        &now,
                                        &now,
                                    ],
                                ) {
                                    println!("[IROH] Warning: Failed to create pending friendship: {}", e);
                                } else {
                                    println!("[IROH] [OK] Recorded pending incoming request from unsolicited FriendAccepted");
                                }
                    }
                }
            }
        }

        // Emit event to UI so it can refresh the friends list. An unsolicited
        // acceptance is surfaced as a REQUEST to approve, not as a new friend.
        if now_accepted {
            let _ = self.app_handle.emit(
                "friend-accepted",
                serde_json::json!({
                    "from_user_id": friend_user_id.to_string(),
                    "from_public_key": from_public_key,
                }),
            );
        } else {
            let _ = self.app_handle.emit(
                "friend-request-received",
                serde_json::json!({
                    "from_user_id": friend_user_id.to_string(),
                    "from_public_key": from_public_key,
                    "from_node_id": from_node_id,
                }),
            );
        }

        // Add their node address to the address book for direct connection
        if let Ok(peer_node_id) = from_node_id.parse::<iroh::EndpointId>() {
            if let Ok(relay_url_parsed) = from_relay_url.parse::<url::Url>() {
                let node_addr =
                    iroh::EndpointAddr::new(peer_node_id).with_relay_url(relay_url_parsed.into());

                self.address_book.add_endpoint_info(node_addr);
                println!("[IROH] [OK] Added friend's address to the address book");

                // NOTE: No resubscription needed! The gossip protocol handles mesh formation
                // automatically. Since we received this FriendAccepted message via gossip,
                // we already have a working gossip connection to the peer.
                // Resubscribing would tear down that connection and cause message loss.
                println!(
                    "[IROH] [OK] Friend's node address added - gossip mesh already established"
                );
                self.add_connected_peer(peer_node_id).await;
            }
        }

        if now_accepted {
            println!(
                "[IROH] [OK] Friendship fully established with {}!",
                from_public_key
            );
        } else {
            println!(
                "[IROH] FriendAccepted from {} processed without granting friendship (awaiting local approval)",
                from_public_key
            );
        }
    }

    /// Seal a payload for the given recipients and publish it on the content
    /// topic. This is the ONLY way application content leaves this node.
    pub async fn publish_sealed(
        &self,
        payload: &super::crypto::ContentPayload,
        recipients: &[String],
    ) -> Result<(), String> {
        let signing_key = self
            .get_user_signing_private_key()
            .ok_or_else(|| "No signing private key".to_string())?;
        let envelope = super::crypto::GossipEnvelope::seal(
            payload,
            recipients,
            &self.public_key,
            &signing_key,
        )?;
        let envelope_json = serde_json::to_string(&envelope)
            .map_err(|e| format!("Failed to serialize envelope: {}", e))?;
        self.publish_message(CONTENT_TOPIC, P2PMessage::SealedEnvelope { envelope_json })
            .await
    }

    /// Our node id and relay URL as strings, for handshake payloads
    async fn our_addr_strings(&self) -> Option<(String, String)> {
        let endpoint_clone = self.endpoint.lock().await.clone();
        let endpoint = endpoint_clone.as_ref()?;
        let addr = endpoint.addr();
        let relay = addr
            .relay_urls()
            .next()
            .map(|u| u.to_string())
            .unwrap_or_default();
        Some((addr.id.to_string(), relay))
    }

    /// Look up a user's stored X25519 encryption public key by user id.
    /// Uses the bounded-retry lock so lock contention can't silently skip
    /// friend-critical resends: None means the user genuinely has no key.
    async fn get_encryption_key_for_user(
        &self,
        user_id: super::types::SqliteUuid,
    ) -> Option<String> {
        let conn = self.lock_db_conn().await;
        conn.query_row(
            "SELECT encryption_public_key FROM users WHERE id = ?1",
            rusqlite::params![user_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .filter(|k| !k.is_empty())
    }

    /// Build and send a sealed FriendRequest to a friend's encryption key
    pub async fn send_friend_request_sealed(
        &self,
        recipient_encryption_key: &str,
    ) -> Result<(), String> {
        let (node_id, relay_url) = self
            .our_addr_strings()
            .await
            .ok_or_else(|| "Endpoint not initialized".to_string())?;
        let payload = super::crypto::ContentPayload::FriendRequest {
            display_name: self.display_name.clone(),
            encryption_public_key: self.get_user_encryption_public_key().unwrap_or_default(),
            node_id,
            relay_url,
            sent_at: chrono::Utc::now().timestamp(),
        };
        self.publish_sealed(&payload, &[recipient_encryption_key.to_string()])
            .await
    }

    /// Build and send a sealed FriendAccepted to a friend's encryption key
    pub async fn send_friend_accepted_sealed(
        &self,
        recipient_encryption_key: &str,
    ) -> Result<(), String> {
        let (node_id, relay_url) = self
            .our_addr_strings()
            .await
            .ok_or_else(|| "Endpoint not initialized".to_string())?;
        let payload = super::crypto::ContentPayload::FriendAccepted {
            display_name: self.display_name.clone(),
            encryption_public_key: self.get_user_encryption_public_key().unwrap_or_default(),
            node_id,
            relay_url,
            sent_at: chrono::Utc::now().timestamp(),
        };
        self.publish_sealed(&payload, &[recipient_encryption_key.to_string()])
            .await
    }

    /// Get our Ed25519 signing private key (used to sign sealed-envelope
    /// payloads and device sync requests)
    fn get_user_signing_private_key(&self) -> Option<String> {
        let conn = self.lock_db_conn_blocking();
        conn.query_row(
            "SELECT private_key FROM users WHERE id = ?1",
            rusqlite::params![self.user_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
    }

    /// Build a signed DeviceSyncRequest. Returns None if we have no signing
    /// key - an unsigned request would be rejected by our other devices.
    fn build_signed_sync_request(&self) -> Option<P2PMessage> {
        let signing_key = self.get_user_signing_private_key()?;
        let device_id = self
            .device_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        // PULL MODEL: ask for everything newer than the newest row WE hold,
        // minus a 1-hour overlap window for clock skew between devices.
        // apply_sync_data is idempotent/newer-wins, so the overlap only costs
        // bandwidth. The periodic 30s request loop makes this the natural
        // retry/ack mechanism: a lost response leaves our watermark unchanged
        // and the next request re-fetches the same delta.
        let last_sync_timestamp = match self.db.get_local_sync_watermark(self.user_id) {
            Ok(watermark) => (watermark - 3600).max(0),
            Err(e) => {
                println!(
                    "[IROH-SYNC] Failed to compute local sync watermark: {} - requesting full sync",
                    e
                );
                0
            }
        };
        let timestamp = chrono::Utc::now().timestamp();
        let context =
            sync_request_context(&self.public_key, &device_id, last_sync_timestamp, timestamp);
        let signature = match Database::sign_message(&context, &signing_key) {
            Ok(sig) => sig,
            Err(e) => {
                println!("[IROH-SYNC] Failed to sign sync request: {}", e);
                return None;
            }
        };
        Some(P2PMessage::DeviceSyncRequest {
            public_key: self.public_key.clone(),
            device_id,
            last_sync_timestamp,
            timestamp,
            signature,
        })
    }

    /// Ensure we have a current pre-key, rotating if it's older than the
    /// rotation interval. On rotation (or first creation) broadcast a
    /// KeyRotation envelope to friends so they adopt the new key promptly;
    /// presence piggybacking covers anyone who misses it.
    async fn ensure_prekey_and_maybe_rotate(&self, identity_signing_private_key: &str) {
        let age = self.db.current_prekey_age_secs(self.user_id);
        let needs_rotation = match age {
            None => true, // no pre-key yet
            Some(a) => a >= super::database::prekeys::PREKEY_ROTATION_SECS,
        };
        if !needs_rotation {
            return;
        }

        let published = if age.is_none() {
            self.db
                .ensure_current_prekey(self.user_id, identity_signing_private_key)
        } else {
            self.db
                .rotate_prekey(self.user_id, identity_signing_private_key)
        };
        let published = match published {
            Ok(p) => p,
            Err(e) => {
                println!("[PREKEY] Failed to create/rotate pre-key: {}", e);
                return;
            }
        };
        println!("[PREKEY] Rotated pre-key, announcing to friends");

        let recipients = self.get_friend_encryption_public_keys();
        if recipients.is_empty() {
            return; // no friends to tell yet; presence will carry it later
        }
        let payload = super::crypto::ContentPayload::KeyRotation {
            prekey_public: published.public_key,
            signature: published.signature,
            prekey_created_at: published.created_at,
            sent_at: chrono::Utc::now().timestamp(),
        };
        if let Err(e) = self.publish_sealed(&payload, &recipients).await {
            println!("[PREKEY] Failed to announce KeyRotation: {}", e);
        }
    }

    /// Verify a friend's advertised pre-key against their authenticated identity
    /// key and store it. `sender_public_key` is the envelope's verified sender.
    fn store_friend_prekey(
        &self,
        sender_public_key: &str,
        prekey_public: Option<&str>,
        prekey_signature: Option<&str>,
        prekey_created_at: i64,
    ) {
        let (Some(prekey), Some(sig)) = (prekey_public, prekey_signature) else {
            return;
        };
        if prekey.is_empty() {
            return;
        }
        // v2 announcements bind the pre-key's creation time into the signature
        // (monotonic - a replayed old rotation can't roll the key back). Peers
        // on older builds send no timestamp; we still verify their v1 signature
        // during the transition (see prekey_signing_context_v1).
        let valid = if prekey_created_at > 0 {
            let context =
                super::crypto::sealed_box::prekey_signing_context(prekey, prekey_created_at);
            Database::verify_signature(&context, sig, sender_public_key)
        } else {
            let legacy = super::crypto::sealed_box::prekey_signing_context_v1(prekey);
            Database::verify_signature(&legacy, sig, sender_public_key)
        };
        if !valid {
            println!("[PREKEY] Rejecting friend pre-key: bad signature");
            return;
        }
        match self
            .db
            .set_friend_prekey(sender_public_key, prekey, prekey_created_at)
        {
            Ok(true) => {}
            Ok(false) => println!(
                "[PREKEY] Ignoring non-advancing pre-key announcement (replay or stale) from {}",
                &sender_public_key[..8.min(sender_public_key.len())]
            ),
            Err(e) => println!("[PREKEY] Failed to store friend pre-key: {}", e),
        }
    }

    /// True if we have an accepted friendship with `friend_user_id` (either
    /// direction). Backfill and other friend-only responses gate on this.
    /// Uses the bounded-retry lock so mere lock contention can never make a
    /// real friend look like a stranger.
    async fn is_accepted_friend(&self, friend_user_id: super::types::SqliteUuid) -> bool {
        let conn = self.lock_db_conn().await;
        conn.query_row(
            "SELECT 1 FROM p2p_connections
             WHERE ((user_id = ?1 AND friend_user_id = ?2)
                 OR (user_id = ?2 AND friend_user_id = ?1))
               AND status = 'accepted' LIMIT 1",
            rusqlite::params![self.user_id, friend_user_id],
            |_| Ok(()),
        )
        .is_ok()
    }

    /// AUTHORIZATION GATE for incoming friend-scoped content (posts, comments,
    /// reactions, DMs).
    ///
    /// The sealed envelope only proves WHO signed a payload - it says nothing
    /// about whether we ever agreed to hear from them. Our X25519 encryption
    /// key is published in every invite QR, so anyone who has ever seen an
    /// invite can seal us content. Without this check that content was
    /// persisted and rendered exactly like a friend's.
    ///
    /// Returns true only for our own other devices, or an ACCEPTED friend who
    /// is not blocked in either direction.
    async fn is_authorized_content_sender(&self, sender_public_key: &str, kind: &str) -> bool {
        let sender_id = super::types::SqliteUuid::from_public_key(sender_public_key);
        let short = &sender_public_key[..8.min(sender_public_key.len())];

        // Our own other devices are always allowed (same identity key)
        if sender_public_key == self.public_key || sender_id == self.user_id {
            return true;
        }

        if !self.is_accepted_friend(sender_id).await {
            println!(
                "[SECURITY] Rejecting {} from {}: not an accepted friend",
                kind, short
            );
            return false;
        }

        match self.db.is_blocked_either_way(self.user_id, sender_id) {
            Ok(true) => {
                println!("[SECURITY] Rejecting {} from blocked user {}", kind, short);
                false
            }
            Ok(false) => true,
            Err(e) => {
                // The friendship is already established; a broken block table
                // shouldn't silently drop a real friend's content
                println!(
                    "[SECURITY] Warning: block check failed for {} ({}) - allowing accepted friend",
                    short, e
                );
                true
            }
        }
    }

    /// AUTHORIZATION GATE for incoming community content: the sender must be a
    /// member of the community they claim to be posting in (and not blocked).
    /// Community membership, not friendship, is the relevant relation here.
    fn is_authorized_community_sender(
        &self,
        sender_public_key: &str,
        community_id: super::types::SqliteUuid,
    ) -> bool {
        let sender_id = super::types::SqliteUuid::from_public_key(sender_public_key);
        let short = &sender_public_key[..8.min(sender_public_key.len())];

        if sender_public_key == self.public_key || sender_id == self.user_id {
            return true;
        }

        match self.db.is_community_member(community_id, sender_id) {
            Ok(true) => {}
            Ok(false) => {
                println!(
                    "[SECURITY] Rejecting community post from {}: not a member of {}",
                    short, community_id
                );
                return false;
            }
            Err(e) => {
                println!(
                    "[SECURITY] Rejecting community post from {}: membership check failed: {}",
                    short, e
                );
                return false;
            }
        }

        match self.db.is_blocked_either_way(self.user_id, sender_id) {
            Ok(true) => {
                println!(
                    "[SECURITY] Rejecting community post from blocked user {}",
                    short
                );
                false
            }
            Ok(false) => true,
            Err(e) => {
                println!(
                    "[SECURITY] Warning: block check failed for {} ({}) - allowing community member",
                    short, e
                );
                true
            }
        }
    }

    /// When a friend comes back online, ask them (rate-limited) to re-send the
    /// posts we may have missed while offline. `since` is derived from the
    /// newest post we already hold from them, falling back to a 7-day window.
    async fn maybe_request_friend_sync(
        &self,
        friend_user_id: super::types::SqliteUuid,
        friend_encryption_key: &str,
    ) {
        // Rate-limit: presence arrives every 30-120s; one request per friend
        // per 5 minutes is plenty to catch up without a request storm.
        // NOTE: the timestamp is recorded only AFTER a successful publish -
        // recording it up front locked the friend out for 5 minutes even when
        // nothing was actually sent.
        const MIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);
        {
            let sent = self.friend_sync_sent.lock().await;
            if let Some(last) = sent.get(&friend_user_id) {
                if last.elapsed() < MIN_INTERVAL {
                    return;
                }
            }
        }

        // Watermark: newest post we hold from this friend, else 7 days ago.
        let now = chrono::Utc::now().timestamp();
        let since = self
            .db
            .newest_post_time_from_author(friend_user_id)
            .ok()
            .flatten()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.timestamp())
            .unwrap_or(now - 7 * 24 * 3600);

        let payload = super::crypto::ContentPayload::FriendSyncRequest {
            since,
            sent_at: now,
        };
        if let Err(e) = self
            .publish_sealed(
                &payload,
                std::slice::from_ref(&friend_encryption_key.to_string()),
            )
            .await
        {
            println!("[BACKFILL] Failed to send friend sync request: {}", e);
        } else {
            // Publish confirmed - only now start the 5-minute rate-limit window
            self.friend_sync_sent
                .lock()
                .await
                .insert(friend_user_id, std::time::Instant::now());
            println!(
                "[BACKFILL] Requested catch-up from friend {} since {}",
                friend_user_id, since
            );
        }
    }

    /// Answer a friend's FriendSyncRequest: re-send our recent posts (authored
    /// after `since`) sealed to them, flagged is_backfill so they render
    /// silently, plus the comments and reactions we authored in the same
    /// window (sent as the same payload types the live path uses, so the
    /// receiving handlers are shared). Only accepted friends are answered.
    async fn resend_posts_to_friend(&self, requester_public_key: &str, since: i64) {
        let requester_id = super::types::SqliteUuid::from_public_key(requester_public_key);
        if requester_id == self.user_id || !self.is_accepted_friend(requester_id).await {
            println!("[BACKFILL] Ignoring sync request from non-friend");
            return;
        }
        let Some(requester_key) = self.get_encryption_key_for_user(requester_id).await else {
            println!("[BACKFILL] No encryption key for requester - cannot backfill");
            return;
        };

        let since_rfc3339 = chrono::DateTime::from_timestamp(since, 0)
            .unwrap_or_else(chrono::Utc::now)
            .to_rfc3339();
        // Cap the responses so a long-absent friend can't pull an unbounded feed
        let posts = match self
            .db
            .get_authored_posts_since(self.user_id, &since_rfc3339, 30)
        {
            Ok(p) => p,
            Err(e) => {
                println!("[BACKFILL] Failed to gather posts: {}", e);
                return;
            }
        };
        let comments = match self
            .db
            .get_authored_comments_since(self.user_id, &since_rfc3339, 100)
        {
            Ok(c) => c,
            Err(e) => {
                println!("[BACKFILL] Failed to gather comments: {}", e);
                vec![]
            }
        };
        let reactions =
            match self
                .db
                .get_authored_reactions_since(self.user_id, &since_rfc3339, 100)
            {
                Ok(r) => r,
                Err(e) => {
                    println!("[BACKFILL] Failed to gather reactions: {}", e);
                    vec![]
                }
            };
        if posts.is_empty() && comments.is_empty() && reactions.is_empty() {
            return;
        }
        println!(
            "[BACKFILL] Re-sending {} post(s), {} comment(s), {} reaction(s) to friend {}",
            posts.len(),
            comments.len(),
            reactions.len(),
            requester_id
        );

        // The original timestamps ride inside the payloads' sent_at so the
        // receiver orders backfilled content correctly
        let rfc3339_to_unix = |ts: &str| {
            chrono::DateTime::parse_from_rfc3339(ts)
                .map(|dt| dt.timestamp())
                .unwrap_or_else(|_| chrono::Utc::now().timestamp())
        };

        let node_id = self
            .our_addr_strings()
            .await
            .map(|(id, _)| id)
            .unwrap_or_default();
        let recipients = [requester_key];
        for (post_id, content) in posts {
            // Rebuild blob references from the persisted attachment bytes -
            // the original refs aren't persisted, and resending with empty
            // refs dropped the post's media entirely. Failures are logged by
            // the helper; backfill is best-effort. No quota tracking: these
            // bytes were accounted for at original publish time.
            let (blob_refs, _failures) = self.build_blob_refs_for_post(post_id, false).await;
            let payload = super::crypto::ContentPayload::Post {
                post_id: post_id.to_string(),
                content,
                node_id: node_id.clone(),
                blob_refs,
                sent_at: chrono::Utc::now().timestamp(),
                is_backfill: true,
            };
            if let Err(e) = self.publish_sealed(&payload, &recipients).await {
                println!("[BACKFILL] Failed to re-send post {}: {}", post_id, e);
            }
        }
        for (comment_id, post_id, content, parent_comment_id, created_at) in comments {
            // Same payload the live comment publish path builds; the payload
            // has no backfill flag, and the receiver dedups by comment id
            let payload = super::crypto::ContentPayload::PostComment {
                comment_id: comment_id.to_string(),
                post_id: post_id.to_string(),
                content,
                parent_comment_id: parent_comment_id.map(|id| id.to_string()),
                sent_at: rfc3339_to_unix(&created_at),
            };
            if let Err(e) = self.publish_sealed(&payload, &recipients).await {
                println!("[BACKFILL] Failed to re-send comment {}: {}", comment_id, e);
            }
        }
        for (post_id, emoji, created_at) in reactions {
            // Only reactions that still exist are stored, so backfill is
            // always an "add"; removals were broadcast live
            let payload = super::crypto::ContentPayload::PostReaction {
                post_id: post_id.to_string(),
                emoji,
                action: "add".to_string(),
                sent_at: rfc3339_to_unix(&created_at),
            };
            if let Err(e) = self.publish_sealed(&payload, &recipients).await {
                println!(
                    "[BACKFILL] Failed to re-send reaction on post {}: {}",
                    post_id, e
                );
            }
        }
    }

    /// Build blob references for a post's persisted attachments so the sent
    /// post carries fetchable media. This is the SINGLE path attachments take
    /// into the blob store - live post publish, community post publish and
    /// backfill re-sends all use it. Blob refs from the ORIGINAL publish
    /// aren't persisted, so re-sends re-encrypt the persisted attachment
    /// BYTES (media_attachments) with a fresh key and re-store them.
    ///
    /// `track_quota`: check the storage quota and record storage used (live
    /// publish paths). Backfill re-sends pass false - their bytes were
    /// already accounted for at original publish time.
    ///
    /// Returns the refs for successfully stored attachments plus a
    /// description of each attachment that had to be dropped.
    pub(crate) async fn build_blob_refs_for_post(
        &self,
        post_id: super::types::SqliteUuid,
        track_quota: bool,
    ) -> (Vec<super::types::BlobReference>, Vec<String>) {
        use base64::Engine as _;
        use chacha20poly1305::aead::{Aead, KeyInit};
        use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
        use rand::Rng;

        let mut failures = Vec::new();

        let attachments = match self.db.get_post_media(post_id) {
            Ok(a) => a,
            Err(e) => {
                println!("[BLOBS] Failed to load media for post {}: {}", post_id, e);
                failures.push(format!("failed to load media for post {}: {}", post_id, e));
                return (vec![], failures);
            }
        };

        let mut blob_refs = Vec::new();
        for attachment in attachments {
            let data = match base64::engine::general_purpose::STANDARD.decode(&attachment.data) {
                Ok(d) => d,
                Err(e) => {
                    println!(
                        "[BLOBS] Failed to decode attachment {}: {}",
                        attachment.id, e
                    );
                    failures.push(format!(
                        "attachment {}: base64 decode failed: {}",
                        attachment.id, e
                    ));
                    continue;
                }
            };

            // Check storage quota before storing (live publish paths only)
            let data_size = data.len() as i64;
            if track_quota {
                if let Ok(false) = self.db.can_store(data_size) {
                    println!(
                        "[BLOBS] Storage quota exceeded, skipping attachment {}",
                        attachment.id
                    );
                    failures.push(format!(
                        "attachment {}: storage quota exceeded",
                        attachment.id
                    ));
                    continue;
                }
            }

            // Encrypt blob data before storing (Signal/WhatsApp style wire
            // format: fresh random key, 24-byte nonce prepended)
            let (encrypted_data, key_bytes) = {
                let mut rng = rand::thread_rng();
                let mut key_bytes = [0u8; 32];
                rng.fill(&mut key_bytes);
                let mut nonce_bytes = [0u8; 24];
                rng.fill(&mut nonce_bytes);

                let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
                match cipher.encrypt(XNonce::from_slice(&nonce_bytes), data.as_slice()) {
                    Ok(ciphertext) => {
                        let mut encrypted = nonce_bytes.to_vec();
                        encrypted.extend(ciphertext);
                        (encrypted, key_bytes)
                    }
                    Err(e) => {
                        println!(
                            "[BLOBS] Failed to encrypt attachment {}: {:?}",
                            attachment.id, e
                        );
                        failures.push(format!(
                            "attachment {}: encryption failed: {:?}",
                            attachment.id, e
                        ));
                        continue;
                    }
                }
            };

            match self.store_blob(encrypted_data).await {
                Ok(hash) => {
                    if track_quota {
                        // Track storage used - a failed update drifts quota
                        // accounting, so at least make it loud
                        if let Err(e) = self.db.add_storage_used(data_size) {
                            eprintln!(
                                "[BLOBS] WARNING: failed to record {} bytes of storage used for attachment {}: {}",
                                data_size, attachment.id, e
                            );
                        }
                    }
                    blob_refs.push(super::types::BlobReference {
                        id: attachment.id,
                        file_type: attachment.file_type.clone(),
                        file_size: attachment.file_size,
                        blob_hash: hex::encode(hash.as_bytes()),
                        downloaded: true, // we're the sender, blob is local
                        encryption_key: Some(
                            base64::engine::general_purpose::STANDARD.encode(key_bytes),
                        ),
                    });
                }
                Err(e) => {
                    println!(
                        "[BLOBS] Failed to store attachment {} as blob: {}",
                        attachment.id, e
                    );
                    failures.push(format!(
                        "attachment {}: blob store failed: {}",
                        attachment.id, e
                    ));
                }
            }
        }
        (blob_refs, failures)
    }

    fn get_user_encryption_private_key(&self) -> Option<String> {
        let conn = self.lock_db_conn_blocking();
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
        let conn = self.lock_db_conn_blocking();

        // DEBUG: Log all p2p_connections for this user
        println!(
            "[DEBUG-KEYS] Looking up encryption keys for user_id: {}",
            self.user_id
        );
        if let Ok(mut stmt) =
            conn.prepare("SELECT friend_user_id, status FROM p2p_connections WHERE user_id = ?1")
        {
            if let Ok(rows) = stmt.query_map(rusqlite::params![self.user_id], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
            }) {
                for row in rows.flatten() {
                    let friend_id =
                        super::types::SqliteUuid::from_bytes(row.0.try_into().unwrap_or([0u8; 16]));
                    println!(
                        "[DEBUG-KEYS]   p2p_connection: friend_user_id={}, status={}",
                        friend_id, row.1
                    );
                }
            }
        }

        // DEBUG: Log all users with encryption_public_key
        if let Ok(mut stmt) = conn.prepare(
            "SELECT id, public_key, encryption_public_key FROM users WHERE encryption_public_key IS NOT NULL"
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            }) {
                for row in rows.flatten() {
                    let user_id = super::types::SqliteUuid::from_bytes(row.0.try_into().unwrap_or([0u8; 16]));
                    println!("[DEBUG-KEYS]   user with enc_key: id={}, public_key={}..., enc_key={}...",
                        user_id, &row.1[..8.min(row.1.len())], &row.2[..8.min(row.2.len())]);
                }
            }
        }

        // CRITICAL: Check BOTH directions of the friendship relationship
        // Connection can be stored as (user_id=me, friend_user_id=them) OR (user_id=them, friend_user_id=me)
        // depending on who initiated the friend request.
        // We select the friend's rotating pre-key alongside their identity key
        // and prefer the pre-key when it's fresh (forward secrecy).
        let mut stmt = match conn.prepare(
            "SELECT u.encryption_public_key, u.prekey_public, u.prekey_updated_at FROM users u
             INNER JOIN p2p_connections p ON
                (u.id = p.friend_user_id AND p.user_id = ?1) OR
                (u.id = p.user_id AND p.friend_user_id = ?1)
             WHERE p.status = 'accepted' AND u.encryption_public_key IS NOT NULL AND u.id != ?1",
        ) {
            Ok(s) => s,
            Err(e) => {
                println!("[DEBUG-KEYS] Failed to prepare query: {}", e);
                return vec![];
            }
        };

        let result: Vec<String> = match stmt.query_map(rusqlite::params![self.user_id], |row| {
            Ok(super::database::prekeys::best_recipient_key(
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        }) {
            Ok(rows) => rows.filter_map(|r| r.ok().flatten()).collect(),
            Err(e) => {
                println!("[DEBUG-KEYS] Query failed: {}", e);
                vec![]
            }
        };
        println!(
            "[DEBUG-KEYS] Final result: {} recipient keys found",
            result.len()
        );
        result
    }

    /// Get connection status
    pub async fn get_connection_status(&self) -> Result<serde_json::Value, String> {
        let endpoint_guard = self.endpoint.lock().await;
        let (has_endpoint, node_id, relay_url) = if let Some(endpoint) = endpoint_guard.as_ref() {
            let node_id = endpoint.id().to_string();
            // Relay URL now comes from our current EndpointAddr (populated once a relay
            // connection is established). Replaces the removed home_relay() watcher.
            let relay_url = endpoint
                .addr()
                .relay_urls()
                .next()
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
            println!(
                "[IROH-DEBUG] connected_peers count={}, ids={:?}",
                connected_count, peer_ids
            );
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

        let handle = tokio::spawn(async move {
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
                    let our_node_id = endpoint.id();
                    drop(endpoint_guard);

                    // Create heartbeat message
                    let heartbeat = P2PMessage::Heartbeat {
                        node_id: our_node_id.to_string(),
                        timestamp: chrono::Utc::now().timestamp(),
                    };

                    // Send to direct gossip neighbors only - heartbeats confirm
                    // live connections and don't need mesh-wide relay, which
                    // would let every node track every device's online times
                    match network
                        .publish_message_to_neighbors(CONTENT_TOPIC, heartbeat)
                        .await
                    {
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
        self.track_background_task(handle);

        println!("[HEARTBEAT] Started heartbeat sender (15s interval)");
    }

    /// Start heartbeat monitor - removes peers that haven't sent heartbeat in 45 seconds
    fn start_heartbeat_monitor(&self) {
        let network = Arc::new(self.clone_for_background());
        let shutdown_flag = self.shutdown_flag.clone();

        let handle = tokio::spawn(async move {
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

                // Find stale peers (no heartbeat in 45 seconds)
                let stale_peers: Vec<iroh::EndpointId> = heartbeats
                    .iter()
                    .filter(|(_, &last_heartbeat)| {
                        now.duration_since(last_heartbeat) > heartbeat_timeout
                    })
                    .map(|(peer_id, _)| *peer_id)
                    .collect();

                drop(heartbeats);

                // Remove stale peers
                if !stale_peers.is_empty() {
                    println!(
                        "[HEARTBEAT] Found {} stale peers (no heartbeat in 45s)",
                        stale_peers.len()
                    );
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
        self.track_background_task(handle);

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
            endpoint.addr().relay_urls().next().is_some()
        } else {
            false
        };
        drop(endpoint_guard);

        // Check gossip exists
        let gossip_guard = self.gossip.lock().await;
        let has_gossip = gossip_guard.is_some();
        drop(gossip_guard);

        // Check topic subscription (a dead stream handler counts as unsubscribed)
        let has_content_topic = self.is_topic_subscribed(CONTENT_TOPIC).await;

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

        // Check last REAL gossip broadcast. The presence/heartbeat timestamps only prove
        // a message was queued locally (publish_message succeeds even on a dead network),
        // which made the old health check blind to dead connections after suspend or a
        // network change. Heartbeats broadcast every 15s, so >60s without a real
        // broadcast means the stream handler is dead or the network is gone.
        let broadcast_guard = self.last_broadcast_success.lock().await;
        let broadcast_stale = match *broadcast_guard {
            Some(last) => now.duration_since(last) > max_stale_time,
            None => true, // Never succeeded
        };
        let broadcast_age_secs = broadcast_guard.map(|t| now.duration_since(t).as_secs());
        drop(broadcast_guard);

        // Determine health.
        // NOTE: relay_connected is informational only. Relays are an optional NAT-traversal
        // helper by design; requiring one made health_check demand reconnects forever in
        // relay-blocked environments, triggering an endless recover() loop.
        let is_healthy = has_endpoint && has_gossip && has_content_topic && !broadcast_stale;
        let needs_reconnect = !has_endpoint || !has_gossip || !has_content_topic || broadcast_stale;

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
            "broadcast_stale": broadcast_stale,
            "broadcast_age_secs": broadcast_age_secs,
        });

        println!(
            "[IROH-HEALTH] Health check: healthy={}, needs_reconnect={}",
            is_healthy, needs_reconnect
        );
        if !is_healthy {
            println!("[IROH-HEALTH] Details: {:?}", status);
        }

        (is_healthy, needs_reconnect, status)
    }

    /// True if the database knows of another device belonging to THIS user
    /// (same public key, different device id). Cheap gate for device sync:
    /// single-device users have nobody to sync with, so broadcasting periodic
    /// DeviceSyncRequests mesh-wide is pure noise for them.
    fn has_other_own_devices(&self) -> bool {
        let Some(device_id) = &self.device_id else {
            return false;
        };
        match self.db.get_peer_node_ids(&self.public_key, device_id) {
            Ok(ids) => !ids.is_empty(),
            // Fail open: a broken query shouldn't silently disable device sync
            Err(_) => true,
        }
    }

    /// Request device sync from other devices with the same user account
    /// Called after recovery to catch up on any content missed while disconnected
    async fn request_device_sync(&self) {
        println!("[IROH-SYNC] Requesting device sync after recovery...");

        let Some(sync_request) = self.build_signed_sync_request() else {
            println!("[IROH-SYNC] Cannot sign device sync request (no signing key) - skipping");
            return;
        };

        if let Err(e) = self.publish_message(CONTENT_TOPIC, sync_request).await {
            println!("[IROH-SYNC] Failed to send device sync request: {}", e);
        } else {
            println!("[IROH-SYNC] Device sync request broadcast to mesh");
        }
    }

    /// Recover network connectivity without full reinitialization
    /// This is faster than full shutdown+init and preserves NAT traversal state
    pub async fn recover(&self) -> Result<(), String> {
        // Single-flight guard: foreground events and the frontend health-check loop can
        // race into recover() concurrently, each spawning its own set of loops.
        // ClearOnDrop also resets the flag if the caller's timeout drops this future.
        struct ClearOnDrop(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for ClearOnDrop {
            fn drop(&mut self) {
                self.0.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        if self
            .recovering
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            println!("[IROH-RECOVER] Recovery already in progress - skipping duplicate call");
            return Ok(());
        }
        let _recovering_guard = ClearOnDrop(self.recovering.clone());

        println!("[IROH-RECOVER] Starting network recovery...");

        // Abort the previous generation of background loops BEFORE resetting the shutdown
        // flag. The old loops only check the flag at their next tick (15-30s away, and
        // never while suspended on mobile); resetting the flag first meant they never
        // exited, so every recover() doubled the number of running loops.
        self.stop_background_tasks();

        // Reset shutdown flag to allow new background loops
        self.shutdown_flag
            .store(false, std::sync::atomic::Ordering::Relaxed);

        // Drop stale connection state. After a suspend or network change these still
        // contain peers whose QUIC paths are dead; the discovery loop skips peers it
        // believes are connected, so stale entries blocked reconnection entirely.
        self.clear_connected_peers().await;
        self.peer_heartbeats.lock().await.clear();
        self.peer_retry_counts.lock().await.clear();

        // Check if we need to re-subscribe to the content topic.
        // is_topic_subscribed() treats an entry whose broadcast channel is
        // closed (stream handler died/panicked) as NOT subscribed and removes
        // it - previously such a zombie entry made recover() skip the
        // re-subscribe while publish_message kept failing forever.
        let has_content_topic = self.is_topic_subscribed(CONTENT_TOPIC).await;

        if !has_content_topic {
            println!("[IROH-RECOVER] Content topic missing or handler dead, re-subscribing...");
            self.subscribe_topic(CONTENT_TOPIC).await?;
        }

        // Restart background loops (the old generation was aborted above)
        println!("[IROH-RECOVER] Restarting background loops...");
        self.start_presence_loop();
        self.start_heartbeat_sender();
        self.start_heartbeat_monitor();
        self.start_periodic_device_sync();

        // Try to reconnect to known peers (first pass runs ~1s after spawn)
        self.start_active_peer_discovery();

        // Announce presence immediately
        if let Err(e) = self.announce_presence().await {
            println!(
                "[IROH-RECOVER] Warning: Initial presence announcement failed: {}",
                e
            );
            // Don't fail recovery for this - background loop will retry
        } else {
            println!("[IROH-RECOVER] [OK] Initial presence announced");
        }

        // Send device sync request to catch up on any missed content
        // This ensures we sync even if we missed presence announcements while disconnected
        self.request_device_sync().await;

        println!("[IROH-RECOVER] Recovery complete");
        Ok(())
    }

    /// Shutdown the network
    pub async fn shutdown(&self) -> Result<(), String> {
        println!("[IROH] Shutting down Iroh network...");

        // Signal background loops to stop
        self.shutdown_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);

        // Abort them immediately - waiting for the flag to be observed takes up to a
        // full 30s tick, far longer than callers' shutdown timeout
        self.stop_background_tasks();

        // Close all topic subscriptions
        self.topics.lock().await.clear();

        // Close gossip
        if let Some(gossip) = self.gossip.lock().await.take() {
            drop(gossip);
        }

        // Shut down the blob store so the fs-backed database flushes and
        // releases its lock - a following re-init loads the same directory
        self.downloader.lock().await.take();
        if let Some(store) = self.store.lock().await.take() {
            if let Err(e) = store.shutdown().await {
                println!("[IROH] Warning: blob store shutdown failed: {}", e);
            }
        }

        // Close endpoint
        if let Some(endpoint) = self.endpoint.lock().await.take() {
            let _ = endpoint.close().await;
        }

        println!("[IROH] Shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod validation_tests {
    use super::{is_supported_media_type, is_supported_reaction_emoji};

    #[test]
    fn reaction_emoji_allowlist() {
        // The set the UI actually offers
        for emoji in ["👍", "❤️", "😂", "😮", "😢", "😡"] {
            assert!(
                is_supported_reaction_emoji(emoji),
                "{} should be allowed",
                emoji
            );
        }
        // Anything else - including markup a peer might hope the UI renders
        assert!(!is_supported_reaction_emoji("<img src=x onerror=alert(1)>"));
        assert!(!is_supported_reaction_emoji("👍👍"));
        assert!(!is_supported_reaction_emoji(""));
        assert!(!is_supported_reaction_emoji("🚀"));
    }

    #[test]
    fn media_type_allowlist() {
        assert!(is_supported_media_type("image/png"));
        assert!(is_supported_media_type("IMAGE/PNG"));
        assert!(is_supported_media_type("video/webm; codecs=vp9"));
        // SVG is an active document - never accepted as an image
        assert!(!is_supported_media_type("image/svg+xml"));
        assert!(!is_supported_media_type("text/html"));
        assert!(!is_supported_media_type("\"><script>alert(1)</script>"));
        assert!(!is_supported_media_type(""));
    }
}
