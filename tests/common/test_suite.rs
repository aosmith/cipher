// Test Suite - Simple utilities for testing Cipher
use app::database::Database;
use app::types::User;
use tempfile::TempDir;

/// Create a new temporary database for a test
pub fn create_test_db() -> (Database, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db = Database::new(&db_path.to_string_lossy())
        .expect("Failed to create database");
    (db, temp_dir)
}

/// Create a test user with a given username
pub fn create_test_user(db: &Database, username: &str) -> (User, String) {
    let device_id = Database::generate_device_id();
    db.create_user_first_launch(username.to_string(), device_id)
        .expect("Failed to create user")
}

#[cfg(test)]
mod test_suite_tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let (db, _dir) = create_test_db();
        let device_id = Database::generate_device_id();
        let result = db.create_user_first_launch("test_user".to_string(), device_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_user_creation() {
        let (db, _dir) = create_test_db();
        let (user, recovery_phrase) = create_test_user(&db, "alice");

        assert_eq!(user.username, "alice");
        assert!(!recovery_phrase.is_empty());
        assert!(user.public_key.is_some());
        assert!(user.private_key.is_some());
    }
}
