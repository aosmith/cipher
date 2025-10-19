use app::{Database, User, Post, Message};
use std::path::Path;
use std::fs;
use uuid::Uuid;

/// Test database wrapper that automatically cleans up on drop
pub struct TestDatabase {
    pub db: Database,
    pub path: String,
}

impl TestDatabase {
    pub fn new(name: &str) -> Self {
        let path = format!("/tmp/cipher_test_{}.db", name);
        let _ = fs::remove_file(&path);
        let db = Database::new(&path).expect("Failed to create test database");
        TestDatabase { db, path }
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl std::ops::Deref for TestDatabase {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

/// Test user factory for creating users with predictable attributes
pub struct TestUserFactory;

impl TestUserFactory {
    pub fn create_user(db: &Database, name: &str) -> User {
        let (user, _recovery_phrase) = db.create_user_first_launch(
            name.to_string(),
            Database::generate_device_id()
        )
        .expect(&format!("Failed to create user {}", name));
        user
    }

    pub fn create_user_with_recovery_phrase(db: &Database, name: &str) -> (User, String) {
        // Creates a user and returns both the user and recovery phrase
        db.create_user_first_launch(
            name.to_string(),
            Database::generate_device_id()
        )
        .expect(&format!("Failed to create user {}", name))
    }

    pub fn sync_user(db: &Database, name: &str) -> User {
        db.sync_peer_user(
            name,
            &format!("{}_public_key", name),
            &format!("{}_enc_key", name)
        ).expect(&format!("Failed to sync user {}", name))
    }

    pub fn sync_real_user(db: &Database, user: &User) -> User {
        let public_key = user.public_key.clone().expect("User should have public key");
        let enc_key = user.encryption_public_key.clone().expect("User should have enc key");

        db.sync_peer_user(
            &user.username,
            &public_key,
            &enc_key
        ).expect(&format!("Failed to sync user {}", user.username))
    }
}

/// Helper for setting up friendships between users
pub struct FriendshipHelper;

impl FriendshipHelper {
    /// Create a bidirectional accepted friendship between two users
    pub fn make_friends(db1: &Database, user1: &User, user2_in_db1: &User) {
        db1.add_friend(user1.id, user2_in_db1.id)
            .expect("Failed to add friend");

        db1.conn.lock().unwrap().execute(
            "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
            rusqlite::params![user1.id, user2_in_db1.id],
        ).expect("Failed to accept friendship");
    }

    /// Create bidirectional friendship across two databases
    pub fn make_friends_bidirectional(
        db1: &Database, user1: &User, user2_in_db1: &User,
        db2: &Database, user2: &User, user1_in_db2: &User
    ) {
        Self::make_friends(db1, user1, user2_in_db1);
        Self::make_friends(db2, user2, user1_in_db2);
    }

    /// Remove friendship between two users
    pub fn remove_friendship(db: &Database, user1_id: Uuid, user2_id: Uuid) {
        db.conn.lock().unwrap().execute(
            "DELETE FROM p2p_connections WHERE user_id = ?1 AND friend_user_id = ?2",
            rusqlite::params![user1_id, user2_id],
        ).expect("Failed to remove friendship");
    }
}

/// Helper for creating and syncing posts
pub struct PostHelper;

impl PostHelper {
    /// Create a post for a user
    pub fn create_post(db: &Database, user: &User, content: &str) -> Post {
        db.create_post(user.id, content, false)
            .expect("Failed to create post")
    }

    /// Create an encrypted post for a user
    pub fn create_encrypted_post(db: &Database, user: &User, content: &str) -> Post {
        db.create_post(user.id, content, true)
            .expect("Failed to create encrypted post")
    }

    /// Sync a post from one database to another
    pub fn sync_post(from_db: &Database, to_db: &Database, post: &Post, user_in_to_db: &User) {
        // Check if post already exists
        let existing = to_db.conn.lock().unwrap().query_row(
            "SELECT id FROM posts WHERE id = ?1",
            rusqlite::params![post.id],
            |_| Ok(())
        );

        if existing.is_err() {
            to_db.conn.lock().unwrap().execute(
                "INSERT INTO posts (id, user_id, content, encrypted, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'))",
                rusqlite::params![post.id, user_in_to_db.id, post.content, post.encrypted],
            ).expect("Failed to sync post");
        }
    }

    /// Sync all posts from one database to another for a specific user
    pub fn sync_all_posts(from_db: &Database, to_db: &Database, user_id: Uuid) {
        let posts = from_db.get_posts(user_id.into()).expect("Failed to get posts");

        for post in posts {
            // Find the user in the target database
            let user = from_db.conn.lock().unwrap().query_row(
                "SELECT username FROM users WHERE id = ?1",
                rusqlite::params![post.user_id],
                |row| row.get::<_, String>(0)
            ).expect("Failed to find user");

            let user_in_to = to_db.find_user_by_username(&user)
                .expect("Failed to find user")
                .expect("User should exist in target database");

            Self::sync_post(from_db, to_db, &post, &user_in_to);
        }
    }
}

/// Helper for message operations
pub struct MessageHelper;

impl MessageHelper {
    /// Send an encrypted message between users
    pub fn send_message(db: &Database, sender: &User, recipient: &User, content: &str) -> Message {
        db.send_encrypted_message(sender.id, recipient.id, content, None)
            .expect("Failed to send message")
    }

    /// Send a disappearing message
    pub fn send_disappearing_message(
        db: &Database,
        sender: &User,
        recipient: &User,
        content: &str,
        ttl_seconds: i64
    ) -> Message {
        db.send_encrypted_message(sender.id, recipient.id, content, Some(ttl_seconds))
            .expect("Failed to send disappearing message")
    }

    /// Mark a conversation as read
    pub fn mark_as_read(db: &Database, user_id: Uuid, other_user_id: Uuid) {
        db.mark_conversation_as_read(user_id, other_user_id)
            .expect("Failed to mark conversation as read");
    }
}

/// Helper for asserting test conditions
pub struct TestAssertions;

impl TestAssertions {
    /// Assert that a user's feed contains exactly the expected posts
    pub fn assert_feed_contains(db: &Database, user: &User, expected_contents: Vec<&str>) {
        let feed = db.get_posts(user.id).expect("Failed to get feed");

        assert_eq!(
            feed.len(),
            expected_contents.len(),
            "Feed should contain {} posts, but has {}",
            expected_contents.len(),
            feed.len()
        );

        for content in expected_contents {
            assert!(
                feed.iter().any(|p| p.content == content),
                "Feed should contain post with content: {}",
                content
            );
        }
    }

    /// Assert that a user's feed does NOT contain certain posts
    pub fn assert_feed_excludes(db: &Database, user: &User, excluded_contents: Vec<&str>) {
        let feed = db.get_posts(user.id).expect("Failed to get feed");

        for content in excluded_contents {
            assert!(
                !feed.iter().any(|p| p.content == content),
                "Feed should NOT contain post with content: {}",
                content
            );
        }
    }

    /// Assert that two users are friends
    pub fn assert_are_friends(db: &Database, user: &User, friend: &User) {
        let friends = db.get_friends(user.id).expect("Failed to get friends");
        assert!(
            friends.iter().any(|f| f.friend_user_id == friend.id),
            "{} and {} should be friends",
            user.username,
            friend.username
        );
    }

    /// Assert that two users are NOT friends
    pub fn assert_not_friends(db: &Database, user: &User, other: &User) {
        let friends = db.get_friends(user.id).expect("Failed to get friends");
        assert!(
            !friends.iter().any(|f| f.friend_user_id == other.id),
            "{} and {} should NOT be friends",
            user.username,
            other.username
        );
    }

    /// Assert message encryption status
    pub fn assert_message_encrypted(message: &Message) {
        assert!(message.encrypted, "Message should be encrypted");
        assert!(!message.content.is_empty(), "Encrypted message should have content");
    }
}

/// Create a test network of users with predefined relationships
pub struct TestNetwork {
    pub databases: Vec<TestDatabase>,
    pub users: Vec<User>,
}

impl TestNetwork {
    /// Create a linear network: A -> B -> C -> D
    pub fn create_linear(names: Vec<&str>) -> Self {
        let mut databases = Vec::new();
        let mut users = Vec::new();

        // Create databases and users
        for name in &names {
            let test_db = TestDatabase::new(name);
            let user = TestUserFactory::create_user(&test_db.db, name);
            users.push(user.clone());
            databases.push(test_db);
        }

        // Sync all users to all databases
        for i in 0..users.len() {
            for j in 0..databases.len() {
                if i != j {
                    TestUserFactory::sync_real_user(&databases[j].db, &users[i]);
                }
            }
        }

        // Create linear friendships (A-B, B-C, C-D)
        for i in 0..names.len() - 1 {
            let user1 = &users[i];
            let user2 = &users[i + 1];
            let db1 = &databases[i].db;
            let db2 = &databases[i + 1].db;

            let user2_in_db1 = db1.find_user_by_username(&names[i + 1])
                .unwrap().unwrap();
            let user1_in_db2 = db2.find_user_by_username(&names[i])
                .unwrap().unwrap();

            FriendshipHelper::make_friends_bidirectional(
                db1, user1, &user2_in_db1,
                db2, user2, &user1_in_db2
            );
        }

        TestNetwork { databases, users }
    }

    /// Create a fully connected network where everyone is friends with everyone
    pub fn create_fully_connected(names: Vec<&str>) -> Self {
        let mut databases = Vec::new();
        let mut users = Vec::new();

        // Create databases and users
        for name in &names {
            let test_db = TestDatabase::new(name);
            let user = TestUserFactory::create_user(&test_db.db, name);
            users.push(user.clone());
            databases.push(test_db);
        }

        // Sync all users to all databases
        for i in 0..users.len() {
            for j in 0..databases.len() {
                if i != j {
                    TestUserFactory::sync_real_user(&databases[j].db, &users[i]);
                }
            }
        }

        // Create friendships between everyone
        for i in 0..names.len() {
            for j in i + 1..names.len() {
                let user1 = &users[i];
                let user2 = &users[j];
                let db1 = &databases[i].db;
                let db2 = &databases[j].db;

                let user2_in_db1 = db1.find_user_by_username(&names[j])
                    .unwrap().unwrap();
                let user1_in_db2 = db2.find_user_by_username(&names[i])
                    .unwrap().unwrap();

                FriendshipHelper::make_friends_bidirectional(
                    db1, user1, &user2_in_db1,
                    db2, user2, &user1_in_db2
                );
            }
        }

        TestNetwork { databases, users }
    }
}