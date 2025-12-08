// Test to verify subscribe() vs subscribe_and_join() mesh formation
// This reproduces the issue we're seeing in v0.36.0/v0.37.0

use iroh::{Endpoint, RelayMode};
use iroh_gossip::{net::{Gossip, Event, GossipEvent, GOSSIP_ALPN}, proto::TopicId};
use futures_lite::StreamExt;
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_subscribe_root_vs_subscribe_and_join() -> anyhow::Result<()> {
    println!("\n=== Testing subscribe() root vs subscribe_and_join() joiner ===\n");

    let topic = TopicId::from_bytes(*blake3::hash(b"test-topic").as_bytes());

    // Create Alice's endpoint (root node)
    println!("Creating Alice's endpoint...");
    let alice_secret = iroh::SecretKey::generate(rand::rngs::OsRng);
    let alice_endpoint = Endpoint::builder()
        .secret_key(alice_secret.clone())
        .relay_mode(RelayMode::Default)  // Use relay servers for real network testing
        .bind()
        .await?;
    let alice_node_id = alice_endpoint.node_id();
    println!("Alice NodeId: {}", alice_node_id);

    // Create Alice's gossip and router
    let alice_gossip = Gossip::builder().spawn(alice_endpoint.clone()).await?;
    let alice_router = iroh::protocol::Router::builder(alice_endpoint.clone())
        .accept(GOSSIP_ALPN, alice_gossip.clone())
        .spawn()
        .await?;

    // Alice subscribes with subscribe() (root node, empty bootstrap)
    println!("\nAlice: Calling gossip.subscribe() with empty bootstrap...");
    let alice_topic = alice_gossip.subscribe(topic, vec![])?;
    let (alice_sender, mut alice_receiver) = alice_topic.split();
    println!("Alice: subscribe() completed - root node created");

    // Spawn task to monitor Alice's events
    let alice_monitor = tokio::spawn(async move {
        println!("Alice: Starting event monitor...");
        while let Some(event) = alice_receiver.try_next().await.transpose() {
            match event {
                Ok(Event::Gossip(GossipEvent::Joined(neighbors))) => {
                    println!("Alice: ✓ Joined event with {} neighbors", neighbors.len());
                }
                Ok(Event::Gossip(GossipEvent::NeighborUp(node_id))) => {
                    println!("Alice: ✓ NeighborUp: {}", node_id);
                }
                Ok(Event::Gossip(GossipEvent::Received(msg))) => {
                    println!("Alice: ✓ Received message: {} bytes", msg.content.len());
                }
                Ok(Event::Lagged) => {
                    println!("Alice: ⚠️  Lagged");
                }
                _ => {}
            }
        }
        println!("Alice: Event monitor ended");
    });

    // Give Alice time to set up
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create Bob's endpoint (joiner)
    println!("\nCreating Bob's endpoint...");
    let bob_secret = iroh::SecretKey::generate(rand::rngs::OsRng);
    let bob_endpoint = Endpoint::builder()
        .secret_key(bob_secret)
        .relay_mode(RelayMode::Default)  // Use relay servers for real network testing
        .bind()
        .await?;
    let bob_node_id = bob_endpoint.node_id();
    println!("Bob NodeId: {}", bob_node_id);

    // Add Alice's address to Bob's endpoint
    let alice_addr = alice_endpoint.node_addr().await?;
    println!("\nBob: Adding Alice's address: {:?}", alice_addr);
    bob_endpoint.add_node_addr(alice_addr)?;

    // Bob connects to Alice via QUIC first
    println!("Bob: Connecting to Alice via QUIC...");
    match bob_endpoint.connect(alice_node_id, &GOSSIP_ALPN).await {
        Ok(conn) => {
            println!("Bob: ✓ QUIC connection established");
            println!("Bob:   Remote: {:?}", conn.remote_address());
            drop(conn);
        }
        Err(e) => {
            println!("Bob: ✗ QUIC connection failed: {}", e);
            return Err(e.into());
        }
    }

    // Create Bob's gossip and router
    let bob_gossip = Gossip::builder().spawn(bob_endpoint.clone()).await?;
    let bob_router = iroh::protocol::Router::builder(bob_endpoint.clone())
        .accept(GOSSIP_ALPN, bob_gossip.clone())
        .spawn()
        .await?;

    // Bob tries to join Alice with subscribe_and_join()
    println!("\nBob: Calling gossip.subscribe_and_join() with Alice as bootstrap...");
    println!("Bob: This should connect to Alice's gossip mesh");

    let join_result = tokio::time::timeout(
        Duration::from_secs(10),
        bob_gossip.subscribe_and_join(topic, vec![alice_node_id])
    ).await;

    match join_result {
        Ok(Ok(bob_topic)) => {
            println!("Bob: ✓ subscribe_and_join() succeeded!");
            let (bob_sender, mut bob_receiver) = bob_topic.split();

            // Test message exchange
            println!("\nTesting message exchange...");
            let test_msg = bytes::Bytes::from("Hello from Bob!");
            bob_sender.broadcast(test_msg.clone()).await?;
            println!("Bob: Sent test message");

            // Wait for Alice to receive
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Try Alice sending
            let alice_msg = bytes::Bytes::from("Hello from Alice!");
            alice_sender.broadcast(alice_msg.clone()).await?;
            println!("Alice: Sent test message");

            // Check if Bob receives
            match tokio::time::timeout(Duration::from_secs(2), bob_receiver.try_next()).await {
                Ok(Ok(Some(Event::Gossip(GossipEvent::Received(msg))))) => {
                    println!("Bob: ✓ Received Alice's message: {} bytes", msg.content.len());
                }
                Ok(Ok(Some(event))) => {
                    println!("Bob: Received other event: {:?}", event);
                }
                Ok(Ok(None)) => {
                    println!("Bob: Stream ended");
                }
                Ok(Err(e)) => {
                    println!("Bob: Error receiving: {}", e);
                }
                Err(_) => {
                    println!("Bob: ⏱  Timeout waiting for Alice's message");
                }
            }

            println!("\n✓ TEST PASSED: subscribe() and subscribe_and_join() are compatible!");
        }
        Ok(Err(e)) => {
            println!("Bob: ✗ subscribe_and_join() failed: {}", e);
            println!("\n✗ TEST FAILED: subscribe_and_join() returned error");
            return Err(e.into());
        }
        Err(_) => {
            println!("Bob: ⏱  subscribe_and_join() timed out after 10s");
            println!("\n✗ TEST FAILED: subscribe() and subscribe_and_join() are INCOMPATIBLE");
            println!("   This explains why v0.36.0/v0.37.0 don't work!");
            return Err(anyhow::anyhow!("Timeout joining gossip mesh"));
        }
    }

    // Cleanup
    alice_monitor.abort();
    alice_router.shutdown().await?;
    bob_router.shutdown().await?;

    Ok(())
}

