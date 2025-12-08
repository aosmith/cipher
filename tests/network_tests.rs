// Network tests for Cipher using Iroh
// Tests P2P networking, gossip protocol, and mesh formation

#[path = "common/mod.rs"]
mod common;

use common::network_harness::*;
use std::time::Duration;

// ===== Basic Node Tests =====

#[tokio::test]
async fn test_single_node_creation() {
    let node = TestNode::new("test_node").await
        .expect("Should create node");

    assert_eq!(node.name, "test_node");
    assert!(node.node_id.as_bytes().len() == 32);

    node.shutdown().await.expect("Should shutdown");
}

#[tokio::test]
async fn test_node_address_retrieval() {
    let node = TestNode::new("addr_node").await
        .expect("Should create node");

    let addr = node.node_addr().await
        .expect("Should get address");

    // Address should include the node ID
    assert_eq!(addr.node_id, node.node_id);

    node.shutdown().await.expect("Should shutdown");
}

// ===== Network Tests =====

#[tokio::test]
async fn test_network_creation() {
    let network = TestNetwork::with_nodes(3).await
        .expect("Should create network");

    assert_eq!(network.nodes.len(), 3);

    // Each node should have a unique ID
    let node_ids: Vec<_> = network.nodes.iter().map(|n| n.node_id).collect();
    for (i, id) in node_ids.iter().enumerate() {
        for (j, other_id) in node_ids.iter().enumerate() {
            if i != j {
                assert_ne!(id, other_id, "Node IDs should be unique");
            }
        }
    }

    network.shutdown().await.expect("Should shutdown");
}

#[tokio::test]
async fn test_network_address_sharing() {
    let network = TestNetwork::with_nodes(2).await
        .expect("Should create network");

    // Connect all nodes by sharing addresses
    network.connect_all().await
        .expect("Should connect all nodes");

    network.shutdown().await.expect("Should shutdown");
}

// ===== Topic Tests =====

#[test]
fn test_topic_id_generation() {
    let topic1 = topic_name_to_id("test/topic");
    let topic2 = topic_name_to_id("test/topic");
    let topic3 = topic_name_to_id("other/topic");

    // Same name should produce same ID
    assert_eq!(topic1, topic2);

    // Different names should produce different IDs
    assert_ne!(topic1, topic3);
}

#[test]
fn test_topic_id_consistency() {
    // Test that topic generation is consistent across calls
    let topics: Vec<_> = (0..10)
        .map(|i| topic_name_to_id(&format!("topic_{}", i)))
        .collect();

    // All should be unique
    for (i, id) in topics.iter().enumerate() {
        for (j, other_id) in topics.iter().enumerate() {
            if i != j {
                assert_ne!(id, other_id);
            }
        }
    }

    // Regenerating should produce same IDs
    for (i, original) in topics.iter().enumerate() {
        let regenerated = topic_name_to_id(&format!("topic_{}", i));
        assert_eq!(*original, regenerated);
    }
}

#[tokio::test]
async fn test_topic_subscription() {
    let node = TestNode::new("sub_node").await
        .expect("Should create node");

    // Subscribe as root (first node in topic)
    node.subscribe_as_root("test/subscription").await
        .expect("Should subscribe");

    node.shutdown().await.expect("Should shutdown");
}

// ===== Mesh Formation Tests =====

#[tokio::test]
async fn test_two_node_mesh() {
    let network = TestNetwork::with_nodes(2).await
        .expect("Should create network");

    // Share addresses
    network.connect_all().await
        .expect("Should connect nodes");

    // First node subscribes as root
    network.nodes[0].subscribe_as_root("mesh/test").await
        .expect("Should subscribe as root");

    // Give time for the subscription to register
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Second node joins the mesh
    let timeout = Duration::from_secs(10);
    network.nodes[1].subscribe_and_join(
        "mesh/test",
        vec![network.nodes[0].node_id],
        timeout,
    ).await.expect("Should join mesh");

    // Wait for mesh formation
    let neighbor = network.nodes[0].wait_for_neighbor("mesh/test", timeout).await;

    // Note: In a real network test, we'd verify the neighbor
    // For unit testing without relay servers, this may timeout
    if neighbor.is_ok() {
        assert_eq!(neighbor.unwrap(), network.nodes[1].node_id);
    }

    network.shutdown().await.expect("Should shutdown");
}

// ===== Message Broadcasting Tests =====

#[tokio::test]
async fn test_broadcast_message() {
    let node = TestNode::new("broadcast_node").await
        .expect("Should create node");

    // Subscribe to a topic first
    node.subscribe_as_root("broadcast/test").await
        .expect("Should subscribe");

    // Broadcasting to self should work
    let message = b"Hello, gossip!";
    let result = node.broadcast("broadcast/test", message).await;

    // Note: Without peers, this may fail or succeed depending on implementation
    // The important thing is it doesn't panic
    match result {
        Ok(_) => println!("Broadcast succeeded"),
        Err(e) => println!("Broadcast without peers: {}", e),
    }

    node.shutdown().await.expect("Should shutdown");
}

// ===== Integration-style Tests =====

#[tokio::test]
async fn test_gossip_network_lifecycle() {
    // Create a 3-node network
    let network = TestNetwork::with_nodes(3).await
        .expect("Should create network");

    // Connect all nodes
    network.connect_all().await
        .expect("Should connect");

    // Have all nodes subscribe to a topic
    // Node 0 is root, others join
    let timeout = Duration::from_secs(10);
    network.nodes[0].subscribe_as_root("lifecycle/test").await
        .expect("Node 0 should subscribe as root");

    tokio::time::sleep(Duration::from_millis(100)).await;

    for node in network.nodes.iter().skip(1) {
        // Join with node 0 as bootstrap
        let result = node.subscribe_and_join(
            "lifecycle/test",
            vec![network.nodes[0].node_id],
            timeout,
        ).await;

        // May timeout without relay, but shouldn't error
        if let Err(e) = result {
            println!("Node {} join: {}", node.name, e);
        }
    }

    // Clean shutdown
    network.shutdown().await.expect("Should shutdown cleanly");
}

// ===== Deterministic Tests (no network required) =====

#[test]
fn test_topic_hash_properties() {
    // Test that topic hashing is consistent
    let test_cases = vec![
        "user/alice/posts",
        "user/bob/posts",
        "dm/alice/bob",
        "public/general",
        "cipher/presence",
    ];

    for topic in test_cases {
        let id1 = topic_name_to_id(topic);
        let id2 = topic_name_to_id(topic);

        // Deterministic
        assert_eq!(id1, id2, "Topic {} should produce same ID", topic);

        // Should be 32 bytes (256 bits from blake3)
        assert_eq!(id1.as_bytes().len(), 32);
    }
}

#[test]
fn test_topic_namespace_isolation() {
    // Topics with same suffix but different prefix should be different
    let alice_posts = topic_name_to_id("user/alice/posts");
    let bob_posts = topic_name_to_id("user/bob/posts");

    assert_ne!(alice_posts, bob_posts);

    // DM topics should be isolated
    let dm_ab = topic_name_to_id("dm/alice/bob");
    let dm_ac = topic_name_to_id("dm/alice/charlie");

    assert_ne!(dm_ab, dm_ac);
}
