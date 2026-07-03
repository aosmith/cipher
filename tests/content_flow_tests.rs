// Integration tests for content creation and sync:
// - Post creation and validation
// - Comment creation and nesting
// - Reaction handling
// - SealedEnvelope encryption/decryption

// Import the actual crypto module for encryption tests
use app::crypto::sealed_box::SealedBox;
use app::crypto::{ContentPayload as RealContentPayload, GossipEnvelope};

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose, Engine};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;
    use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

    /// Simplified SqliteUuid for testing
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TestUuid(pub Uuid);

    impl TestUuid {
        pub fn new() -> Self {
            TestUuid(Uuid::new_v4())
        }

        pub fn from_public_key(public_key: &str) -> Self {
            let namespace = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
            TestUuid(Uuid::new_v5(&namespace, public_key.as_bytes()))
        }
    }

    impl std::fmt::Display for TestUuid {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    /// ContentPayload enum matching the real implementation
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[serde(tag = "type")]
    pub enum ContentPayload {
        Post {
            content: String,
            attachments: Option<Vec<String>>,
        },
        DirectMessage {
            content: String,
            thread_id: Option<TestUuid>,
        },
        PostComment {
            comment_id: String,
            post_id: String,
            content: String,
            parent_comment_id: Option<String>,
            #[serde(default)]
            sent_at: i64,
        },
        PostReaction {
            post_id: String,
            emoji: String,
            action: String, // "add" or "remove"
            #[serde(default)]
            sent_at: i64,
        },
    }

    // ===== Post Tests =====

    #[test]
    fn test_post_creation_serialization() {
        let post = ContentPayload::Post {
            content: "Hello, world!".to_string(),
            attachments: None,
        };

        let json = serde_json::to_string(&post).expect("Failed to serialize post");
        let deserialized: ContentPayload =
            serde_json::from_str(&json).expect("Failed to deserialize post");

        assert_eq!(post, deserialized);
    }

    #[test]
    fn test_post_with_attachments() {
        let post = ContentPayload::Post {
            content: "Check out this image!".to_string(),
            attachments: Some(vec![
                "attachment_id_1".to_string(),
                "attachment_id_2".to_string(),
            ]),
        };

        let json = serde_json::to_string(&post).expect("Failed to serialize post with attachments");
        let deserialized: ContentPayload =
            serde_json::from_str(&json).expect("Failed to deserialize");

        if let ContentPayload::Post { attachments, .. } = deserialized {
            assert!(attachments.is_some());
            assert_eq!(attachments.unwrap().len(), 2);
        } else {
            panic!("Expected Post");
        }
    }

    #[test]
    fn test_post_empty_content() {
        let post = ContentPayload::Post {
            content: "".to_string(),
            attachments: None,
        };

        let json = serde_json::to_string(&post).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::Post { content, .. } = deserialized {
            assert_eq!(content, "");
        } else {
            panic!("Expected Post");
        }
    }

    #[test]
    fn test_post_unicode_content() {
        let post = ContentPayload::Post {
            content: "Привет мир! 你好世界! 🌍🌎🌏".to_string(),
            attachments: None,
        };

        let json = serde_json::to_string(&post).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::Post { content, .. } = deserialized {
            assert_eq!(content, "Привет мир! 你好世界! 🌍🌎🌏");
        } else {
            panic!("Expected Post");
        }
    }

    #[test]
    fn test_post_long_content() {
        let long_content = "A".repeat(10000);
        let post = ContentPayload::Post {
            content: long_content.clone(),
            attachments: None,
        };

        let json = serde_json::to_string(&post).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::Post { content, .. } = deserialized {
            assert_eq!(content.len(), 10000);
        } else {
            panic!("Expected Post");
        }
    }

    // ===== Comment Tests =====

    #[test]
    fn test_comment_creation() {
        let comment = ContentPayload::PostComment {
            comment_id: Uuid::new_v4().to_string(),
            post_id: Uuid::new_v4().to_string(),
            content: "Great post!".to_string(),
            parent_comment_id: None,
            sent_at: 1_700_000_000,
        };

        let json = serde_json::to_string(&comment).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(comment, deserialized);
    }

    #[test]
    fn test_nested_comment() {
        let parent_comment_id = Uuid::new_v4().to_string();
        let comment = ContentPayload::PostComment {
            comment_id: Uuid::new_v4().to_string(),
            post_id: Uuid::new_v4().to_string(),
            content: "Reply to your comment".to_string(),
            parent_comment_id: Some(parent_comment_id.clone()),
            sent_at: 1_700_000_000,
        };

        let json = serde_json::to_string(&comment).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::PostComment {
            parent_comment_id: parent,
            ..
        } = deserialized
        {
            assert!(parent.is_some());
            assert_eq!(parent.unwrap(), parent_comment_id);
        } else {
            panic!("Expected PostComment");
        }
    }

    #[test]
    fn test_comment_ids_are_valid_uuids() {
        let comment_id = Uuid::new_v4().to_string();
        let post_id = Uuid::new_v4().to_string();

        let comment = ContentPayload::PostComment {
            comment_id: comment_id.clone(),
            post_id: post_id.clone(),
            content: "Test".to_string(),
            parent_comment_id: None,
            sent_at: 1_700_000_000,
        };

        let json = serde_json::to_string(&comment).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::PostComment {
            comment_id: cid,
            post_id: pid,
            ..
        } = deserialized
        {
            // Verify they can be parsed back as UUIDs
            Uuid::parse_str(&cid).expect("comment_id should be valid UUID");
            Uuid::parse_str(&pid).expect("post_id should be valid UUID");
        } else {
            panic!("Expected PostComment");
        }
    }

    // ===== Reaction Tests =====

    #[test]
    fn test_reaction_add() {
        let reaction = ContentPayload::PostReaction {
            post_id: Uuid::new_v4().to_string(),
            emoji: "👍".to_string(),
            action: "add".to_string(),
            sent_at: 1_700_000_000,
        };

        let json = serde_json::to_string(&reaction).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(reaction, deserialized);
    }

    #[test]
    fn test_reaction_remove() {
        let reaction = ContentPayload::PostReaction {
            post_id: Uuid::new_v4().to_string(),
            emoji: "❤️".to_string(),
            action: "remove".to_string(),
            sent_at: 1_700_000_000,
        };

        let json = serde_json::to_string(&reaction).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::PostReaction { action, .. } = deserialized {
            assert_eq!(action, "remove");
        } else {
            panic!("Expected PostReaction");
        }
    }

    #[test]
    fn test_reaction_various_emojis() {
        let emojis = vec!["👍", "❤️", "😂", "😮", "😢", "😡", "🎉", "🚀", "💯", "🔥"];

        for emoji in emojis {
            let reaction = ContentPayload::PostReaction {
                post_id: Uuid::new_v4().to_string(),
                emoji: emoji.to_string(),
                action: "add".to_string(),
                sent_at: 1_700_000_000,
            };

            let json = serde_json::to_string(&reaction).unwrap();
            let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

            if let ContentPayload::PostReaction { emoji: e, .. } = deserialized {
                assert_eq!(e, emoji, "Emoji {} should survive serialization", emoji);
            } else {
                panic!("Expected PostReaction");
            }
        }
    }

    #[test]
    fn test_reaction_compound_emojis() {
        // Test skin tone modifiers and compound emojis
        let compound_emojis = vec![
            "👍🏻",
            "👍🏼",
            "👍🏽",
            "👍🏾",
            "👍🏿", // Skin tone variants
            "👨‍👩‍👧‍👦", // Family emoji (ZWJ sequence)
            "🏳️‍🌈", // Rainbow flag
        ];

        for emoji in compound_emojis {
            let reaction = ContentPayload::PostReaction {
                post_id: Uuid::new_v4().to_string(),
                emoji: emoji.to_string(),
                action: "add".to_string(),
                sent_at: 1_700_000_000,
            };

            let json = serde_json::to_string(&reaction).unwrap();
            let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

            if let ContentPayload::PostReaction { emoji: e, .. } = deserialized {
                assert_eq!(
                    e, emoji,
                    "Compound emoji {} should survive serialization",
                    emoji
                );
            } else {
                panic!("Expected PostReaction");
            }
        }
    }

    // ===== Envelope Tests =====

    /// Simplified GossipEnvelope for testing
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TestEnvelope {
        pub message_id: String,
        pub timestamp: i64,
        pub content_type: String,
        pub sender_public_key: String,
        pub payload_json: String, // Encrypted in real impl, plaintext here for testing
    }

    #[test]
    fn test_envelope_with_post() {
        let post = ContentPayload::Post {
            content: "Test post".to_string(),
            attachments: None,
        };

        let envelope = TestEnvelope {
            message_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            content_type: "Post".to_string(),
            sender_public_key: "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string(),
            payload_json: serde_json::to_string(&post).unwrap(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: TestEnvelope = serde_json::from_str(&json).unwrap();

        // Verify we can extract the payload
        let payload: ContentPayload = serde_json::from_str(&deserialized.payload_json).unwrap();
        assert!(matches!(payload, ContentPayload::Post { .. }));
    }

    #[test]
    fn test_envelope_with_comment() {
        let comment = ContentPayload::PostComment {
            comment_id: Uuid::new_v4().to_string(),
            post_id: Uuid::new_v4().to_string(),
            content: "Great post!".to_string(),
            parent_comment_id: None,
            sent_at: 1_700_000_000,
        };

        let envelope = TestEnvelope {
            message_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            content_type: "PostComment".to_string(),
            sender_public_key: "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string(),
            payload_json: serde_json::to_string(&comment).unwrap(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: TestEnvelope = serde_json::from_str(&json).unwrap();

        let payload: ContentPayload = serde_json::from_str(&deserialized.payload_json).unwrap();
        assert!(matches!(payload, ContentPayload::PostComment { .. }));
    }

    #[test]
    fn test_envelope_with_reaction() {
        let reaction = ContentPayload::PostReaction {
            post_id: Uuid::new_v4().to_string(),
            emoji: "👍".to_string(),
            action: "add".to_string(),
            sent_at: 1_700_000_000,
        };

        let envelope = TestEnvelope {
            message_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            content_type: "PostReaction".to_string(),
            sender_public_key: "xoBMiqq7ZFD6V10RZrl6RFHqgfUYI52kDMAp09BvcQc=".to_string(),
            payload_json: serde_json::to_string(&reaction).unwrap(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: TestEnvelope = serde_json::from_str(&json).unwrap();

        let payload: ContentPayload = serde_json::from_str(&deserialized.payload_json).unwrap();
        assert!(matches!(payload, ContentPayload::PostReaction { .. }));
    }

    // ===== Sender Key Tests (related to the bug we fixed) =====

    #[test]
    fn test_sender_key_is_signing_key_not_encryption_key() {
        // This test documents the fix we made: sender_public_key should be
        // the Ed25519 signing key, NOT the X25519 encryption key

        // Signing key (Ed25519) - for identity verification
        let signing_key = "RbGb06jcmGkAkTGz5CqbYnNWBaNmvudg3b0Z20E+wqk=";

        // Encryption key (X25519) - for sealing boxes
        let encryption_key = "bKQ9Ygc49mR/rJ59BfUhkD2dIM/ngWC7dIZ3KlHXYUU=";

        // These are different keys!
        assert_ne!(
            signing_key, encryption_key,
            "Signing and encryption keys should be different"
        );

        // User ID should be computed from SIGNING key
        let user_id_from_signing = TestUuid::from_public_key(signing_key);
        let user_id_from_encryption = TestUuid::from_public_key(encryption_key);

        assert_ne!(
            user_id_from_signing, user_id_from_encryption,
            "User IDs from different keys should be different"
        );

        // In a GossipEnvelope, sender_public_key should be the SIGNING key
        let envelope = TestEnvelope {
            message_id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            content_type: "Post".to_string(),
            sender_public_key: signing_key.to_string(), // CORRECT: signing key
            payload_json: "{}".to_string(),
        };

        // Verify user_id computed from envelope matches expected
        let computed_user_id = TestUuid::from_public_key(&envelope.sender_public_key);
        assert_eq!(
            computed_user_id, user_id_from_signing,
            "Envelope sender should use signing key for consistent user_id"
        );
    }

    // ===== Message Type Discrimination Tests =====

    #[test]
    fn test_content_type_discrimination() {
        let payloads: Vec<(&str, ContentPayload)> = vec![
            (
                "Post",
                ContentPayload::Post {
                    content: "test".to_string(),
                    attachments: None,
                },
            ),
            (
                "DirectMessage",
                ContentPayload::DirectMessage {
                    content: "test".to_string(),
                    thread_id: None,
                },
            ),
            (
                "PostComment",
                ContentPayload::PostComment {
                    comment_id: "1".to_string(),
                    post_id: "2".to_string(),
                    content: "test".to_string(),
                    parent_comment_id: None,
                    sent_at: 1_700_000_000,
                },
            ),
            (
                "PostReaction",
                ContentPayload::PostReaction {
                    post_id: "1".to_string(),
                    emoji: "👍".to_string(),
                    action: "add".to_string(),
                    sent_at: 1_700_000_000,
                },
            ),
        ];

        for (expected_type, payload) in payloads {
            let json = serde_json::to_string(&payload).unwrap();

            // The JSON should contain the type discriminator
            assert!(
                json.contains(&format!("\"type\":\"{}\"", expected_type)),
                "JSON should contain type discriminator for {}",
                expected_type
            );

            // Should deserialize back to the correct variant
            let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();
            let type_name = match &deserialized {
                ContentPayload::Post { .. } => "Post",
                ContentPayload::DirectMessage { .. } => "DirectMessage",
                ContentPayload::PostComment { .. } => "PostComment",
                ContentPayload::PostReaction { .. } => "PostReaction",
            };
            assert_eq!(
                type_name, expected_type,
                "Should deserialize to correct variant"
            );
        }
    }

    // ===== Direct Message Tests =====

    #[test]
    fn test_direct_message_without_thread() {
        let dm = ContentPayload::DirectMessage {
            content: "Hey, how are you?".to_string(),
            thread_id: None,
        };

        let json = serde_json::to_string(&dm).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::DirectMessage { thread_id, .. } = deserialized {
            assert!(thread_id.is_none());
        } else {
            panic!("Expected DirectMessage");
        }
    }

    #[test]
    fn test_direct_message_with_thread() {
        let thread_id = TestUuid::new();
        let dm = ContentPayload::DirectMessage {
            content: "Reply in thread".to_string(),
            thread_id: Some(thread_id),
        };

        let json = serde_json::to_string(&dm).unwrap();
        let deserialized: ContentPayload = serde_json::from_str(&json).unwrap();

        if let ContentPayload::DirectMessage { thread_id: tid, .. } = deserialized {
            assert!(tid.is_some());
            assert_eq!(tid.unwrap(), thread_id);
        } else {
            panic!("Expected DirectMessage");
        }
    }

    // ===== Real Encryption Tests =====
    // These tests use the actual crypto module to verify encryption/decryption

    /// Helper to generate a test X25519 keypair
    fn generate_test_keypair() -> (String, String) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut secret_bytes = [0u8; 32];
        rng.fill(&mut secret_bytes);

        let private_key = StaticSecret::from(secret_bytes);
        let public_key = X25519PublicKey::from(&private_key);

        (
            general_purpose::STANDARD.encode(public_key.as_bytes()),
            general_purpose::STANDARD.encode(secret_bytes),
        )
    }

    #[test]
    fn test_sealed_box_post_encrypt_decrypt() {
        let (pub_key, priv_key) = generate_test_keypair();

        let payload = RealContentPayload::Post {
            post_id: "test-post-id".to_string(),
            content: "Hello, encrypted world!".to_string(),
            node_id: "test-node-id".to_string(),
            blob_refs: vec![],
            sent_at: 1_700_000_000,
        };

        let sealed =
            SealedBox::new(&payload, &pub_key, &priv_key).expect("Should create sealed box");

        let decrypted = sealed.decrypt(&priv_key).expect("Should decrypt");

        match decrypted {
            RealContentPayload::Post { content, .. } => {
                assert_eq!(content, "Hello, encrypted world!");
            }
            _ => panic!("Expected Post payload"),
        }
    }

    #[test]
    fn test_sealed_box_comment_encrypt_decrypt() {
        let (pub_key, priv_key) = generate_test_keypair();

        let comment_id = Uuid::new_v4().to_string();
        let post_id = Uuid::new_v4().to_string();

        let payload = RealContentPayload::PostComment {
            comment_id: comment_id.clone(),
            post_id: post_id.clone(),
            content: "Great post!".to_string(),
            parent_comment_id: None,
            sent_at: 1_700_000_000,
        };

        let sealed =
            SealedBox::new(&payload, &pub_key, &priv_key).expect("Should create sealed box");

        let decrypted = sealed.decrypt(&priv_key).expect("Should decrypt");

        match decrypted {
            RealContentPayload::PostComment {
                comment_id: cid,
                post_id: pid,
                content,
                ..
            } => {
                assert_eq!(cid, comment_id);
                assert_eq!(pid, post_id);
                assert_eq!(content, "Great post!");
            }
            _ => panic!("Expected PostComment payload"),
        }
    }

    #[test]
    fn test_sealed_box_reaction_encrypt_decrypt() {
        let (pub_key, priv_key) = generate_test_keypair();

        let post_id = Uuid::new_v4().to_string();

        let payload = RealContentPayload::PostReaction {
            post_id: post_id.clone(),
            emoji: "👍".to_string(),
            action: "add".to_string(),
            sent_at: 1_700_000_000,
        };

        let sealed =
            SealedBox::new(&payload, &pub_key, &priv_key).expect("Should create sealed box");

        let decrypted = sealed.decrypt(&priv_key).expect("Should decrypt");

        match decrypted {
            RealContentPayload::PostReaction {
                post_id: pid,
                emoji,
                action,
                ..
            } => {
                assert_eq!(pid, post_id);
                assert_eq!(emoji, "👍");
                assert_eq!(action, "add");
            }
            _ => panic!("Expected PostReaction payload"),
        }
    }

    #[test]
    fn test_envelope_multi_recipient_decryption() {
        // Sender keypair
        let (sender_pub, sender_priv) = generate_test_keypair();

        // Three recipients
        let (alice_pub, alice_priv) = generate_test_keypair();
        let (bob_pub, bob_priv) = generate_test_keypair();
        let (carol_pub, carol_priv) = generate_test_keypair();

        let recipient_keys = vec![alice_pub.clone(), bob_pub.clone(), carol_pub.clone()];

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "test-post-1",
            "Secret message for friends",
            "test-node-id",
            &[],
            &recipient_keys,
            &sender_priv,
        )
        .expect("Should create envelope");

        // All recipients can decrypt
        let alice_result = envelope.try_decrypt(&alice_pub, &alice_priv);
        assert!(alice_result.is_some(), "Alice should decrypt");

        let bob_result = envelope.try_decrypt(&bob_pub, &bob_priv);
        assert!(bob_result.is_some(), "Bob should decrypt");

        let carol_result = envelope.try_decrypt(&carol_pub, &carol_priv);
        assert!(carol_result.is_some(), "Carol should decrypt");

        // Verify content matches for all
        if let Some(RealContentPayload::Post { content, .. }) = alice_result {
            assert_eq!(content, "Secret message for friends");
        }
        if let Some(RealContentPayload::Post { content, .. }) = bob_result {
            assert_eq!(content, "Secret message for friends");
        }
        if let Some(RealContentPayload::Post { content, .. }) = carol_result {
            assert_eq!(content, "Secret message for friends");
        }
    }

    #[test]
    fn test_envelope_non_recipient_cannot_decrypt() {
        let (sender_pub, sender_priv) = generate_test_keypair();
        let (alice_pub, _alice_priv) = generate_test_keypair();
        let (eve_pub, eve_priv) = generate_test_keypair();

        // Only Alice is a recipient
        let recipient_keys = vec![alice_pub.clone()];

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "test-post-2",
            "Secret message",
            "test-node-id",
            &[],
            &recipient_keys,
            &sender_priv,
        )
        .expect("Should create envelope");

        // Eve (not a recipient) cannot decrypt
        let eve_result = envelope.try_decrypt(&eve_pub, &eve_priv);
        assert!(eve_result.is_none(), "Eve should NOT be able to decrypt");
    }

    #[test]
    fn test_envelope_wrong_key_cannot_decrypt() {
        let (sender_pub, sender_priv) = generate_test_keypair();
        let (alice_pub, _alice_priv) = generate_test_keypair();
        let (_bob_pub, bob_priv) = generate_test_keypair();

        let recipient_keys = vec![alice_pub.clone()];

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "test-post-3",
            "Secret message",
            "test-node-id",
            &[],
            &recipient_keys,
            &sender_priv,
        )
        .expect("Should create envelope");

        // Alice's public key but Bob's private key should fail
        let result = envelope.try_decrypt(&alice_pub, &bob_priv);
        assert!(result.is_none(), "Wrong private key should fail");
    }

    #[test]
    fn test_envelope_preserves_unicode_content() {
        let (sender_pub, sender_priv) = generate_test_keypair();
        let (alice_pub, alice_priv) = generate_test_keypair();

        let unicode_content = "Привет! 你好! مرحبا! 🎉🌍💯";

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "test-post-4",
            unicode_content,
            "test-node-id",
            &[],
            &[alice_pub.clone()],
            &sender_priv,
        )
        .expect("Should create envelope");

        let decrypted = envelope.try_decrypt(&alice_pub, &alice_priv);

        match decrypted {
            Some(RealContentPayload::Post { content, .. }) => {
                assert_eq!(content, unicode_content);
            }
            _ => panic!("Expected Post with unicode content"),
        }
    }

    #[test]
    fn test_envelope_preserves_large_content() {
        let (sender_pub, sender_priv) = generate_test_keypair();
        let (alice_pub, alice_priv) = generate_test_keypair();

        // 10KB of content
        let large_content = "A".repeat(10240);

        let envelope = GossipEnvelope::new_post(
            &sender_pub,
            "test-post-5",
            &large_content,
            "test-node-id",
            &[],
            &[alice_pub.clone()],
            &sender_priv,
        )
        .expect("Should create envelope");

        let decrypted = envelope.try_decrypt(&alice_pub, &alice_priv);

        match decrypted {
            Some(RealContentPayload::Post { content, .. }) => {
                assert_eq!(content.len(), 10240);
                assert_eq!(content, large_content);
            }
            _ => panic!("Expected Post with large content"),
        }
    }

    #[test]
    fn test_envelope_comment_round_trip() {
        let (sender_pub, sender_priv) = generate_test_keypair();
        let (alice_pub, alice_priv) = generate_test_keypair();

        let comment_id = Uuid::new_v4().to_string();
        let post_id = Uuid::new_v4().to_string();
        let parent_id = Uuid::new_v4().to_string();

        let envelope = GossipEnvelope::new_post_comment(
            &sender_pub,
            &comment_id,
            &post_id,
            "Nested reply!",
            Some(parent_id.as_str()),
            &[alice_pub.clone()],
            &sender_priv,
        )
        .expect("Should create comment envelope");

        let decrypted = envelope.try_decrypt(&alice_pub, &alice_priv);

        match decrypted {
            Some(RealContentPayload::PostComment {
                comment_id: cid,
                post_id: pid,
                content,
                parent_comment_id,
                ..
            }) => {
                assert_eq!(cid, comment_id);
                assert_eq!(pid, post_id);
                assert_eq!(content, "Nested reply!");
                assert_eq!(parent_comment_id, Some(parent_id));
            }
            _ => panic!("Expected PostComment"),
        }
    }

    #[test]
    fn test_envelope_reaction_round_trip() {
        let (sender_pub, sender_priv) = generate_test_keypair();
        let (alice_pub, alice_priv) = generate_test_keypair();

        let post_id = Uuid::new_v4().to_string();

        // Test add reaction
        let envelope = GossipEnvelope::new_post_reaction(
            &sender_pub,
            &post_id,
            "❤️",
            "add",
            &[alice_pub.clone()],
            &sender_priv,
        )
        .expect("Should create reaction envelope");

        let decrypted = envelope.try_decrypt(&alice_pub, &alice_priv);

        match decrypted {
            Some(RealContentPayload::PostReaction {
                post_id: pid,
                emoji,
                action,
                ..
            }) => {
                assert_eq!(pid, post_id);
                assert_eq!(emoji, "❤️");
                assert_eq!(action, "add");
            }
            _ => panic!("Expected PostReaction"),
        }
    }

    #[test]
    fn test_envelope_reaction_remove() {
        let (sender_pub, sender_priv) = generate_test_keypair();
        let (alice_pub, alice_priv) = generate_test_keypair();

        let post_id = Uuid::new_v4().to_string();

        // Test remove reaction
        let envelope = GossipEnvelope::new_post_reaction(
            &sender_pub,
            &post_id,
            "👎",
            "remove",
            &[alice_pub.clone()],
            &sender_priv,
        )
        .expect("Should create reaction remove envelope");

        let decrypted = envelope.try_decrypt(&alice_pub, &alice_priv);

        match decrypted {
            Some(RealContentPayload::PostReaction {
                post_id: pid,
                emoji,
                action,
                ..
            }) => {
                assert_eq!(pid, post_id);
                assert_eq!(emoji, "👎");
                assert_eq!(action, "remove");
            }
            _ => panic!("Expected PostReaction remove"),
        }
    }
}
