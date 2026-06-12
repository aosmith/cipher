// Common test utilities module
// This module is imported by all test files

pub use app::database::Database;
pub use app::types::{SqliteUuid, User};
pub use tempfile::TempDir;

// Re-export test suite functionality
mod test_suite;
pub use test_suite::*;

// Network test harness for P2P integration testing
pub mod network_harness;
pub use network_harness::*;