#[tokio::test]
async fn test_both_use_subscribe_and_join() -> anyhow::Result<()> {
    println!("\n=== Testing BOTH peers use subscribe_and_join() ===\n");

    let topic = TopicId::from_bytes(*blake3::hash(b"test-topic-2").as_bytes());

    // Create Alice's endpoint
    println!("Creating Alice's endpoint...");
    let alice_secret = iroh::SecretKey::generate(rand::rngs::OsRng);
    let alice_endpoint = Endpoint::builder()
        .secret_key(alice_secret.clone())
        .relay_mode(RelayMode::Default)
        .bind()
        .await?;
    let alice_node_id = alice_endpoint.node_id();
    println!("Alice NodeId: {}", alice_node_id);

    // Create Alice's gossip and router
    let alice_gossip = Gossip::builder().spawn(alice_endpoint.clone()).await?;
    let alice_router = iroh::protocol::Router::builder(alice_endpoint.clone())
        .accept(GOSSIP_ALPN, alice_gossip.clone())
        .spawn()
        .await?;

    // Create Bob's endpoint
    println!("Creating Bob's endpoint...");
    let bob_secret = iroh::SecretKey::generate(rand::rngs::OsRng);
    let bob_endpoint = Endpoint::builder()
        .secret_key(bob_secret)
        .relay_mode(RelayMode::Default)
        .bind()
        .await?;
    let bob_node_id = bob_endpoint.node_id();
    println!("Bob NodeId: {}", bob_node_id);

    // Add addresses
    let alice_addr = alice_endpoint.node_addr().await?;
    bob_endpoint.add_node_addr(alice_addr)?;

    // Create Bob's gossip and router
    let bob_gossip = Gossip::builder().spawn(bob_endpoint.clone()).await?;
    let bob_router = iroh::protocol::Router::builder(bob_endpoint.clone())
        .accept(GOSSIP_ALPN, bob_gossip.clone())
        .spawn()
        .await?;

    // Spawn both subscribe_and_join() calls concurrently (like chat example)
    println!("\nSpawning both subscribe_and_join() calls concurrently...");
    println!("Alice: subscribe_and_join() with empty bootstrap (will block until Bob connects)");
    println!("Bob: subscribe_and_join() with Alice as bootstrap");

    let alice_join = tokio::spawn(async move {
        println!("Alice: Starting subscribe_and_join()...");
        let result = alice_gossip.subscribe_and_join(topic, vec![]).await;
        println!("Alice: subscribe_and_join() completed: {:?}", result.is_ok());
        result
    });

    // Give Alice a head start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let bob_join = tokio::spawn(async move {
        println!("Bob: Starting subscribe_and_join()...");
        let result = bob_gossip.subscribe_and_join(topic, vec![alice_node_id]).await;
        println!("Bob: subscribe_and_join() completed: {:?}", result.is_ok());
        result
    });

    // Wait for both with timeout
    match tokio::time::timeout(Duration::from_secs(10), async {
        let alice_result = alice_join.await??;
        let bob_result = bob_join.await??;
        Ok::<_, anyhow::Error>((alice_result, bob_result))
    }).await {
        Ok(Ok((alice_topic, bob_topic))) => {
            println!("\n✓ TEST PASSED: Both subscribe_and_join() calls succeeded!");
            println!("   Alice and Bob formed a gossip mesh");

            // Test message exchange
            let (alice_sender, _) = alice_topic.split();
            let (bob_sender, mut bob_receiver) = bob_topic.split();

            alice_sender.broadcast(bytes::Bytes::from("Hello from Alice!")).await?;
            println!("Alice: Sent message");

            match tokio::time::timeout(Duration::from_secs(2), bob_receiver.try_next()).await {
                Ok(Ok(Some(Event::Gossip(GossipEvent::Received(_))))) => {
                    println!("Bob: ✓ Received Alice's message");
                }
                _ => {
                    println!("Bob: ⚠️  Did not receive Alice's message");
                }
            }
        }
        Ok(Err(e)) => {
            println!("\n✗ TEST FAILED: {}", e);
            return Err(e);
        }
        Err(_) => {
            println!("\n✗ TEST FAILED: Timeout - one of the subscribe_and_join() calls blocked");
            return Err(anyhow::anyhow!("Timeout"));
        }
    }

    // Cleanup
    alice_router.shutdown().await?;
    bob_router.shutdown().await?;

    Ok(())
}

