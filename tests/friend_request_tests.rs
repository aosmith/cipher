// Integration tests for friend request/accept flow:
// - P2PMessage serialization/deserialization
// - Friend request/accept filtering logic
// - User ID computation from public key

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    /// Simplified SqliteUuid for testing (mirrors the real implementation)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TestUuid(pub Uuid);

    impl TestUuid {
        pub fn new() -> Self {
            TestUuid(Uuid::new_v4())
        }

        /// Compute deterministic user_id from public key (same as real implementation)
        pub fn from_public_key(public_key: &str) -> Self {
            let namespace = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
            TestUuid(Uuid::new_v5(&namespace, public_key.as_bytes()))
        }
    }

    /// Test P2PMessage enum for testing (mirrors the real implementation)
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(tag = "type")]
    pub enum TestP2PMessage {
        FriendRequest {
            from_public_key: String,
            from_user_id: TestUuid,
            from_display_name: String,
            from_node_id: String,
            from_relay_url: String,
            to_public_key: String,
            timestamp: i64,
        },
        FriendAccepted {
            from_user_id: TestUuid,
            from_public_key: String,
            from_display_name: String,
            from_node_id: String,
            from_relay_url: String,
            to_public_key: String,
        },
        Presence {
            user_id: TestUuid,
            public_key: String,
            device_id: String,
            timestamp: i64,
            display_name: String,
        },
        Heartbeat {
            node_id: String,
            timestamp: i64,
        },
    }

    // ===== Serialization Tests =====

    #[test]
    fn test_friend_request_serialization() {
        let alice_pub_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string();
        let bob_pub_key = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string();
        let alice_user_id = TestUuid::from_public_key(&alice_pub_key);

        let request = TestP2PMessage::FriendRequest {
            from_public_key: alice_pub_key.clone(),
            from_user_id: alice_user_id,
            from_display_name: "Alice".to_string(),
            from_node_id: "a0540bcbbbcb5b184c33bf36ef9f7d1fe5b0e9a7335688436177e277fa291e55"
                .to_string(),
            from_relay_url: "https://aps1-1.relay.iroh.network./".to_string(),
            to_public_key: bob_pub_key.clone(),
            timestamp: 1234567890,
        };

        // Serialize
        let json = serde_json::to_string(&request).expect("Failed to serialize FriendRequest");
        println!("FriendRequest JSON size: {} bytes", json.len());
        println!("FriendRequest JSON: {}", json);

        // Deserialize
        let deserialized: TestP2PMessage =
            serde_json::from_str(&json).expect("Failed to deserialize FriendRequest");

        assert_eq!(
            request, deserialized,
            "FriendRequest should survive serialization round-trip"
        );
    }

    #[test]
    fn test_friend_accepted_serialization() {
        let alice_pub_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string();
        let bob_pub_key = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string();
        let bob_user_id = TestUuid::from_public_key(&bob_pub_key);

        let accepted = TestP2PMessage::FriendAccepted {
            from_user_id: bob_user_id,
            from_public_key: bob_pub_key.clone(),
            from_display_name: "Bob".to_string(),
            from_node_id: "67d2e9e195fa9f95e16ea57edaa87493cd125f3ce595c75d058c2616a08d2153"
                .to_string(),
            from_relay_url: "https://aps1-1.relay.iroh.network./".to_string(),
            to_public_key: alice_pub_key.clone(),
        };

        // Serialize
        let json = serde_json::to_string(&accepted).expect("Failed to serialize FriendAccepted");
        println!("FriendAccepted JSON size: {} bytes", json.len());
        println!("FriendAccepted JSON: {}", json);

        // Deserialize
        let deserialized: TestP2PMessage =
            serde_json::from_str(&json).expect("Failed to deserialize FriendAccepted");

        assert_eq!(
            accepted, deserialized,
            "FriendAccepted should survive serialization round-trip"
        );
    }

    #[test]
    fn test_message_type_discrimination() {
        // Test that the serde tag discriminator works correctly
        let friend_request_json = r#"{"type":"FriendRequest","from_public_key":"key1","from_user_id":"550e8400-e29b-41d4-a716-446655440000","from_display_name":"Alice","from_node_id":"node1","from_relay_url":"relay1","to_public_key":"key2","timestamp":123}"#;
        let friend_accepted_json = r#"{"type":"FriendAccepted","from_user_id":"550e8400-e29b-41d4-a716-446655440000","from_public_key":"key1","from_display_name":"Bob","from_node_id":"node1","from_relay_url":"relay1","to_public_key":"key2"}"#;
        let heartbeat_json = r#"{"type":"Heartbeat","node_id":"node1","timestamp":123}"#;

        let request: TestP2PMessage =
            serde_json::from_str(friend_request_json).expect("Failed to parse FriendRequest");
        let accepted: TestP2PMessage =
            serde_json::from_str(friend_accepted_json).expect("Failed to parse FriendAccepted");
        let heartbeat: TestP2PMessage =
            serde_json::from_str(heartbeat_json).expect("Failed to parse Heartbeat");

        // Verify correct type discrimination
        assert!(matches!(request, TestP2PMessage::FriendRequest { .. }));
        assert!(matches!(accepted, TestP2PMessage::FriendAccepted { .. }));
        assert!(matches!(heartbeat, TestP2PMessage::Heartbeat { .. }));
    }

    // ===== Filtering Logic Tests =====

    #[test]
    fn test_friend_accepted_filtering_for_recipient() {
        let alice_pub_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string();
        let bob_pub_key = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string();

        // Bob sends FriendAccepted to Alice
        let accepted = TestP2PMessage::FriendAccepted {
            from_user_id: TestUuid::from_public_key(&bob_pub_key),
            from_public_key: bob_pub_key.clone(),
            from_display_name: "Bob".to_string(),
            from_node_id: "node1".to_string(),
            from_relay_url: "relay1".to_string(),
            to_public_key: alice_pub_key.clone(), // Target: Alice
        };

        // Simulate Alice receiving the message
        let my_public_key = &alice_pub_key;
        if let TestP2PMessage::FriendAccepted { to_public_key, .. } = &accepted {
            let is_for_me = to_public_key == my_public_key;
            assert!(
                is_for_me,
                "Alice should receive FriendAccepted intended for her"
            );
        } else {
            panic!("Expected FriendAccepted");
        }
    }

    #[test]
    fn test_friend_accepted_filtering_not_for_us() {
        let alice_pub_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string();
        let bob_pub_key = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string();
        let charlie_pub_key = "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZc=".to_string();

        // Bob sends FriendAccepted to Alice
        let accepted = TestP2PMessage::FriendAccepted {
            from_user_id: TestUuid::from_public_key(&bob_pub_key),
            from_public_key: bob_pub_key.clone(),
            from_display_name: "Bob".to_string(),
            from_node_id: "node1".to_string(),
            from_relay_url: "relay1".to_string(),
            to_public_key: alice_pub_key.clone(), // Target: Alice
        };

        // Simulate Charlie (not Alice) receiving the message
        let my_public_key = &charlie_pub_key;
        if let TestP2PMessage::FriendAccepted { to_public_key, .. } = &accepted {
            let is_for_me = to_public_key == my_public_key;
            assert!(
                !is_for_me,
                "Charlie should NOT receive FriendAccepted intended for Alice"
            );
        } else {
            panic!("Expected FriendAccepted");
        }
    }

    #[test]
    fn test_self_message_filtering() {
        let bob_pub_key = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string();

        // Bob receives his own FriendAccepted (echoed back via gossip)
        let accepted = TestP2PMessage::FriendAccepted {
            from_user_id: TestUuid::from_public_key(&bob_pub_key),
            from_public_key: bob_pub_key.clone(),
            from_display_name: "Bob".to_string(),
            from_node_id: "node1".to_string(),
            from_relay_url: "relay1".to_string(),
            to_public_key: "alice_key".to_string(),
        };

        // Bob should filter out messages from himself
        let my_public_key = &bob_pub_key;
        if let TestP2PMessage::FriendAccepted {
            from_public_key, ..
        } = &accepted
        {
            let is_from_me = from_public_key == my_public_key;
            assert!(is_from_me, "Bob should detect this message is from himself");
            // In real code, we'd skip processing if from_public_key == self.public_key
        } else {
            panic!("Expected FriendAccepted");
        }
    }

    // ===== User ID Computation Tests =====

    #[test]
    fn test_deterministic_user_id_from_public_key() {
        let public_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=";

        // Compute user_id twice - should be identical
        let user_id_1 = TestUuid::from_public_key(public_key);
        let user_id_2 = TestUuid::from_public_key(public_key);

        assert_eq!(
            user_id_1, user_id_2,
            "User ID should be deterministic from public key"
        );
    }

    #[test]
    fn test_different_keys_different_user_ids() {
        let alice_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=";
        let bob_key = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=";

        let alice_id = TestUuid::from_public_key(alice_key);
        let bob_id = TestUuid::from_public_key(bob_key);

        assert_ne!(
            alice_id, bob_id,
            "Different public keys should produce different user IDs"
        );
    }

    #[test]
    fn test_user_id_matches_across_serialization() {
        let alice_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=";
        let alice_id = TestUuid::from_public_key(alice_key);

        let request = TestP2PMessage::FriendRequest {
            from_public_key: alice_key.to_string(),
            from_user_id: alice_id,
            from_display_name: "Alice".to_string(),
            from_node_id: "node1".to_string(),
            from_relay_url: "relay1".to_string(),
            to_public_key: "bob_key".to_string(),
            timestamp: 123,
        };

        // Serialize and deserialize
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: TestP2PMessage = serde_json::from_str(&json).unwrap();

        // Extract the user_id and verify it matches recomputation
        if let TestP2PMessage::FriendRequest {
            from_public_key,
            from_user_id,
            ..
        } = deserialized
        {
            let recomputed_id = TestUuid::from_public_key(&from_public_key);
            assert_eq!(
                from_user_id, recomputed_id,
                "User ID should match after round-trip"
            );
        } else {
            panic!("Expected FriendRequest");
        }
    }

    // ===== Message Flow Tests =====

    #[test]
    fn test_full_friend_request_flow() {
        let alice_pub_key = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string();
        let bob_pub_key = "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string();

        // Step 1: Alice sends friend request to Bob
        let friend_request = TestP2PMessage::FriendRequest {
            from_public_key: alice_pub_key.clone(),
            from_user_id: TestUuid::from_public_key(&alice_pub_key),
            from_display_name: "Alice".to_string(),
            from_node_id: "alice_node".to_string(),
            from_relay_url: "alice_relay".to_string(),
            to_public_key: bob_pub_key.clone(),
            timestamp: 1000,
        };

        // Serialize for network transmission
        let request_json = serde_json::to_string(&friend_request).unwrap();
        println!("FriendRequest size: {} bytes", request_json.len());

        // Step 2: Bob receives and processes
        let received_request: TestP2PMessage = serde_json::from_str(&request_json).unwrap();
        let mut should_process_request = false;
        if let TestP2PMessage::FriendRequest {
            to_public_key,
            from_public_key,
            ..
        } = &received_request
        {
            // Bob checks if intended for him
            if *to_public_key == bob_pub_key {
                // Bob checks it's not from himself
                if *from_public_key != bob_pub_key {
                    should_process_request = true;
                }
            }
        }
        assert!(
            should_process_request,
            "Bob should process the friend request"
        );

        // Step 3: Bob accepts and sends FriendAccepted
        let friend_accepted = TestP2PMessage::FriendAccepted {
            from_user_id: TestUuid::from_public_key(&bob_pub_key),
            from_public_key: bob_pub_key.clone(),
            from_display_name: "Bob".to_string(),
            from_node_id: "bob_node".to_string(),
            from_relay_url: "bob_relay".to_string(),
            to_public_key: alice_pub_key.clone(), // Send back to Alice
        };

        let accepted_json = serde_json::to_string(&friend_accepted).unwrap();
        println!("FriendAccepted size: {} bytes", accepted_json.len());

        // Step 4: Alice receives and processes
        let received_accepted: TestP2PMessage = serde_json::from_str(&accepted_json).unwrap();
        let mut should_process_accepted = false;
        if let TestP2PMessage::FriendAccepted {
            to_public_key,
            from_public_key,
            ..
        } = &received_accepted
        {
            // Alice checks if intended for her
            if *to_public_key == alice_pub_key {
                // Alice checks it's not from herself
                if *from_public_key != alice_pub_key {
                    should_process_accepted = true;
                }
            }
        }
        assert!(
            should_process_accepted,
            "Alice should process the friend accepted"
        );
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_empty_fields() {
        let accepted = TestP2PMessage::FriendAccepted {
            from_user_id: TestUuid::new(),
            from_public_key: "".to_string(),
            from_display_name: "".to_string(),
            from_node_id: "".to_string(),
            from_relay_url: "".to_string(),
            to_public_key: "".to_string(),
        };

        // Should still serialize/deserialize without error
        let json = serde_json::to_string(&accepted).expect("Empty fields should serialize");
        let _deserialized: TestP2PMessage =
            serde_json::from_str(&json).expect("Empty fields should deserialize");
    }

    #[test]
    fn test_unicode_display_name() {
        let accepted = TestP2PMessage::FriendAccepted {
            from_user_id: TestUuid::new(),
            from_public_key: "key".to_string(),
            from_display_name: "日本語 🎉 émojis".to_string(),
            from_node_id: "node".to_string(),
            from_relay_url: "relay".to_string(),
            to_public_key: "key2".to_string(),
        };

        let json = serde_json::to_string(&accepted).unwrap();
        let deserialized: TestP2PMessage = serde_json::from_str(&json).unwrap();

        if let TestP2PMessage::FriendAccepted {
            from_display_name, ..
        } = deserialized
        {
            assert_eq!(from_display_name, "日本語 🎉 émojis");
        } else {
            panic!("Expected FriendAccepted");
        }
    }

    #[test]
    fn test_public_key_case_sensitivity() {
        // Public keys are base64 and CASE SENSITIVE
        let key_lower = "xobmiqq7zfd6v10rzrl6rfhqgfuyi52kdmap09bvcqc=";
        let key_upper = "XOBMIQQ7ZFD6V10RZRL6RFHQGFUYI52KDMAP09BVCQC=";
        let key_mixed = "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=";

        let id_lower = TestUuid::from_public_key(key_lower);
        let id_upper = TestUuid::from_public_key(key_upper);
        let id_mixed = TestUuid::from_public_key(key_mixed);

        // All should be different because keys are case-sensitive
        assert_ne!(id_lower, id_upper, "Case should matter for base64 keys");
        assert_ne!(id_lower, id_mixed, "Case should matter for base64 keys");
        assert_ne!(id_upper, id_mixed, "Case should matter for base64 keys");
    }

    // ===== Message Size Tests =====

    #[test]
    fn test_message_sizes() {
        // These should approximately match what we see in the logs
        let friend_request = TestP2PMessage::FriendRequest {
            from_public_key: "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string(),
            from_user_id: TestUuid::from_public_key("xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc="),
            from_display_name: "Alice".to_string(),
            from_node_id: "a0540bcbbbcb5b184c33bf36ef9f7d1fe5b0e9a7335688436177e277fa291e55"
                .to_string(),
            from_relay_url: "https://aps1-1.relay.iroh.network./".to_string(),
            to_public_key: "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string(),
            timestamp: 1737405566,
        };

        let friend_accepted = TestP2PMessage::FriendAccepted {
            from_user_id: TestUuid::from_public_key("kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs="),
            from_public_key: "kQgclgotvTIvtTDkdwmvI/YSdGppOmSNvoI3HtBAABs=".to_string(),
            from_display_name: "Bob".to_string(),
            from_node_id: "67d2e9e195fa9f95e16ea57edaa87493cd125f3ce595c75d058c2616a08d2153"
                .to_string(),
            from_relay_url: "https://aps1-1.relay.iroh.network./".to_string(),
            to_public_key: "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string(),
        };

        let heartbeat = TestP2PMessage::Heartbeat {
            node_id: "67d2e9e195fa9f95e16ea57edaa87493cd125f3ce595c75d058c2616a08d2153".to_string(),
            timestamp: 1737405566,
        };

        let request_size = serde_json::to_string(&friend_request).unwrap().len();
        let accepted_size = serde_json::to_string(&friend_accepted).unwrap().len();
        let heartbeat_size = serde_json::to_string(&heartbeat).unwrap().len();

        println!("FriendRequest size: {} bytes", request_size);
        println!("FriendAccepted size: {} bytes", accepted_size);
        println!("Heartbeat size: {} bytes", heartbeat_size);

        // From logs: FriendRequest ~389 bytes, FriendAccepted ~365 bytes, Heartbeat ~115 bytes
        // Our test messages should be in similar ballpark
        assert!(
            request_size > 300 && request_size < 500,
            "FriendRequest size unexpected: {}",
            request_size
        );
        assert!(
            accepted_size > 300 && accepted_size < 500,
            "FriendAccepted size unexpected: {}",
            accepted_size
        );
        assert!(
            heartbeat_size > 50 && heartbeat_size < 200,
            "Heartbeat size unexpected: {}",
            heartbeat_size
        );
    }
}

