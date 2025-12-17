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
}

// Module declarations
pub mod crypto;
mod devices;
mod friends;
mod media;
mod messaging;
mod notifications;
mod pending_messages;
mod posts;
mod safety;
mod schema;
pub mod settings;
pub mod sync;
mod users;
mod utils;