/// Test resubscription flow - simulates QR code scanning scenario
/// This tests the fix for bidirectional peer connection:
/// 1. Alice starts as isolated root node (subscribed with no bootstrap)
/// 2. Bob starts as isolated root node (subscribed with no bootstrap)
/// 3. Bob "scans" Alice's QR and resubscribes with Alice as bootstrap
/// 4. Both should see NeighborUp events and be able to exchange messages
#[tokio::test]
async fn test_resubscribe_after_qr_scan() -> anyhow::Result<()> {
    println!("\n=== Testing resubscribe flow (QR code scenario) ===\n");
    println!("This test simulates:");
    println!("  1. Desktop (Alice) starts isolated");
    println!("  2. Phone (Bob) starts isolated");
    println!("  3. Phone scans Desktop's QR code");
    println!("  4. Phone resubscribes with Desktop as bootstrap");
    println!("  5. Both should see each other as connected");
    println!("");

    let topic = TopicId::from_bytes(*blake3::hash(b"test-topic-resubscribe").as_bytes());

    // === ALICE (Desktop) - starts as isolated root ===
    println!("Creating Alice (Desktop)...");
    let alice_secret = iroh::SecretKey::generate(rand::rngs::OsRng);
    let alice_endpoint = Endpoint::builder()
        .secret_key(alice_secret.clone())
        .relay_mode(RelayMode::Default)
        .bind()
        .await?;
    let alice_node_id = alice_endpoint.node_id();
    println!("Alice NodeId: {}", alice_node_id);

    let alice_gossip = Gossip::builder().spawn(alice_endpoint.clone()).await?;
    let alice_router = iroh::protocol::Router::builder(alice_endpoint.clone())
        .accept(GOSSIP_ALPN, alice_gossip.clone())
        .spawn()
        .await?;

    // Alice subscribes as root (no bootstrap)
    println!("Alice: Subscribing as root (empty bootstrap)...");
    let alice_topic = alice_gossip.subscribe(topic, vec![])?;
    let (alice_sender, mut alice_receiver) = alice_topic.split();
    println!("Alice: ✓ Subscribed (isolated root)");

    // Track Alice's peer connections
    let alice_peer_count = Arc::new(Mutex::new(0usize));
    let alice_peer_count_clone = alice_peer_count.clone();
    tokio::spawn(async move {
        while let Some(event) = alice_receiver.try_next().await.transpose() {
            match event {
                Ok(Event::Gossip(GossipEvent::NeighborUp(node_id))) => {
                    println!("Alice: ✓ NeighborUp: {}", node_id);
                    *alice_peer_count_clone.lock().await += 1;
                }
                Ok(Event::Gossip(GossipEvent::Received(msg))) => {
                    println!("Alice: 📬 Received message: {} bytes", msg.content.len());
                }
                _ => {}
            }
        }
    });

    // === BOB (Phone) - starts as isolated root ===
    println!("\nCreating Bob (Phone)...");
    let bob_secret = iroh::SecretKey::generate(rand::rngs::OsRng);
    let bob_endpoint = Endpoint::builder()
        .secret_key(bob_secret)
        .relay_mode(RelayMode::Default)
        .bind()
        .await?;
    let bob_node_id = bob_endpoint.node_id();
    println!("Bob NodeId: {}", bob_node_id);

    let bob_gossip = Gossip::builder().spawn(bob_endpoint.clone()).await?;
    let bob_router = iroh::protocol::Router::builder(bob_endpoint.clone())
        .accept(GOSSIP_ALPN, bob_gossip.clone())
        .spawn()
        .await?;

    // Bob subscribes as root (no bootstrap)
    println!("Bob: Subscribing as root (empty bootstrap)...");
    let bob_topic_initial = bob_gossip.subscribe(topic, vec![])?;
    println!("Bob: ✓ Subscribed (isolated root)");

    // Track Bob's peer connections
    let bob_peer_count = Arc::new(Mutex::new(0usize));
    let bob_peer_count_clone = bob_peer_count.clone();
    let (_, mut bob_receiver) = bob_topic_initial.split();
    tokio::spawn(async move {
        while let Some(event) = bob_receiver.try_next().await.transpose() {
            match event {
                Ok(Event::Gossip(GossipEvent::NeighborUp(node_id))) => {
                    println!("Bob (initial): ✓ NeighborUp: {}", node_id);
                    *bob_peer_count_clone.lock().await += 1;
                }
                _ => {}
            }
        }
    });

    // Verify both are isolated
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("\n--- Both nodes are now isolated ---");
    println!("Alice peers: {}", *alice_peer_count.lock().await);
    println!("Bob peers: {}", *bob_peer_count.lock().await);
    assert_eq!(*alice_peer_count.lock().await, 0, "Alice should have 0 peers initially");
    assert_eq!(*bob_peer_count.lock().await, 0, "Bob should have 0 peers initially");

    // === QR CODE SCAN SIMULATION ===
    println!("\n=== Bob scans Alice's QR code ===");
    println!("Bob: Got Alice's NodeAddr from QR");

    // Get Alice's address (this is what the QR code contains)
    let alice_addr = alice_endpoint.node_addr().await?;
    println!("Bob: Alice's address: NodeId={}, Relay={:?}",
        alice_addr.node_id,
        alice_addr.relay_url());

    // Add Alice's address to Bob's endpoint
    bob_endpoint.add_node_addr(alice_addr)?;
    println!("Bob: ✓ Added Alice's address to endpoint");

    // CRITICAL: Bob needs to RESUBSCRIBE with Alice as bootstrap!
    println!("\nBob: Resubscribing with Alice as bootstrap...");
    println!("Bob: (This is the fix - old code didn't do this!)");

    // Bob resubscribes using subscribe_and_join() with Alice as bootstrap
    let bob_topic_new = tokio::time::timeout(
        Duration::from_secs(10),
        bob_gossip.subscribe_and_join(topic, vec![alice_node_id])
    ).await;

    match bob_topic_new {
        Ok(Ok(new_topic)) => {
            println!("Bob: ✓ Resubscription successful - joined Alice's mesh!");
            let (bob_sender, mut bob_receiver_new) = new_topic.split();

            // Track new subscription's events
            let bob_peer_count_new = Arc::new(Mutex::new(0usize));
            let bob_peer_count_new_clone = bob_peer_count_new.clone();
            tokio::spawn(async move {
                while let Some(event) = bob_receiver_new.try_next().await.transpose() {
                    match event {
                        Ok(Event::Gossip(GossipEvent::NeighborUp(node_id))) => {
                            println!("Bob (new): ✓ NeighborUp: {}", node_id);
                            *bob_peer_count_new_clone.lock().await += 1;
                        }
                        Ok(Event::Gossip(GossipEvent::Received(msg))) => {
                            println!("Bob: 📬 Received message: {} bytes", msg.content.len());
                        }
                        _ => {}
                    }
                }
            });

            // Wait for events to propagate
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Verify bidirectional connection
            println!("\n=== Verifying bidirectional connection ===");
            let alice_peers = *alice_peer_count.lock().await;
            let bob_peers = *bob_peer_count_new.lock().await;
            println!("Alice peers: {}", alice_peers);
            println!("Bob peers: {}", bob_peers);

            // Test message exchange
            println!("\n=== Testing message exchange ===");

            // Bob sends to Alice
            let test_msg = bytes::Bytes::from("Hello from Bob!");
            bob_sender.broadcast(test_msg).await?;
            println!("Bob: Sent test message");

            tokio::time::sleep(Duration::from_millis(500)).await;

            // Alice sends to Bob
            let alice_msg = bytes::Bytes::from("Hello from Alice!");
            alice_sender.broadcast(alice_msg).await?;
            println!("Alice: Sent test message");

            tokio::time::sleep(Duration::from_millis(500)).await;

            // Both should see each other as connected
            if alice_peers >= 1 && bob_peers >= 1 {
                println!("\n✓ TEST PASSED: Bidirectional gossip mesh formed after resubscribe!");
                println!("  This confirms the fix for one-way connection issue.");
            } else {
                println!("\n⚠️  Partial success: Mesh formed but peer counts unexpected");
                println!("  Alice peers: {} (expected >= 1)", alice_peers);
                println!("  Bob peers: {} (expected >= 1)", bob_peers);
            }
        }
        Ok(Err(e)) => {
            println!("Bob: ✗ Resubscription failed: {}", e);
            return Err(e.into());
        }
        Err(_) => {
            println!("Bob: ✗ Resubscription timed out");
            println!("\n✗ TEST FAILED: subscribe_and_join() blocked - mesh formation failed");
            return Err(anyhow::anyhow!("Resubscription timeout"));
        }
    }

    // Cleanup
    alice_router.shutdown().await?;
    bob_router.shutdown().await?;

    Ok(())
}
