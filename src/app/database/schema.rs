use rusqlite::{Connection, Result as SqliteResult};

/// Generate a random device ID (16 bytes hex = 32 chars)
fn generate_device_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn create_tables(conn: &Connection) -> SqliteResult<()> {
    // Create users table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id BLOB PRIMARY KEY,
            display_name TEXT NOT NULL,
            public_key TEXT UNIQUE,
            private_key TEXT,
            encryption_public_key TEXT,
            encryption_private_key TEXT,
            device_id TEXT,
            bio TEXT,
            profile_picture TEXT,
            profile_signature TEXT,
            recovery_phrase_hash TEXT,
            recovery_phrase_shown BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    // Replay protection for sealed envelopes: persistently tracks processed
    // message_ids so recorded envelopes can't be replayed across restarts
    conn.execute(
        "CREATE TABLE IF NOT EXISTS seen_envelopes (
            message_id TEXT PRIMARY KEY,
            seen_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_seen_envelopes_seen_at ON seen_envelopes(seen_at)",
        [],
    )?;

    // Rotating signed pre-keys for forward secrecy. Senders seal to our CURRENT
    // pre-key instead of our static identity key; we keep the current + the
    // immediately-previous private key (one rotation of overlap for lossy
    // gossip) and delete older ones, so a later identity-key compromise can't
    // decrypt recorded traffic sealed to a deleted pre-key. Pre-keys are random
    // (not derived from the recovery phrase), so a restored device simply mints
    // a fresh one - the old private keys are gone, which is the point.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS signed_prekeys (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id BLOB NOT NULL,
            public_key TEXT NOT NULL,
            private_key TEXT NOT NULL,
            signature TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            is_current INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;

    // Create posts table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS posts (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            content TEXT NOT NULL,
            encrypted BOOLEAN DEFAULT 0,
            pinned BOOLEAN DEFAULT 0,
            shared_post_id BLOB,
            share_comment TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id)
        )",
        [],
    )?;

    // Create p2p_connections table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS p2p_connections (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            friend_user_id BLOB NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            initiated_by BLOB NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            iroh_node_id TEXT,
            friend_relay_url TEXT,
            known_display_name TEXT,
            friend_profile_signature TEXT,
            FOREIGN KEY (user_id) REFERENCES users (id),
            FOREIGN KEY (friend_user_id) REFERENCES users (id),
            FOREIGN KEY (initiated_by) REFERENCES users (id),
            UNIQUE(user_id, friend_user_id)
        )",
        [],
    )?;

    // Create messages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id BLOB PRIMARY KEY,
            sender_id BLOB NOT NULL,
            recipient_id BLOB NOT NULL,
            content TEXT NOT NULL,
            encrypted BOOLEAN DEFAULT 1,
            signature TEXT,
            thread_id BLOB,
            disappear_after_seconds INTEGER,
            disappears_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            edited_at TEXT,
            FOREIGN KEY (sender_id) REFERENCES users (id),
            FOREIGN KEY (recipient_id) REFERENCES users (id),
            FOREIGN KEY (thread_id) REFERENCES messages (id)
        )",
        [],
    )?;

    // Create media_attachments table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS media_attachments (
            id BLOB PRIMARY KEY,
            post_id BLOB,
            file_type TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            data BLOB,
            FOREIGN KEY (post_id) REFERENCES posts (id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create message_reactions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS message_reactions (
            id BLOB PRIMARY KEY,
            message_id BLOB NOT NULL,
            user_id BLOB NOT NULL,
            emoji TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (message_id) REFERENCES messages (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users (id),
            UNIQUE(message_id, user_id, emoji)
        )",
        [],
    )?;

    // Create post_reactions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS post_reactions (
            id BLOB PRIMARY KEY,
            post_id BLOB NOT NULL,
            user_id BLOB NOT NULL,
            emoji TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (post_id) REFERENCES posts (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users (id),
            UNIQUE(post_id, user_id, emoji)
        )",
        [],
    )?;

    // Create post_comments table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS post_comments (
            id BLOB PRIMARY KEY,
            post_id BLOB NOT NULL,
            user_id BLOB NOT NULL,
            content TEXT NOT NULL,
            parent_comment_id BLOB,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (post_id) REFERENCES posts (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users (id),
            FOREIGN KEY (parent_comment_id) REFERENCES post_comments (id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create friend_invites table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS friend_invites (
            id BLOB PRIMARY KEY,
            creator_id BLOB NOT NULL,
            invite_code TEXT UNIQUE NOT NULL,
            public_key TEXT NOT NULL,
            display_name TEXT NOT NULL,
            uses_remaining INTEGER NOT NULL DEFAULT 1,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (creator_id) REFERENCES users (id)
        )",
        [],
    )?;

    // Create recent_contacts table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS recent_contacts (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            contact_user_id BLOB NOT NULL,
            last_interaction TEXT NOT NULL,
            interaction_count INTEGER NOT NULL DEFAULT 1,
            FOREIGN KEY (user_id) REFERENCES users (id),
            FOREIGN KEY (contact_user_id) REFERENCES users (id),
            UNIQUE(user_id, contact_user_id)
        )",
        [],
    )?;

    // Create devices table for multi-device sync
    conn.execute(
        "CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY,
            user_public_key TEXT NOT NULL,
            device_name TEXT,
            last_sync TEXT NOT NULL,
            created_at TEXT NOT NULL,
            iroh_node_id TEXT,
            relay_url TEXT
        )",
        [],
    )?;

    // Create sync_state table for tracking last sync timestamps per device
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_state (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id TEXT NOT NULL,
            table_name TEXT NOT NULL,
            last_sync_timestamp TEXT NOT NULL,
            UNIQUE(device_id, table_name)
        )",
        [],
    )?;

    // Create notifications table for persistent notifications
    conn.execute(
        "CREATE TABLE IF NOT EXISTS notifications (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            notification_type TEXT NOT NULL,
            title TEXT NOT NULL,
            message TEXT NOT NULL,
            data TEXT,
            read BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id)
        )",
        [],
    )?;

    // Create blocked_users table for blocking functionality
    conn.execute(
        "CREATE TABLE IF NOT EXISTS blocked_users (
            id BLOB PRIMARY KEY,
            blocker_id BLOB NOT NULL,
            blocked_id BLOB NOT NULL,
            reason TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (blocker_id) REFERENCES users (id),
            FOREIGN KEY (blocked_id) REFERENCES users (id),
            UNIQUE(blocker_id, blocked_id)
        )",
        [],
    )?;

    // Create muted_users table for muting functionality
    conn.execute(
        "CREATE TABLE IF NOT EXISTS muted_users (
            id BLOB PRIMARY KEY,
            muter_id BLOB NOT NULL,
            muted_id BLOB NOT NULL,
            mute_notifications BOOLEAN DEFAULT 1,
            mute_messages BOOLEAN DEFAULT 1,
            mute_posts BOOLEAN DEFAULT 1,
            expires_at TEXT,
            created_at TEXT NOT NULL,
            FOREIGN KEY (muter_id) REFERENCES users (id),
            FOREIGN KEY (muted_id) REFERENCES users (id),
            UNIQUE(muter_id, muted_id)
        )",
        [],
    )?;

    // Create pending_messages table for offline delivery queue
    conn.execute(
        "CREATE TABLE IF NOT EXISTS pending_messages (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            message_type TEXT NOT NULL,
            content_json TEXT NOT NULL,
            retry_count INTEGER DEFAULT 0,
            max_retries INTEGER DEFAULT 5,
            created_at TEXT NOT NULL,
            last_attempt_at TEXT,
            FOREIGN KEY (user_id) REFERENCES users (id)
        )",
        [],
    )?;

    // Create ratchet_states table for Double Ratchet (Phase 3)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ratchet_states (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            peer_public_key TEXT NOT NULL,
            state_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id),
            UNIQUE(user_id, peer_public_key)
        )",
        [],
    )?;

    // Create message_cache table for relay cache & purging (Phase 4)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS message_cache (
            id BLOB PRIMARY KEY,
            message_id TEXT UNIQUE NOT NULL,
            envelope_json TEXT NOT NULL,
            sender_public_key TEXT NOT NULL,
            received_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        )",
        [],
    )?;

    // Create prekeys table for X3DH pre-keys (Phase 5)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS prekeys (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            prekey_id INTEGER NOT NULL,
            bundle_json TEXT NOT NULL,
            private_key TEXT,
            is_own INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id),
            UNIQUE(user_id, prekey_id, is_own)
        )",
        [],
    )?;

    // Create communities table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS communities (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            avatar TEXT,
            creator_id BLOB NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (creator_id) REFERENCES users (id)
        )",
        [],
    )?;

    // Create community_members table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS community_members (
            id BLOB PRIMARY KEY,
            community_id BLOB NOT NULL,
            user_id BLOB NOT NULL,
            public_key TEXT NOT NULL,
            display_name TEXT,
            role TEXT NOT NULL DEFAULT 'member',
            invited_by BLOB,
            joined_at TEXT NOT NULL,
            FOREIGN KEY (community_id) REFERENCES communities (id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users (id),
            FOREIGN KEY (invited_by) REFERENCES users (id),
            UNIQUE(community_id, user_id)
        )",
        [],
    )?;

    // Create community_posts table (maps posts to communities)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS community_posts (
            id BLOB PRIMARY KEY,
            community_id BLOB NOT NULL,
            post_id BLOB NOT NULL,
            show_in_main_feed BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (community_id) REFERENCES communities (id) ON DELETE CASCADE,
            FOREIGN KEY (post_id) REFERENCES posts (id) ON DELETE CASCADE,
            UNIQUE(community_id, post_id)
        )",
        [],
    )?;

    // Create community_invites table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS community_invites (
            id BLOB PRIMARY KEY,
            community_id BLOB NOT NULL,
            creator_id BLOB NOT NULL,
            invite_code TEXT UNIQUE NOT NULL,
            uses_remaining INTEGER NOT NULL DEFAULT 1,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (community_id) REFERENCES communities (id) ON DELETE CASCADE,
            FOREIGN KEY (creator_id) REFERENCES users (id)
        )",
        [],
    )?;

    // Create app_settings table for global application settings
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;

    // Insert default settings if they don't exist
    // Default storage: 10 GB (10737418240 bytes)
    // Device ID: unique identifier for this device instance
    let device_id = generate_device_id();
    conn.execute(
        "INSERT OR IGNORE INTO app_settings (key, value, updated_at) VALUES
            ('storage_limit_bytes', '10737418240', datetime('now')),
            ('storage_used_bytes', '0', datetime('now')),
            ('device_id', ?1, datetime('now'))",
        [&device_id],
    )?;

    // Indexes for the hot query paths. Created here so fresh databases get them,
    // and IF NOT EXISTS so existing databases pick them up on the next launch.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_posts_user_created ON posts(user_id, created_at);
         CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages(sender_id);
         CREATE INDEX IF NOT EXISTS idx_messages_recipient ON messages(recipient_id);
         CREATE INDEX IF NOT EXISTS idx_post_comments_post ON post_comments(post_id);
         CREATE INDEX IF NOT EXISTS idx_post_reactions_post ON post_reactions(post_id);
         CREATE INDEX IF NOT EXISTS idx_notifications_user_read ON notifications(user_id, read);
         CREATE INDEX IF NOT EXISTS idx_p2p_connections_friend ON p2p_connections(friend_user_id);",
    )?;

    Ok(())
}

