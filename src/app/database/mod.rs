use rusqlite::{Connection, Result as SqliteResult};
use std::sync::{Arc, Mutex};

use crate::app::types::NotificationServer;

// Database wrapper
#[derive(Clone)]
pub struct Database {
    pub conn: Arc<Mutex<Connection>>,
    pub notification_server: Arc<NotificationServer>,
}

impl Database {
    pub fn new(db_path: &str) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode for better concurrency
        // WAL allows readers to not block writers and vice versa
        // This prevents deadlocks between frontend queries and network handler writes
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        // Create all tables
        schema::create_tables(&conn)?;

        // Run migrations
        schema::run_migrations(&conn)?;

        // Initialize notification server
        let notification_server = Arc::new(NotificationServer::new());

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
            notification_server,
        })
    }

    /// Checkpoint the WAL file to ensure all data is written to the main database
    /// Call this before app goes to background or quits to prevent data loss
    pub fn checkpoint(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        // TRUNCATE mode: checkpoint and truncate the WAL file to zero bytes
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        println!("[DB] WAL checkpoint completed");
        Ok(())
    }
}

// Module declarations
mod communities;
pub mod crypto;
mod devices;
mod friends;
mod media;
mod messaging;
mod notifications;
mod pending_messages;
mod posts;
pub mod prekeys;
mod safety;
mod schema;
pub mod settings;
pub mod sync;
mod users;
mod utils;
