use crate::app::{Database, Device, DeviceInfo};
use chrono::Utc;
use rusqlite::Result as SqliteResult;

impl Database {
    /// Get all devices for a user (identified by public key)
    pub fn get_user_devices(&self, user_public_key: &str) -> SqliteResult<Vec<Device>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_public_key, device_name, last_sync, created_at
             FROM devices
             WHERE user_public_key = ?
             ORDER BY last_sync DESC",
        )?;

        let devices = stmt
            .query_map([user_public_key], |row| {
                Ok(Device {
                    id: row.get("id")?,
                    user_public_key: row.get("user_public_key")?,
                    device_name: row.get("device_name")?,
                    last_sync: row.get("last_sync")?,
                    created_at: row.get("created_at")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(devices)
    }

    /// Get a specific device by ID
    pub fn get_device(&self, device_id: &str) -> SqliteResult<Option<Device>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_public_key, device_name, last_sync, created_at
             FROM devices
             WHERE id = ?",
        )?;

        let mut rows = stmt.query([device_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Device {
                id: row.get("id")?,
                user_public_key: row.get("user_public_key")?,
                device_name: row.get("device_name")?,
                last_sync: row.get("last_sync")?,
                created_at: row.get("created_at")?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update device name
    pub fn update_device_name(&self, device_id: &str, device_name: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE devices SET device_name = ? WHERE id = ?",
            [device_name, device_id],
        )?;
        Ok(())
    }

    /// Update device last_sync timestamp
    pub fn update_device_sync(&self, device_id: &str) -> SqliteResult<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE devices SET last_sync = ? WHERE id = ?",
            [&now, device_id],
        )?;
        Ok(())
    }

    /// Remove a device (e.g., user wants to unlink a device)
    pub fn remove_device(&self, device_id: &str, user_public_key: &str) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "DELETE FROM devices WHERE id = ? AND user_public_key = ?",
            [device_id, user_public_key],
        )?;
        Ok(rows_affected > 0)
    }

    /// Get device info for display (includes whether it's the current device)
    pub fn get_device_info_list(
        &self,
        user_public_key: &str,
        current_device_id: &str,
    ) -> SqliteResult<Vec<DeviceInfo>> {
        let devices = self.get_user_devices(user_public_key)?;

        let device_infos = devices
            .into_iter()
            .map(|device| DeviceInfo {
                id: device.id.clone(),
                device_name: device.device_name.clone(),
                last_sync: device.last_sync.clone(),
                created_at: device.created_at.clone(),
                is_current: device.id == current_device_id,
            })
            .collect();

        Ok(device_infos)
    }

    /// Check if a device exists and belongs to a user
    pub fn verify_device_ownership(
        &self,
        device_id: &str,
        user_public_key: &str,
    ) -> SqliteResult<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM devices WHERE id = ? AND user_public_key = ?")?;

        let count: i64 = stmt.query_row([device_id, user_public_key], |row| row.get("COUNT(*)"))?;
        Ok(count > 0)
    }

    /// Get count of devices for a user
    pub fn get_device_count(&self, user_public_key: &str) -> SqliteResult<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM devices WHERE user_public_key = ?")?;

        let count: i64 = stmt.query_row([user_public_key], |row| row.get("COUNT(*)"))?;
        Ok(count as usize)
    }

    /// Update device's Iroh NodeId
    pub fn update_device_node_id(&self, device_id: &str, node_id: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE devices SET iroh_node_id = ? WHERE id = ?",
            [node_id, device_id],
        )?;
        Ok(())
    }

    /// Get Iroh NodeIds for all other devices with the same user (excluding current device)
    #[allow(dead_code)]
    pub fn get_peer_node_ids(
        &self,
        user_public_key: &str,
        exclude_device_id: &str,
    ) -> SqliteResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT iroh_node_id FROM devices
             WHERE user_public_key = ? AND id != ? AND iroh_node_id IS NOT NULL",
        )?;

        let node_ids = stmt
            .query_map([user_public_key, exclude_device_id], |row| {
                row.get("iroh_node_id")
            })?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(node_ids)
    }

    /// Get all Iroh NodeIds from all devices in the database (for peer discovery)
    #[allow(dead_code)]
    pub fn get_all_node_ids(&self) -> SqliteResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT DISTINCT iroh_node_id FROM devices WHERE iroh_node_id IS NOT NULL")?;

        let node_ids = stmt
            .query_map([], |row| row.get("iroh_node_id"))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(node_ids)
    }

    /// Update device's Iroh relay URL
    pub fn update_device_relay_url(&self, device_id: &str, relay_url: &str) -> SqliteResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE devices SET relay_url = ? WHERE id = ?",
            [relay_url, device_id],
        )?;
        Ok(())
    }

    /// Get all Iroh NodeIds with relay URLs from all devices (for peer discovery with full addressing)
    #[allow(dead_code)]
    pub fn get_all_peer_addrs(&self) -> SqliteResult<Vec<(String, Option<String>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT iroh_node_id, relay_url FROM devices WHERE iroh_node_id IS NOT NULL",
        )?;

        let peer_addrs = stmt
            .query_map([], |row| {
                let node_id: String = row.get("iroh_node_id")?;
                let relay_url: Option<String> = row.get("relay_url").ok();
                Ok((node_id, relay_url))
            })?
            .collect::<Result<Vec<(String, Option<String>)>, _>>()?;

        Ok(peer_addrs)
    }

    /// Get Iroh NodeIds with relay URLs for peer devices (same user, excluding current device)
    #[allow(dead_code)]
    pub fn get_peer_addrs(
        &self,
        user_public_key: &str,
        exclude_device_id: &str,
    ) -> SqliteResult<Vec<(String, Option<String>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT iroh_node_id, relay_url FROM devices
             WHERE user_public_key = ? AND id != ? AND iroh_node_id IS NOT NULL",
        )?;

        let peer_addrs = stmt
            .query_map([user_public_key, exclude_device_id], |row| {
                let node_id: String = row.get("iroh_node_id")?;
                let relay_url: Option<String> = row.get("relay_url").ok();
                Ok((node_id, relay_url))
            })?
            .collect::<Result<Vec<(String, Option<String>)>, _>>()?;

        Ok(peer_addrs)
    }
}
