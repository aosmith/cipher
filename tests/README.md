# Cipher Test Suite

This directory contains comprehensive tests for the Cipher application, covering both functional and end-to-end testing scenarios.

## Test Files

### 1. `functional_tests.rs`
**Unit and Integration Tests for Core Functionality**

Tests the core Rust backend functionality without the UI:
- User registration and authentication
- Cryptographic key generation and validation
- Friend management system
- Encrypted messaging functionality
- Database operations and data persistence
- Security boundaries and validation
- Concurrent operations handling

**Key Test Scenarios:**
- ✅ User registration with unique usernames
- ✅ Login with correct/incorrect credentials
- ✅ Ed25519 + X25519 key generation and validation
- ✅ Friend adding and relationship management
- ✅ Encrypted message sending and receiving
- ✅ Data persistence across database connections
- ✅ Concurrent message handling
- ✅ Security boundary enforcement

### 2. `end_to_end_tests.rs`
**Complete User Journey Tests via UI Driver**

Tests the full application through the user interface using tauri-driver:
- Complete user signup and login flow
- Dashboard navigation and tab switching
- Posts feed viewing and interaction
- Messaging interface testing
- Friends management interface
- Form validation and error handling
- Multi-user interaction workflows
- UI accessibility and usability

**Key Test Scenarios:**
- ✅ User signs up successfully
- ✅ User logs in and sees dashboard
- ✅ User navigates between Posts, Messages, Friends tabs
- ✅ User attempts to send messages (tests interface)
- ✅ User tries to add friends (tests interface)
- ✅ User logs out and can log back in
- ✅ Form validation works correctly
- ✅ Error messages display appropriately
- ✅ Complete user journey from signup to logout

### 3. `app_driver_tests.rs`
**UI Component and Interaction Tests**

Tests specific UI components and user interactions:
- App launch and initialization
- Form switching between login/register
- Dashboard tab navigation
- Individual interface element testing
- Button clicks and form submissions
- Error message display and handling

### 4. `desktop_app_native_test.rs`
**Desktop App Specific Tests**

Tests desktop application specific functionality:
- App window creation and title verification
- Glassmorphism UI element presence
- Form element accessibility and display
- Native app specific features

## Running Tests

### Prerequisites
- Tauri development environment set up
- Test database can be created in temp directory
- For UI tests: Display environment available

### Fixture Utilities
- Deterministic Alice/Bob identities with exchanged messages can be generated via:
  ```bash
  cargo run --bin seed_fixture
  ```
  The command creates `/tmp/cipher_fixture/{alice,bob}` with isolated homes (database, storage). These directories power the macOS native UI harness and can be copied into simulator environments for mobile smoke tests.

### Run All Tests
```bash
cargo test
```

### Run Specific Test Suites
```bash
# Run only functional tests (backend logic)
cargo test functional_tests

# Run only end-to-end tests (full UI workflow)
cargo test end_to_end_tests

# Run only app driver tests (UI components)
cargo test app_driver_tests

# Run only desktop app tests
cargo test desktop_app_native_test

# macOS native UI smoke test (launches Cipher twice with seeded storage)
tests/run-macos-native-ui.sh

# Android Espresso smoke test (device/emulator required)
scripts/run-android-ui-tests.sh

# iOS simulator UI test
SKIP_TAURI_RUST_BUILD=1 xcodebuild -scheme cipher-social_iOS \
  -destination 'platform=iOS Simulator,name=iPhone 15 Pro,OS=17.0' test
```

### Run Individual Tests
```bash
# Example: Run just the user registration test
cargo test test_user_registration_and_authentication

# Example: Run just the complete user journey test
cargo test test_complete_user_journey
```

## Test Structure

### Functional Tests
- Use temporary SQLite databases for isolation
- Test pure Rust backend functionality
- No UI dependencies
- Fast execution
- Focus on business logic correctness

### UI Tests (End-to-End & Driver Tests)
- Use tauri-driver for browser automation
- Test complete user workflows
- Include visual and interaction testing
- Slower execution due to UI rendering
- Focus on user experience validation

## What Gets Tested

### Core Features
1. **User Management**
   - Registration with display name only
   - Recovery phrase generation (24 words BIP39)
   - Account restoration via recovery phrase
   - Logout functionality
   - Data persistence

2. **Cryptography**
   - Ed25519 signing key generation
   - X25519 encryption key derivation
   - BIP39 recovery phrase generation
   - Deterministic key derivation from recovery phrase
   - Key uniqueness and format validation

3. **Social Features**
   - Friend adding and management
   - Friendship validation and security
   - Friend list retrieval

4. **Messaging**
   - End-to-end encrypted message sending
   - Message storage and retrieval
   - Message ordering and timestamps
   - Security boundaries (non-friends can't message)

5. **User Interface**
   - Form validation and error handling
   - Navigation between application sections
   - Responsive design and accessibility
   - Glassmorphism visual effects

6. **Data Integrity**
   - Database operations
   - Concurrent access handling
   - Transaction safety
   - Data persistence across sessions

## Test Coverage Goals

- ✅ **User Authentication**: Complete signup/login flow
- ✅ **Core Features**: All major functionality tested
- ✅ **Security**: Cryptography and access controls validated
- ✅ **UI/UX**: Complete user journeys verified
- ✅ **Error Handling**: All error cases covered
- ✅ **Data Safety**: Persistence and integrity confirmed
- ✅ **Concurrency**: Multi-user scenarios tested
- ✅ **Accessibility**: UI elements and navigation verified

## Test Data Management

- **Functional Tests**: Use temporary databases, automatically cleaned up
- **UI Tests**: Create test users with predictable credentials
- **Isolation**: Each test runs independently
- **Cleanup**: All test data is ephemeral and self-cleaning

## CI/CD Integration

These tests are designed to run in automated environments:
- No external dependencies required
- Self-contained database creation
- Headless UI testing supported
- Deterministic test outcomes
- Fast feedback for development workflow

## Debugging Tests

### For Functional Tests
```bash
# Run with output
cargo test test_name -- --nocapture

# Run single-threaded for debugging
cargo test -- --test-threads=1
```

### For UI Tests
```bash
# Enable verbose output
RUST_LOG=debug cargo test test_name

# Keep browser open for debugging (if supported)
cargo test test_name -- --nocapture
```

## Adding New Tests

### For New Features
1. Add functional tests for backend logic in `functional_tests.rs`
2. Add UI workflow tests in `end_to_end_tests.rs`
3. Update this README with new test descriptions

### Test Naming Convention
- Use descriptive names: `test_user_can_send_encrypted_message`
- Group related tests with prefixes: `test_crypto_*`, `test_ui_*`
- Include expected behavior: `test_invalid_login_shows_error`

### Test Organization
- **Arrange**: Set up test data and environment
- **Act**: Perform the action being tested
- **Assert**: Verify expected outcomes
- **Cleanup**: Clean up resources (automatic for temp databases)