// ===== Real invite format tests (exercise the actual parser) =====
mod invite_v3 {
    use app::iroh_commands::parse_invite_code;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use base64::Engine;

    /// Build a v3 invite: [32 ed25519][32 x25519][32 node][1 name_len][name]
    fn build_v3(name: Option<&str>) -> (String, String, String, String) {
        let pubkey = [1u8; 32];
        let enc = [2u8; 32];
        let node = [3u8; 32];
        let mut payload = Vec::new();
        payload.extend_from_slice(&pubkey);
        payload.extend_from_slice(&enc);
        payload.extend_from_slice(&node);
        if let Some(n) = name {
            payload.push(n.len() as u8);
            payload.extend_from_slice(n.as_bytes());
        }
        (
            format!("cipher://v3/{}", URL_SAFE_NO_PAD.encode(&payload)),
            STANDARD.encode(pubkey),
            STANDARD.encode(enc),
            hex::encode(node),
        )
    }

    #[test]
    fn test_parse_v3_invite_with_name() {
        let (invite, pubkey, enc_key, node_id) = build_v3(Some("Alice"));
        let parsed = parse_invite_code(invite).expect("v3 invite should parse");
        assert_eq!(parsed.public_key, pubkey);
        assert_eq!(
            parsed.encryption_public_key.as_deref(),
            Some(enc_key.as_str())
        );
        assert_eq!(parsed.node_id, node_id);
        assert_eq!(parsed.display_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn test_parse_v3_invite_without_name() {
        let (invite, pubkey, enc_key, node_id) = build_v3(None);
        let parsed = parse_invite_code(invite).expect("v3 invite should parse");
        assert_eq!(parsed.public_key, pubkey);
        assert_eq!(
            parsed.encryption_public_key.as_deref(),
            Some(enc_key.as_str())
        );
        assert_eq!(parsed.node_id, node_id);
        assert_eq!(parsed.display_name, None);
    }

    #[test]
    fn test_old_invite_formats_are_rejected() {
        // Old formats lack the encryption key, so friend requests would have
        // to travel in plaintext - the parser must refuse them
        for old in [
            "cipher://i/AAAA",
            "cipher://f/AAAA",
            "cipher://add-friend?key=x&node=y",
        ] {
            let err = parse_invite_code(old.to_string())
                .expect_err("old invite formats must be rejected");
            assert!(
                err.contains("older version"),
                "error should tell the user to get a new invite, got: {err}"
            );
        }
    }

    #[test]
    fn test_truncated_v3_invite_is_rejected() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let short = format!("cipher://v3/{}", URL_SAFE_NO_PAD.encode([0u8; 64]));
        assert!(parse_invite_code(short).is_err());
    }

    #[test]
    fn test_garbage_is_rejected() {
        assert!(parse_invite_code("https://example.com".to_string()).is_err());
        assert!(parse_invite_code("cipher://v3/!!!not-base64!!!".to_string()).is_err());
    }
}
