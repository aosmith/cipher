// Network Test Harness for Iroh P2P Testing
//
// This module provides utilities for testing the Iroh-based P2P networking stack.
// It creates isolated test networks with multiple nodes that can communicate.

use futures_lite::StreamExt;
use iroh::address_lookup::memory::MemoryLookup;
use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use iroh_gossip::{
    api::Event,
    net::{Gossip, GOSSIP_ALPN},
    proto::TopicId,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// A test node in the network with its own endpoint and gossip instance
pub struct TestNode {
    pub name: String,
    pub endpoint: Endpoint,
    pub gossip: Gossip,
    pub router: iroh::protocol::Router,
    pub node_id: EndpointId,
    /// Out-of-band peer address registry (replaces 0.35 endpoint.add_node_addr)
    address_book: MemoryLookup,
    subscriptions: Arc<Mutex<HashMap<String, TopicSubscription>>>,
}

/// A subscription to a gossip topic with send/receive capabilities
pub struct TopicSubscription {
    pub topic_id: TopicId,
    sender: iroh_gossip::api::GossipSender,
    receiver: Arc<Mutex<iroh_gossip::api::GossipReceiver>>,
}

impl TestNode {
    /// Create a new test node with a random identity
    pub async fn new(name: &str) -> anyhow::Result<Self> {
        let secret = SecretKey::generate();
        Self::with_secret(name, secret).await
    }

    /// Create a new test node with a specific secret key
    pub async fn with_secret(name: &str, secret: SecretKey) -> anyhow::Result<Self> {
        // Address book lets add_peer() teach this endpoint about peers out-of-band,
        // replacing the removed endpoint.add_node_addr().
        let address_book = MemoryLookup::new();
        let endpoint = Endpoint::builder(presets::N0)
            .secret_key(secret)
            .address_lookup(address_book.clone())
            .bind()
            .await?;

        let node_id = endpoint.id();
        // Gossip::spawn is synchronous in iroh-gossip 0.101
        let gossip = Gossip::builder().spawn(endpoint.clone());
        let router = iroh::protocol::Router::builder(endpoint.clone())
            .accept(GOSSIP_ALPN, gossip.clone())
            .spawn();

        Ok(Self {
            name: name.to_string(),
            endpoint,
            gossip,
            router,
            node_id,
            address_book,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Get this node's full address (for sharing with other nodes)
    pub async fn node_addr(&self) -> anyhow::Result<EndpointAddr> {
        Ok(self.endpoint.addr())
    }

    /// Add another node's address to enable direct connection
    pub fn add_peer(&self, addr: EndpointAddr) -> anyhow::Result<()> {
        self.address_book.add_endpoint_info(addr);
        Ok(())
    }

    /// Connect to a peer via QUIC to verify connectivity
    pub async fn connect_to(&self, peer_id: EndpointId) -> anyhow::Result<()> {
        let conn = self
            .endpoint
            .connect(EndpointAddr::new(peer_id), GOSSIP_ALPN)
            .await?;
        drop(conn); // We just want to verify connectivity
        Ok(())
    }

    /// Subscribe to a topic as a root node (no bootstrap peers)
    pub async fn subscribe_as_root(&self, topic_name: &str) -> anyhow::Result<()> {
        let topic_id = topic_name_to_id(topic_name);
        let subscription = self.gossip.subscribe(topic_id, vec![]).await?;
        let (sender, receiver) = subscription.split();

        let mut subs = self.subscriptions.lock().await;
        subs.insert(
            topic_name.to_string(),
            TopicSubscription {
                topic_id,
                sender,
                receiver: Arc::new(Mutex::new(receiver)),
            },
        );
        Ok(())
    }

    /// Subscribe to a topic and join an existing mesh via bootstrap peers
    pub async fn subscribe_and_join(
        &self,
        topic_name: &str,
        bootstrap_peers: Vec<EndpointId>,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let topic_id = topic_name_to_id(topic_name);

        let subscription = tokio::time::timeout(
            timeout,
            self.gossip.subscribe_and_join(topic_id, bootstrap_peers),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Timeout joining topic {}", topic_name))??;

        let (sender, receiver) = subscription.split();

        let mut subs = self.subscriptions.lock().await;
        subs.insert(
            topic_name.to_string(),
            TopicSubscription {
                topic_id,
                sender,
                receiver: Arc::new(Mutex::new(receiver)),
            },
        );
        Ok(())
    }

    /// Broadcast a message to a topic
    pub async fn broadcast(&self, topic_name: &str, message: &[u8]) -> anyhow::Result<()> {
        let subs = self.subscriptions.lock().await;
        let sub = subs
            .get(topic_name)
            .ok_or_else(|| anyhow::anyhow!("Not subscribed to topic {}", topic_name))?;

        sub.sender
            .broadcast(bytes::Bytes::from(message.to_vec()))
            .await?;
        Ok(())
    }

    /// Wait for a message on a topic with timeout
    pub async fn receive(
        &self,
        topic_name: &str,
        timeout: Duration,
    ) -> anyhow::Result<Option<bytes::Bytes>> {
        let subs = self.subscriptions.lock().await;
        let sub = subs
            .get(topic_name)
            .ok_or_else(|| anyhow::anyhow!("Not subscribed to topic {}", topic_name))?;

        let receiver = sub.receiver.clone();
        drop(subs); // Release lock before waiting

        let mut receiver = receiver.lock().await;

        match tokio::time::timeout(timeout, async {
            while let Some(event) = receiver.try_next().await.transpose() {
                match event {
                    Ok(Event::Received(msg)) => {
                        return Some(msg.content);
                    }
                    Ok(_) => continue, // Skip other events
                    Err(_) => return None,
                }
            }
            None
        })
        .await
        {
            Ok(msg) => Ok(msg),
            Err(_) => Ok(None), // Timeout
        }
    }

    /// Wait for a neighbor up event (mesh formation)
    pub async fn wait_for_neighbor(
        &self,
        topic_name: &str,
        timeout: Duration,
    ) -> anyhow::Result<EndpointId> {
        let subs = self.subscriptions.lock().await;
        let sub = subs
            .get(topic_name)
            .ok_or_else(|| anyhow::anyhow!("Not subscribed to topic {}", topic_name))?;

        let receiver = sub.receiver.clone();
        drop(subs);

        let mut receiver = receiver.lock().await;

        tokio::time::timeout(timeout, async {
            while let Some(event) = receiver.try_next().await.transpose() {
                if let Ok(Event::NeighborUp(node_id)) = event {
                    return Ok(node_id);
                }
            }
            Err(anyhow::anyhow!("Stream ended without neighbor up event"))
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timeout waiting for neighbor"))?
    }

    /// Shutdown this node
    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.router.shutdown().await?;
        Ok(())
    }
}

/// A network of test nodes for integration testing
pub struct TestNetwork {
    pub nodes: Vec<TestNode>,
}

impl TestNetwork {
    /// Create a new empty network
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Create a network with the specified number of nodes
    pub async fn with_nodes(count: usize) -> anyhow::Result<Self> {
        let mut network = Self::new();
        for i in 0..count {
            let name = format!("node_{}", i);
            network.add_node(&name).await?;
        }
        Ok(network)
    }

    /// Add a new node to the network
    pub async fn add_node(&mut self, name: &str) -> anyhow::Result<usize> {
        let node = TestNode::new(name).await?;
        self.nodes.push(node);
        Ok(self.nodes.len() - 1)
    }

    /// Get a node by index
    pub fn node(&self, index: usize) -> &TestNode {
        &self.nodes[index]
    }

    /// Get a mutable node by index
    pub fn node_mut(&mut self, index: usize) -> &mut TestNode {
        &mut self.nodes[index]
    }

    /// Connect all nodes to each other by sharing addresses
    pub async fn connect_all(&self) -> anyhow::Result<()> {
        // Collect all addresses first
        let mut addrs = Vec::new();
        for node in &self.nodes {
            addrs.push((node.node_id, node.node_addr().await?));
        }

        // Add all addresses to all nodes
        for node in &self.nodes {
            for (peer_id, addr) in &addrs {
                if *peer_id != node.node_id {
                    node.add_peer(addr.clone())?;
                }
            }
        }
        Ok(())
    }

    /// Have all nodes subscribe to a topic, with node 0 as root
    pub async fn all_subscribe(&self, topic_name: &str, timeout: Duration) -> anyhow::Result<()> {
        if self.nodes.is_empty() {
            return Ok(());
        }

        // First node subscribes as root
        self.nodes[0].subscribe_as_root(topic_name).await?;

        // Give root time to set up
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Other nodes join with root as bootstrap
        let root_id = self.nodes[0].node_id;
        for node in self.nodes.iter().skip(1) {
            node.subscribe_and_join(topic_name, vec![root_id], timeout)
                .await?;
        }
        Ok(())
    }

    /// Wait for mesh to form (all nodes see at least one neighbor)
    pub async fn wait_for_mesh(&self, topic_name: &str, timeout: Duration) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;

        for node in &self.nodes {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for mesh formation on node {}",
                    node.name
                ));
            }

            // Wait for at least one neighbor
            node.wait_for_neighbor(topic_name, remaining).await?;
        }
        Ok(())
    }

    /// Broadcast from one node and verify all others receive it
    pub async fn broadcast_and_verify(
        &self,
        sender_idx: usize,
        topic_name: &str,
        message: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<()> {
        // Send from the specified node
        self.nodes[sender_idx]
            .broadcast(topic_name, message)
            .await?;

        // Verify all other nodes receive it
        for (i, node) in self.nodes.iter().enumerate() {
            if i == sender_idx {
                continue;
            }

            let received = node.receive(topic_name, timeout).await?;
            match received {
                Some(data) if data.as_ref() == message => {
                    // Success
                }
                Some(data) => {
                    return Err(anyhow::anyhow!(
                        "Node {} received wrong message: expected {:?}, got {:?}",
                        node.name,
                        message,
                        data.as_ref()
                    ));
                }
                None => {
                    return Err(anyhow::anyhow!(
                        "Node {} did not receive message within timeout",
                        node.name
                    ));
                }
            }
        }
        Ok(())
    }

    /// Shutdown all nodes
    pub async fn shutdown(self) -> anyhow::Result<()> {
        for node in self.nodes {
            node.shutdown().await?;
        }
        Ok(())
    }
}

/// Convert a topic name to a TopicId using blake3 hash
pub fn topic_name_to_id(name: &str) -> TopicId {
    TopicId::from_bytes(*blake3::hash(name.as_bytes()).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_node_creation() -> anyhow::Result<()> {
        let node = TestNode::new("test").await?;
        assert!(!node.name.is_empty());
        assert!(node.node_id.as_bytes().len() == 32);
        node.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_topic_id_consistency() {
        let id1 = topic_name_to_id("test/topic");
        let id2 = topic_name_to_id("test/topic");
        let id3 = topic_name_to_id("other/topic");

        assert_eq!(id1, id2, "Same topic name should produce same ID");
        assert_ne!(
            id1, id3,
            "Different topic names should produce different IDs"
        );
    }

    #[tokio::test]
    async fn test_network_creation() -> anyhow::Result<()> {
        let network = TestNetwork::with_nodes(3).await?;
        assert_eq!(network.nodes.len(), 3);
        network.shutdown().await?;
        Ok(())
    }
}
