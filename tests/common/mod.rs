// Common test utilities module
// This module is imported by all test files; each test binary uses a
// different subset of these re-exports, so allow the unused ones per-binary.

#[allow(unused_imports)]
pub use app::database::Database;
#[allow(unused_imports)]
pub use app::types::{SqliteUuid, User};
#[allow(unused_imports)]
pub use tempfile::TempDir;

// Re-export test suite functionality
mod test_suite;
#[allow(unused_imports)]
pub use test_suite::*;

// Network test harness for P2P integration testing
pub mod network_harness;
#[allow(unused_imports)]
pub use network_harness::*;
