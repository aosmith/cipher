# Cipher Release Changelog

## v0.19.0 - 2025-10-14

### Major Security Hardening Release

This release implements comprehensive cryptographic improvements and security hardening measures to enhance the privacy and security of the Cipher social network.

### Security Improvements

**Forward Secrecy**
- Implemented per-message ephemeral X25519 keys for perfect forward secrecy
- Messages can no longer be decrypted even if long-term private keys are compromised
- Each message uses a unique ephemeral key pair that is discarded after use

**Replay Attack Prevention**
- Added timestamp-based message freshness validation (5-minute window)
- Messages older than 5 minutes are automatically rejected
- Prevents attackers from replaying captured encrypted messages

**Enhanced Encryption**
- Upgraded from ChaCha20Poly1305 to XChaCha20Poly1305
- Extended nonce size from 96 bits to 192 bits for better collision resistance
- Maintains AEAD (Authenticated Encryption with Associated Data) properties

**Domain-Separated Key Derivation**
- Separate key derivation for signing keys vs encryption keys
- Uses domain separation prefixes: "cipher_signing_v1:" and "cipher_encryption_v1:"
- Prevents key confusion attacks between different cryptographic contexts

**Cryptographically Secure Random Salts**
- Generate unique 32-byte random salt for each user
- Uses ring::rand::SystemRandom for cryptographic security
- Database migration automatically adds kdf_salt column for existing users

**Private Key Protection**
- Enhanced serialization guards prevent accidental private key exposure
- Context-aware User model serialization (own vs others' private keys)
- Parameter filtering prevents private keys in application logs
- Private keys never transmitted over P2P network channels

### UI Improvements

**Mobile Navigation**
- Fixed mobile navbar left margin alignment issue
- Improved spacing and visual consistency on mobile devices

### Testing

**Comprehensive Test Suite**
- Updated cryptography tests for new domain-separated API (14/14 passing)
- Updated database tests for new schema (11/11 passing)
- Fixed libp2p network tests for new message format
- Refactored and cleaned up redundant test code
- All 25 core tests passing

### Technical Details

**Cryptographic Stack**
- Ed25519 for digital signatures and identity
- X25519 for encryption key exchange (with ephemeral keys)
- XChaCha20Poly1305 for AEAD encryption
- PBKDF2-HMAC-SHA256 (100,000 iterations) for key derivation
- ring crate for cryptographically secure random number generation

**Database Schema**
- Added kdf_salt column to users table (BLOB, 32 bytes)
- Automatic migration for existing installations
- Backward compatible with previous versions

### Files Changed

- Core security: crypto.rs, schema.rs, users.rs, friends.rs, user.rs
- Tests: cryptography_tests.rs, database_tests.rs, feed_with_posts_test.rs, libp2p_network_tests.rs
- UI: styles.css, index.html, main.js, navbar.js, p2p.js
- P2P: libp2p_commands.rs, libp2p_network.rs
- Build: Cargo.lock

### Upgrade Notes

- This release includes a database migration that adds the kdf_salt column
- Existing users will be automatically migrated on first launch
- No user action required - migrations run automatically
- Forward secrecy is enabled by default for all new messages

### Security Advisories

- Private keys are stored locally and never transmitted over P2P
- Users should keep their devices secure as private keys are stored on disk
- Consider using full-disk encryption for additional protection

---

## v0.18.8 - 2025-10-07

### Improvements

- Add Linux ARM64 package support
- Build system improvements
- UI refinements

## v0.18.7 - 2025-10-06

### Improvements

- Build system improvements
- UI refinements

## v0.18.6 - 2025-10-05

### Features

- Frontend testing infrastructure
- Build optimizations

## v0.18.5 - 2025-10-04

### Features

- Theme toggle
- Simplified friend addition
- P2P improvements
