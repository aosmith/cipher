// Tests for app settings and storage quota accounting
// (src/app/database/settings.rs).

use app::Database;
use tempfile::TempDir;

fn fresh_db() -> Database {
    let temp_dir = TempDir::new().unwrap();
    let db = Database::new(&temp_dir.path().join("test.db").to_string_lossy()).unwrap();
    std::mem::forget(temp_dir);
    db
}

#[test]
fn test_get_setting_missing_returns_none() {
    let db = fresh_db();
    assert_eq!(db.get_setting("does_not_exist").unwrap(), None);
}

#[test]
fn test_set_and_get_setting() {
    let db = fresh_db();
    db.set_setting("theme", "dark").unwrap();
    assert_eq!(db.get_setting("theme").unwrap(), Some("dark".to_string()));
}

#[test]
fn test_set_setting_upserts() {
    let db = fresh_db();
    db.set_setting("theme", "dark").unwrap();
    db.set_setting("theme", "light").unwrap();
    // Second write updates rather than duplicating.
    assert_eq!(db.get_setting("theme").unwrap(), Some("light".to_string()));
}

#[test]
fn test_get_app_settings_defaults() {
    let db = fresh_db();
    let settings = db.get_app_settings().unwrap();
    // Default 10 GB limit, nothing used yet.
    assert_eq!(settings.storage_limit_bytes, 10_737_418_240);
    assert_eq!(settings.storage_used_bytes, 0);
}

#[test]
fn test_set_storage_limit() {
    let db = fresh_db();
    db.set_storage_limit(1024).unwrap();
    assert_eq!(db.get_app_settings().unwrap().storage_limit_bytes, 1024);
}

#[test]
fn test_add_storage_used_accumulates() {
    let db = fresh_db();
    assert_eq!(db.add_storage_used(100).unwrap(), 100);
    assert_eq!(db.add_storage_used(250).unwrap(), 350);
    assert_eq!(db.get_app_settings().unwrap().storage_used_bytes, 350);
}

#[test]
fn test_can_store_within_limit() {
    let db = fresh_db();
    db.set_storage_limit(1000).unwrap();
    db.add_storage_used(600).unwrap();

    // 600 + 300 = 900 <= 1000.
    assert!(db.can_store(300).unwrap());
    // 600 + 401 = 1001 > 1000.
    assert!(!db.can_store(401).unwrap());
    // Exactly at the limit is allowed.
    assert!(db.can_store(400).unwrap());
}

#[test]
fn test_can_store_disabled_when_limit_zero() {
    let db = fresh_db();
    db.set_storage_limit(0).unwrap();
    assert!(
        !db.can_store(1).unwrap(),
        "a zero (or negative) limit disables storage"
    );
}
