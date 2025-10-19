use rusqlite::{Connection, Result as SqliteResult};

pub fn create_tables(conn: &Connection) -> SqliteResult<()> {
    // Create users table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id BLOB PRIMARY KEY,
            username TEXT NOT NULL,
            public_key TEXT UNIQUE,
            private_key TEXT,
            encryption_public_key TEXT,
            encryption_private_key TEXT,
            device_id TEXT,
            bio TEXT,
            profile_picture TEXT,
            recovery_phrase_hash TEXT,
            recovery_phrase_shown BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
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

    // Create voice_messages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS voice_messages (
            id BLOB PRIMARY KEY,
            sender_id BLOB NOT NULL,
            recipient_id BLOB NOT NULL,
            audio_data TEXT NOT NULL,
            duration_seconds REAL NOT NULL,
            waveform TEXT,
            encrypted BOOLEAN DEFAULT 1,
            thread_id BLOB,
            created_at TEXT NOT NULL,
            FOREIGN KEY (sender_id) REFERENCES users (id),
            FOREIGN KEY (recipient_id) REFERENCES users (id),
            FOREIGN KEY (thread_id) REFERENCES messages (id)
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
            username TEXT NOT NULL,
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

    // Create call_logs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS call_logs (
            id BLOB PRIMARY KEY,
            call_id TEXT UNIQUE NOT NULL,
            caller_id BLOB NOT NULL,
            callee_id BLOB NOT NULL,
            call_type TEXT NOT NULL, -- 'audio' or 'video'
            status TEXT NOT NULL, -- 'calling', 'connected', 'ended', 'rejected', 'missed'
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_seconds INTEGER,
            FOREIGN KEY (caller_id) REFERENCES users (id),
            FOREIGN KEY (callee_id) REFERENCES users (id)
        )",
        [],
    )?;

    // Create peer_addresses table for persistent P2P peer address book
    conn.execute(
        "CREATE TABLE IF NOT EXISTS peer_addresses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            peer_id TEXT NOT NULL,
            multiaddr TEXT NOT NULL,
            last_seen TEXT NOT NULL,
            connection_success_count INTEGER NOT NULL DEFAULT 0,
            connection_failure_count INTEGER NOT NULL DEFAULT 0,
            UNIQUE(peer_id, multiaddr)
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
            created_at TEXT NOT NULL
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

    // Create typing_indicators table for real-time typing status
    conn.execute(
        "CREATE TABLE IF NOT EXISTS typing_indicators (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id BLOB NOT NULL,
            conversation_partner_id BLOB NOT NULL,
            is_typing BOOLEAN DEFAULT 0,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id),
            FOREIGN KEY (conversation_partner_id) REFERENCES users (id),
            UNIQUE(user_id, conversation_partner_id)
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

    Ok(())
}

pub fn run_migrations(_conn: &Connection) -> SqliteResult<()> {
    // No migrations needed for v0.0.1 - this is the initial schema
    // Future migrations will be added here as the schema evolves
    Ok(())
}