/// Column-adding migration that tolerates the column already existing but
/// surfaces every other failure (I/O error, locked database, ...). Ignoring all
/// errors - as this used to - lets the app run on a half-migrated schema.
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> SqliteResult<()> {
    if column_exists(conn, table, column)? {
        return Ok(());
    }
    match conn.execute(
        &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition),
        [],
    ) {
        Ok(_) => Ok(()),
        // Racing another connection that just added it
        Err(e) if is_duplicate_column(&e) => Ok(()),
        Err(e) => Err(e),
    }
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> SqliteResult<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name.eq_ignore_ascii_case(column) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_duplicate_column(err: &rusqlite::Error) -> bool {
    err.to_string().contains("duplicate column name")
}

pub fn run_migrations(conn: &Connection) -> SqliteResult<()> {
    // Rename migrations from the pre-BLOB-id era. These only apply to databases
    // that still have a `username` column; guarded on the old column existing so
    // a genuine failure isn't swallowed.
    if column_exists(conn, "users", "username")? && !column_exists(conn, "users", "display_name")? {
        conn.execute(
            "ALTER TABLE users RENAME COLUMN username TO display_name",
            [],
        )?;
    }
    if column_exists(conn, "friend_invites", "username")?
        && !column_exists(conn, "friend_invites", "display_name")?
    {
        conn.execute(
            "ALTER TABLE friend_invites RENAME COLUMN username TO display_name",
            [],
        )?;
    }

    // Migration: Add P2P columns to devices table
    add_column_if_missing(conn, "devices", "iroh_node_id", "TEXT")?;
    add_column_if_missing(conn, "devices", "relay_url", "TEXT")?;

    // Migration: Add P2P columns to p2p_connections table
    add_column_if_missing(conn, "p2p_connections", "iroh_node_id", "TEXT")?;
    add_column_if_missing(conn, "p2p_connections", "friend_relay_url", "TEXT")?;

    // Migration: Add profile_signature column to users table
    add_column_if_missing(conn, "users", "profile_signature", "TEXT")?;

    // Migration: Add known_display_name to p2p_connections for tracking friend name changes
    add_column_if_missing(conn, "p2p_connections", "known_display_name", "TEXT")?;

    // Migration: Add friend_profile_signature to p2p_connections for signature verification
    add_column_if_missing(conn, "p2p_connections", "friend_profile_signature", "TEXT")?;

    // Migration: Add a friend's current rotating pre-key (forward secrecy).
    // prekey_public is the friend's current X25519 pre-key we seal to;
    // prekey_updated_at gates freshness so we fall back to their identity key
    // if we've missed too many rotations.
    add_column_if_missing(conn, "users", "prekey_public", "TEXT")?;
    add_column_if_missing(conn, "users", "prekey_updated_at", "INTEGER")?;

    // Migration: messages.edited_at - previously queried by get_message_thread but
    // never created, so that query always failed with "no such column".
    add_column_if_missing(conn, "messages", "edited_at", "TEXT")?;

    Ok(())
}

