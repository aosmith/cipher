# Manual Testing Scripts

This directory contains scripts for manually testing Cipher's P2P functionality with two running instances.

## Two-Instance Testing

Testing P2P features like friend adding and messaging requires running two separate instances of the application simultaneously.

### Quick Start

```bash
# From the project root
./test_two_instances.sh
```

This script:
- Cleans up old test data
- Creates separate database directories for Alice and Bob
- Launches two instances with isolated databases
- Provides interactive testing instructions

### Visual Test Script

For comprehensive testing with screenshots:

```bash
./tests/manual/two_instance_visual_test.sh
```

This interactive script:
- Launches two instances (Alice & Bob)
- Guides you through each test step
- Takes screenshots at each stage
- Saves logs for debugging
- Validates that friend adding and messaging work

Screenshots are saved to: `/tmp/cipher-test-screenshots/`

## Test Scenarios

### 1. Friend Adding
- Create two users with different credentials
- Verify each has a unique public key
- Add each other as friends using public keys
- Confirm friends appear in friends list

### 2. Encrypted Messaging
- Select friend from search
- Send encrypted message
- Verify message appears on both instances
- Test bidirectional messaging

### 3. Posts & Feed
- Create posts on both instances
- Verify posts appear in feed
- Test cross-instance post synchronization

## Technical Details

### Separate Databases

Each instance uses a separate database via the `CIPHER_TEST_DATA_DIR` environment variable:

```bash
CIPHER_TEST_DATA_DIR="/tmp/cipher-test-alice" ./target/release/cipher-social
CIPHER_TEST_DATA_DIR="/tmp/cipher-test-bob" ./target/release/cipher-social
```

This ensures:
- Different user identities (public/private keys)
- Isolated data storage
- True P2P testing environment

### Log Files

Application logs are saved for debugging:
- Alice: `/tmp/cipher-test-alice/output.log`
- Bob: `/tmp/cipher-test-bob/output.log`

### Cleanup

To clean up test data:

```bash
rm -rf /tmp/cipher-test-*
pkill -f cipher-social
```

## Troubleshooting

### Both instances show same public key
- Make sure you're running the **binary directly**, not via `open` command
- The test script has been fixed to use `./target/release/cipher-social`
- Environment variables don't pass through macOS `open` command

### Friend adding fails
- Verify public keys are different on each instance
- Check logs for error messages
- Ensure you're not trying to add yourself as a friend

### Messages not sending
- Verify friend was added successfully
- Check that recipient is selected (search and click on friend)
- Review logs for encryption/network errors

## Development

When modifying P2P functionality:

1. Run `cargo build --release` to rebuild
2. Run `./test_two_instances.sh` for quick testing
3. Run `./tests/manual/two_instance_visual_test.sh` for comprehensive validation
4. Review screenshots and logs to verify behavior

## Requirements

- macOS (for `screencapture` command in visual test)
- Rust toolchain
- SQLite
- Two terminal windows or tmux/screen for monitoring logs
