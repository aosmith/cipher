use app::Database;
use std::path::{Path, PathBuf};
use std::{env, fs};

struct UserConfig {
    display_name: &'static str,
    base_dir: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = env::var("CIPHER_FIXTURE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/cipher_fixture"));
    let alice = UserConfig {
        display_name: "alice",
        base_dir: base_dir.join("alice"),
    };
    let bob = UserConfig {
        display_name: "bob",
        base_dir: base_dir.join("bob"),
    };

    seed_pair(&alice, &bob)?;
    println!("Seeded fixtures under {}", base_dir.display());
    println!("Alice HOME={}", alice.base_dir.display());
    println!("Bob HOME={}", bob.base_dir.display());
    Ok(())
}

fn seed_pair(alice: &UserConfig, bob: &UserConfig) -> Result<(), Box<dyn std::error::Error>> {
    prepare_home(&alice.base_dir)?;
    prepare_home(&bob.base_dir)?;

    let alice_db = open_db(&alice.base_dir)?;
    let bob_db = open_db(&bob.base_dir)?;

    // Create users with recovery phrases
    let (alice_user, alice_recovery_phrase) = alice_db.create_user_first_launch(
        alice.display_name.to_string(),
        Database::generate_device_id(),
    )?;

    let (bob_user, bob_recovery_phrase) = bob_db
        .create_user_first_launch(bob.display_name.to_string(), Database::generate_device_id())?;

    println!("\nRecovery Phrases:");
    println!("Alice: {}", alice_recovery_phrase);
    println!("Bob: {}", bob_recovery_phrase);

    // Sync peers by public keys
    let bob_in_alice = alice_db.sync_peer_user(
        bob.display_name,
        bob_user.public_key.as_ref().expect("bob public key"),
        bob_user
            .encryption_public_key
            .as_ref()
            .expect("bob enc key"),
    )?;

    let alice_in_bob = bob_db.sync_peer_user(
        alice.display_name,
        alice_user.public_key.as_ref().expect("alice public key"),
        alice_user
            .encryption_public_key
            .as_ref()
            .expect("alice enc key"),
    )?;

    // Add friends both directions
    alice_db.add_friend(alice_user.id, bob_in_alice.id)?;
    bob_db.add_friend(bob_user.id, alice_in_bob.id)?;

    // Accept friendships
    alice_db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![alice_user.id, bob_in_alice.id],
    )?;
    bob_db.conn.lock().unwrap().execute(
        "UPDATE p2p_connections SET status = 'accepted' WHERE user_id = ?1 AND friend_user_id = ?2",
        rusqlite::params![bob_user.id, alice_in_bob.id],
    )?;

    // Exchange messages
    let message_content = format!("Hello {}!", bob.display_name);
    alice_db.send_encrypted_message(alice_user.id, bob_in_alice.id, &message_content, None)?;

    let reply_content = format!("Hi {}, nice to meet you!", alice.display_name);
    bob_db.send_encrypted_message(bob_user.id, alice_in_bob.id, &reply_content, None)?;

    let alice_messages = alice_db.get_messages_for_user(alice_user.id)?;
    let bob_messages = bob_db.get_messages_for_user(bob_user.id)?;

    println!("Alice messages: {}", alice_messages.len());
    println!("Bob messages: {}", bob_messages.len());

    Ok(())
}

fn prepare_home(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if home.exists() {
        fs::remove_dir_all(home)?;
    }
    fs::create_dir_all(home)?;
    fs::create_dir_all(home.join("Library/Application Support"))?;
    fs::create_dir_all(home.join("Library/Preferences"))?;
    Ok(())
}

fn open_db(home: &Path) -> Result<Database, Box<dyn std::error::Error>> {
    let db_path = home.join("Library/Application Support/com.cipher.social/cipher.db");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let db_path_str = db_path.to_string_lossy().to_string();
    let db = Database::new(&db_path_str)?;
    Ok(db)
}