/// Delete rows that were stranded while foreign key enforcement was off.
/// Without `PRAGMA foreign_keys=ON` every ON DELETE CASCADE was a no-op, so
/// deleting a post left its media BLOBs, reactions and comments behind - a
/// privacy problem for image data in particular. Runs on every startup; it is a
/// no-op once the database is clean.
pub fn cleanup_orphans(conn: &Connection) -> SqliteResult<()> {
    // Foreign keys are enforced by this point, so the deletes below cascade
    // (e.g. removing an orphaned comment removes its replies).
    let statements = [
        "DELETE FROM media_attachments WHERE post_id IS NOT NULL
            AND post_id NOT IN (SELECT id FROM posts)",
        "DELETE FROM post_reactions WHERE post_id NOT IN (SELECT id FROM posts)",
        "DELETE FROM post_comments WHERE post_id NOT IN (SELECT id FROM posts)",
        "DELETE FROM post_comments WHERE parent_comment_id IS NOT NULL
            AND parent_comment_id NOT IN (SELECT id FROM post_comments)",
        "DELETE FROM message_reactions WHERE message_id NOT IN (SELECT id FROM messages)",
        "DELETE FROM community_members
            WHERE community_id NOT IN (SELECT id FROM communities)",
        "DELETE FROM community_posts
            WHERE community_id NOT IN (SELECT id FROM communities)
               OR post_id NOT IN (SELECT id FROM posts)",
        "DELETE FROM community_invites
            WHERE community_id NOT IN (SELECT id FROM communities)",
    ];

    let mut removed = 0usize;
    for sql in statements {
        removed += conn.execute(sql, [])?;
    }

    if removed > 0 {
        println!(
            "[DB] Removed {} orphaned row(s) stranded by missing FK enforcement",
            removed
        );
    }

    // Reclaim space from deleted media BLOBs so the bytes actually leave the file
    if removed > 0 {
        let _ = conn.execute_batch("VACUUM;");
    }

    Ok(())
}
