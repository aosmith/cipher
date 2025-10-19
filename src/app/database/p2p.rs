use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

use crate::app::Database;

impl Database {
    /// Save a peer address to the persistent address book
    pub fn save_peer_address(&self, peer_id: &str, multiaddr: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO peer_addresses (peer_id, multiaddr, last_seen, connection_success_count, connection_failure_count)
             VALUES (?1, ?2, ?3, 0, 0)
             ON CONFLICT(peer_id, multiaddr) DO UPDATE SET last_seen = ?3",
            params![peer_id, multiaddr, now],
        )?;

        Ok(())
    }

    /// Load all peer addresses from the persistent address book
    pub fn load_peer_addresses(&self) -> SqliteResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT peer_id, multiaddr FROM peer_addresses ORDER BY last_seen DESC")?;

        let addresses = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>("peer_id")?,
                row.get::<_, String>("multiaddr")?,
            ))
        })?;

        let mut result = Vec::new();
        for addr in addresses {
            result.push(addr?);
        }

        Ok(result)
    }

    /// Record a successful connection to a peer address
    #[allow(dead_code)]
    pub fn record_peer_connection_success(
        &self,
        peer_id: &str,
        multiaddr: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE peer_addresses
             SET connection_success_count = connection_success_count + 1,
                 last_seen = ?3
             WHERE peer_id = ?1 AND multiaddr = ?2",
            params![peer_id, multiaddr, now],
        )?;

        Ok(())
    }

    /// Record a failed connection attempt to a peer address
    #[allow(dead_code)]
    pub fn record_peer_connection_failure(
        &self,
        peer_id: &str,
        multiaddr: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE peer_addresses
             SET connection_failure_count = connection_failure_count + 1
             WHERE peer_id = ?1 AND multiaddr = ?2",
            params![peer_id, multiaddr],
        )?;

        Ok(())
    }

    /// Clean up unreliable peer addresses (10+ failures with <10% success rate)
    #[allow(dead_code)]
    pub fn clean_unreliable_peer_addresses(&self) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM peer_addresses
             WHERE (connection_success_count + connection_failure_count) >= 10
               AND (connection_success_count * 1.0 / (connection_success_count + connection_failure_count)) < 0.1",
            [],
        )?;

        Ok(())
    }
}
