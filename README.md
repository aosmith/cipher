

# Cipher

**End-to-End Encrypted Peer-to-Peer Social Network**

[![Version](https://img.shields.io/badge/version-0.0.1-blue.svg)](https://github.com/aosmith/cipher/releases)
[![License](https://img.shields.io/badge/license-AGPL--3.0-green.svg)](LICENSE.txt)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-2.8.5-brightgreen.svg)](https://tauri.app/)

---

## Overview

Cipher is a **local-first, serverless social network** built on peer-to-peer technology with end-to-end encryption at its core. Unlike traditional social platforms that rely on centralized servers, Cipher enables direct communication between users with zero trust in external infrastructure.

**Key Differentiators:**
- 🔐 **True End-to-End Encryption** - Ed25519 + X25519 + ChaCha20Poly1305
- 🌐 **Pure P2P Architecture** - No servers, no middlemen, powered by [Iroh](https://iroh.computer/)
- 💾 **Local-First** - All data stored locally in SQLite, you own your data
- 🔑 **Deterministic Identity** - Your identity derived cryptographically from your keys
- 🚫 **Zero Telemetry** - No tracking, no analytics, no data collection
- 🎨 **Beautiful UI** - Glassmorphism dark theme design

Built with **Rust** and **Tauri**, Cipher runs natively on macOS, Android, iOS, Linux, and Windows.

---

## Features

### Security & Privacy
- **End-to-End Encryption**: All messages encrypted with ChaCha20Poly1305
- **Recovery Phrase Authentication**: 24-word BIP39 mnemonic generated on first run
- **Deterministic Keys**: Private keys derived from recovery phrase only
- **Zero Knowledge**: Private keys never leave your device, never transmitted over network
- **Cryptographic Identity**: Public keys serve as primary identifier (prevents username collisions)
- **Local Storage**: All data encrypted and stored in local SQLite database

### Peer-to-Peer Networking
- **Iroh Protocol**: Built on [Iroh](https://iroh.computer/) for robust P2P communication
- **Gossip Protocol**: Message propagation across peer mesh network
- **Multi-Path Discovery**: DHT + DNS + Relay for reliable peer finding
- **NAT Traversal**: Automatic hole-punching for peer connectivity
- **Offline-First**: Works without internet, syncs when connected

### Cross-Platform
- ✅ **macOS** (Apple Silicon & Intel)
- ✅ **Android** (ARM64)
- ✅ **iOS** (with Apple Developer account)
- ✅ **Linux** (Debian, Ubuntu, Fedora, AppImage)
- ✅ **Windows** (Windows 10/11)

---

## Screenshots

<!-- TODO: Add screenshots of the application -->

### Main Feed
![Main Feed](docs/screenshots/feed.png)
*Home feed showing encrypted messages from peers*

### Chat Interface
![Chat](docs/screenshots/chat.png)
*Direct encrypted messaging with online status*

### Profile & Settings
![Profile](docs/screenshots/profile.png)
*User profile with QR code for peer discovery*

---

## Getting Started

### First Time Setup
1. **Launch Cipher** - Open the application
2. **Enter Display Name** - Choose how you want to be identified to others
3. **Save Recovery Phrase** - The app generates a 24-word recovery phrase
   - ⚠️ **CRITICAL**: Write this down on paper and store securely
   - This is shown ONLY ONCE and cannot be recovered
   - You'll need it to restore your account on other devices

### Restoring Your Account
1. **Launch Cipher** on a new device
2. **Click "Restore Existing Account"**
3. **Enter your display name** and **24-word recovery phrase**
4. Your account, keys, and identity are restored deterministically

### Security Notes
- **No passwords** - Authentication is based solely on cryptographic recovery phrases
- **No account recovery** - If you lose your recovery phrase, you cannot recover your account
- **Deterministic identity** - Same recovery phrase always generates the same keys
- **Multi-device support** - Use the same recovery phrase to access your account on multiple devices

---

## Quick Start

### Installation from Releases

Download the latest release for your platform:

**macOS:**
```bash
# Download from GitHub Releases
# https://github.com/aosmith/cipher/releases/latest

# Install DMG
open Cipher.dmg
```

**Android:**
```bash
# Download APK from GitHub Releases
# https://github.com/aosmith/cipher/releases/latest

# Install via ADB
adb install Cipher.apk
```

**Linux:**
```bash
# Debian/Ubuntu
sudo dpkg -i Cipher.deb

# Fedora/RHEL
sudo rpm -i Cipher.rpm

# AppImage (any distro)
chmod +x Cipher.AppImage
./Cipher.AppImage
```

### Building from Source

**Prerequisites:**
```bash
# Install Rust (required)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Tauri CLI
cargo install tauri-cli

# For Android builds
# Install Android Studio: https://developer.android.com/studio
export ANDROID_HOME=~/Library/Android/sdk
export NDK_HOME=$ANDROID_HOME/ndk

# For iOS builds
# Install Xcode from Mac App Store
```

**Clone and Build:**
```bash
git clone https://github.com/aosmith/cipher.git
cd cipher

# Desktop (macOS/Linux/Windows)
cargo tauri build

# Android
env ANDROID_HOME=~/Library/Android/sdk NDK_HOME=~/Library/Android/sdk/ndk \
  OPENSSL_STATIC=1 OPENSSL_VENDORED=1 \
  cargo tauri android build --target aarch64

# iOS
cargo tauri ios build
```

See [BUILD_INSTRUCTIONS.md](BUILD_INSTRUCTIONS.md) for platform-specific details.

---

## Architecture

### Technology Stack

**Frontend:**
- HTML5 + CSS3 (Glassmorphism design)
- Vanilla JavaScript (no framework dependencies)

**Backend:**
- **Rust** - Core application logic
- **Tauri 2.x** - Cross-platform application framework
- **SQLite** - Local database (via rusqlite)
- **Iroh** - P2P networking library

**Cryptography:**
- **Ed25519** - Digital signatures (identity)
- **X25519** - Key exchange (ECDH)
- **ChaCha20Poly1305** - Authenticated encryption (AEAD)
- **BIP39** - Recovery phrase generation and key derivation

**Networking:**
- **Iroh Gossip** - Message propagation protocol
- **QUIC** - Transport layer (via Quinn)
- **Mainline DHT** - Peer discovery
- **HTTPS/DNS** - Additional discovery mechanisms

### Security Model

```
First Run: Generate 24-word Recovery Phrase
    ↓
Display Once (User Must Save)
    ↓
Deterministic Key Derivation (BIP39)
    ↓
Private Key (Ed25519) → Public Key (Identity)
    ↓
X25519 Key Exchange → Shared Secret
    ↓
ChaCha20Poly1305 Encryption → Ciphertext
    ↓
P2P Network (Iroh Gossip)
```

**Key Security Properties:**
1. **Private keys derived locally** - Never transmitted, never stored in plaintext
2. **Public key as identity** - Prevents username collisions, enables trust
3. **Perfect forward secrecy** - Ephemeral key exchange per session
4. **Authenticated encryption** - AEAD guarantees integrity and authenticity
5. **Deterministic user IDs** - UUID v5 derived from public key

**Threat Model:**
- ✅ Protects against: Network eavesdropping, MITM attacks, server compromise
- ✅ Guarantees: Message confidentiality, authenticity, integrity
- ⚠️ Does NOT protect against: Endpoint compromise (device seizure), side-channel attacks

### P2P Networking (Iroh)

Cipher uses [Iroh](https://iroh.computer/) v1.0 for peer-to-peer communication:

**Discovery Methods:**
1. **Mainline DHT** - BitTorrent DHT for global peer discovery
2. **DNS** - Domain-based peer hints
3. **Relay Servers** - STUN/TURN-like fallback (user-operated)
4. **Local mDNS** - LAN peer discovery
5. **QR Codes** - Manual peer introduction

**Gossip Protocol:**
- **Topic-based** - Subscribe to `/cipher/presence`, `/cipher/messages/{user_id}`
- **Epidemic broadcast** - Messages propagate through mesh network
- **Eventual consistency** - All peers eventually receive all messages
- **Deduplication** - Message hashes prevent duplicate processing

**Connection Flow:**
```
Peer A                          Peer B
  |                                |
  | 1. Announce to DHT             |
  |------------------------------>|
  |                                |
  | 2. QUIC Connection             |
  |<----------------------------->|
  |                                |
  | 3. Gossip Handshake            |
  |<----------------------------->|
  |                                |
  | 4. Subscribe to Topics         |
  |<----------------------------->|
  |                                |
  | 5. Encrypted Messages          |
  |<=============================>|
```

### Database Schema

Local SQLite database (`cipher.db`) stores:

**Tables:**
- `users` - Local user accounts (encrypted private keys)
- `friends` - Peer public keys and metadata
- `messages` - Encrypted message history
- `feed_items` - Decrypted feed cache
- `files` - Uploaded media references

**Security:**
- Private keys encrypted at rest (recovery phrase required)
- Database file permissions: `0600` (owner read/write only)
- No cloud sync, no backups to external services

### Project Structure

```
cipher/
├── src/
│   ├── app/                      # Rust backend
│   │   ├── database/             # SQLite database layer
│   │   │   ├── mod.rs            # Database initialization
│   │   │   ├── users.rs          # User management
│   │   │   ├── friends.rs        # Friend/peer management
│   │   │   └── schema.rs         # Database schema
│   │   ├── crypto.rs             # Cryptography primitives
│   │   ├── iroh_network.rs       # Iroh P2P networking
│   │   ├── iroh_commands.rs      # Tauri commands for P2P
│   │   └── mod.rs                # Module exports
│   ├── main.rs                   # Tauri entry point
│   └── [frontend HTML/CSS/JS]    # Web UI
├── scripts/                      # Build automation
│   ├── build-all.sh              # Build all platforms
│   ├── build-linux.sh            # Linux builds (Docker)
│   └── build-ios-testflight.sh   # iOS TestFlight builds
├── tests/                        # Integration tests
│   ├── iroh_gossip_mesh_tests.rs # P2P mesh tests
│   └── test_p2p_desktop.sh       # Desktop P2P testing
├── Cargo.toml                    # Rust dependencies
├── tauri.conf.json               # Desktop config
├── tauri.android.conf.json       # Android config
├── tauri.ios.conf.json           # iOS config
└── BUILD_INSTRUCTIONS.md         # Platform build guide
```

---

## Development

### Running in Dev Mode

```bash
# Desktop with hot reload
cargo tauri dev

# Android (requires device or emulator)
cargo tauri android dev

# iOS (requires Xcode)
cargo tauri ios dev
```

### Building for Production

```bash
# Build all available platforms (macOS only)
./scripts/build-all.sh

# Build specific platform
cargo tauri build --target aarch64-apple-darwin  # macOS ARM64
cargo tauri build --target x86_64-apple-darwin   # macOS Intel
cargo tauri build --target x86_64-pc-windows-msvc # Windows
cargo tauri build --target x86_64-unknown-linux-gnu # Linux

# Android
env ANDROID_HOME=~/Library/Android/sdk \
  NDK_HOME=~/Library/Android/sdk/ndk \
  OPENSSL_STATIC=1 OPENSSL_VENDORED=1 \
  cargo tauri android build --target aarch64 --debug

# iOS
cargo tauri ios build --config tauri.ios.conf.json
```

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test iroh_gossip_mesh_tests

# P2P tests (requires 2+ instances)
./tests/test_p2p_desktop.sh

# Format and lint
cargo fmt
cargo clippy
```

### Code Style

- **Rust:** Follow official Rust style guide (enforced by `rustfmt`)
- **JavaScript:** Vanilla JS, no frameworks
- **Commits:** Descriptive messages following conventional commits
- **Documentation:** Inline comments for complex logic, README for architecture

---

## Building & Distribution

### Platform-Specific Requirements

**macOS:**
- Xcode Command Line Tools
- Apple Developer account (for code signing)

**Android:**
- Android Studio
- Android SDK (API 33+)
- NDK (latest)

**iOS:**
- Xcode
- Apple Developer account (required)
- Development Team ID in `tauri.ios.conf.json`
- Code signing certificate (`aps.cer` - see BUILD_INSTRUCTIONS.md)

**Linux:**
- GTK3 development libraries
- WebKit2GTK
- Docker (for cross-platform builds)

**Windows:**
- Visual Studio Build Tools
- Windows SDK

### Code Signing & Secrets

Cipher requires platform-specific configuration files that are **NOT committed to git**:

**iOS Code Signing:**
1. Download `aps.cer` from Apple Developer Portal
2. Place in project root: `aps.cer`
3. Update `tauri.ios.conf.json` with your Development Team ID

**Android Signing:**
- Debug builds: No configuration needed
- Release builds: Configure keystore in Android Studio

See [BUILD_INSTRUCTIONS.md](BUILD_INSTRUCTIONS.md) for complete setup instructions.

### Release Process

```bash
# 1. Update version in all configs
./scripts/update-release-links.sh 0.1.0

# 2. Build all platforms
./scripts/build-all.sh

# 3. Verify builds
ls -lh releases/*/latest/

# 4. Create git tag
git tag -a v0.1.0 -m "Release v0.1.0: [description]"

# 5. Push to GitHub
git push origin main --tags

# 6. Upload binaries to GitHub Releases
# (Binaries are NOT committed to git - only uploaded to GitHub Releases)
```

---

## Contributing

Contributions are welcome! Please follow these guidelines:

### How to Contribute

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes**
4. **Run tests**: `cargo test && cargo clippy`
5. **Commit**: `git commit -m "Add amazing feature"`
6. **Push**: `git push origin feature/amazing-feature`
7. **Open a Pull Request**

### Development Guidelines

- **Quality over speed** - Do it right, not fast
- **Fix root causes** - Never comment out failing tests
- **Test thoroughly** - P2P features require 2+ instances
- **Document changes** - Update README/docs as needed
- **Security first** - Never log private keys, review crypto changes carefully

### Testing Requirements

- All new features must include tests
- P2P features require integration tests
- Run `cargo fmt` and `cargo clippy` before committing
- Verify builds on target platforms when possible

### Pull Request Process

1. Update documentation if adding/changing features
2. Add tests for new functionality
3. Ensure all tests pass: `cargo test`
4. Update CHANGELOG.md with notable changes
5. PR will be reviewed for security and code quality

---

## Security

### Encryption Details

**Algorithms:**
- **Ed25519**: Digital signatures (RFC 8032)
- **X25519**: Elliptic Curve Diffie-Hellman (RFC 7748)
- **ChaCha20Poly1305**: Authenticated encryption (RFC 8439)
- **BIP39**: Recovery phrase generation and deterministic key derivation

**Key Management:**
1. On first run: App generates 24-word BIP39 recovery phrase for user
2. User provides only a display name (no password required)
3. Recovery phrase shown ONCE - user must save it securely
4. Recovery phrase derives deterministic Ed25519 signing keypair
5. Public key serves as identity, private key stored encrypted in SQLite
6. To restore account: User enters display name + 24-word phrase
7. Per-session ephemeral keys for forward secrecy (X25519)

**Message Encryption Flow:**
```rust
// Sender
let shared_secret = x25519_dh(sender_private, recipient_public);
let nonce = random_bytes(12);
let ciphertext = chacha20poly1305_encrypt(plaintext, shared_secret, nonce);
let signature = ed25519_sign(ciphertext, sender_private);

// Recipient
let shared_secret = x25519_dh(recipient_private, sender_public);
verify_signature(ciphertext, signature, sender_public);
let plaintext = chacha20poly1305_decrypt(ciphertext, shared_secret, nonce);
```

### Privacy Guarantees

**What Cipher Protects:**
- ✅ Message content (end-to-end encrypted)
- ✅ Message authenticity (digital signatures)
- ✅ Identity verification (public key cryptography)
- ✅ Metadata privacy (no server logs)
- ✅ Offline privacy (local-first storage)

**What Cipher Does NOT Protect:**
- ❌ Network metadata (IP addresses visible to peers)
- ❌ Timing analysis (message send times)
- ❌ Device compromise (malware, physical access)
- ❌ Social graph (peer relationships visible to network observers)

**Recommendations:**
- Use Tor/VPN for network anonymity
- Enable full-disk encryption on your device
- Store recovery phrase securely offline (never digitally)
- Keep software updated for security patches

### Reporting Security Issues

**DO NOT** open public GitHub issues for security vulnerabilities.

Instead, please email security concerns to: [hello@thingg.co](mailto:hello@thingg.co)

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if available)

We will respond within 48 hours and work with you to address the issue.

---

## Roadmap

### Current Status: v0.0.1 (Initial Release)

**Working Features:**
- ✅ End-to-end encrypted messaging
- ✅ P2P peer discovery and connection
- ✅ Local SQLite database
- ✅ Friend/peer management
- ✅ Message feed
- ✅ Cross-platform builds (macOS, Android, iOS)

**Known Issues:**
- Peer discovery can be slow on initial connection
- iOS builds require manual code signing setup
- UI polish needed (glassmorphism theme in progress)

### Planned Features (v0.1.0+)

**Short Term:**
- [ ] Group messaging support
- [ ] File/media sharing (encrypted)
- [ ] Offline message queue
- [ ] Push notifications (mobile)
- [ ] Profile customization

**Medium Term:**
- [ ] Voice/video calls (WebRTC)
- [ ] Desktop notifications
- [ ] Message search
- [ ] Data export/backup
- [ ] Multi-device sync

**Long Term:**
- [ ] Federated relays (user-operated)
- [ ] Plugin system
- [ ] Encrypted cloud backup (user-controlled)
- [ ] Bridge to other protocols (Matrix, XMPP)

### Community Wishlist

Vote on features or suggest new ones:
[GitHub Discussions](https://github.com/aosmith/cipher/discussions)

---

## License

This project is licensed under the **GNU Affero General Public License v3.0 (AGPL-3.0)** - see the [LICENSE.txt](LICENSE.txt) file for details.

```
Copyright (C) 2025 Cipher Contributors

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
```

The AGPL v3 license ensures that any modifications to Cipher, including those running as a network service, must be made available to users under the same open source terms.

---

## Acknowledgments

Cipher stands on the shoulders of giants:

- **[Iroh](https://iroh.computer/)** - Robust P2P networking library
- **[Tauri](https://tauri.app/)** - Lightweight cross-platform framework
- **[Rust Community](https://www.rust-lang.org/)** - Safe, performant systems programming
- **[NaCl/libsodium](https://libsodium.gitbook.io/)** - Cryptographic primitives inspiration
- **[Signal Protocol](https://signal.org/docs/)** - E2E encryption best practices
- **[Matrix](https://matrix.org/)** - Decentralized communication inspiration

### Dependencies

Key Rust crates used in this project:
- `tauri` - Application framework
- `iroh` - P2P networking
- `rusqlite` - SQLite database
- `ed25519-dalek` - Digital signatures
- `x25519-dalek` - Key exchange
- `chacha20poly1305` - Authenticated encryption
- `pbkdf2` - Key derivation
- `uuid` - Deterministic user IDs

---

## Links

- **Website:** [cipher.example](https://cipher.example) *(coming soon)*
- **GitHub:** [github.com/aosmith/cipher](https://github.com/aosmith/cipher)
- **Issues:** [github.com/aosmith/cipher/issues](https://github.com/aosmith/cipher/issues)
- **Discussions:** [github.com/aosmith/cipher/discussions](https://github.com/aosmith/cipher/discussions)
- **Documentation:** [docs/](docs/) *(coming soon)*

---

## FAQ

**Q: Is Cipher production-ready?**
A: No. Version 0.0.1 is an initial release. Use at your own risk. Security audit pending.

**Q: How does Cipher compare to Signal/WhatsApp?**
A: Cipher is fully peer-to-peer (no servers), local-first (you own your data), and open source. Signal/WhatsApp rely on centralized infrastructure.

**Q: Can I use Cipher over Tor?**
A: Yes, though Iroh's DHT discovery may be slower. Relay-based discovery works better over Tor.

**Q: Does Cipher phone home?**
A: No. Zero telemetry, zero analytics, zero external connections (except to peers).

**Q: How do I backup my data?**
A: Export your SQLite database: `cp cipher.db cipher-backup.db`. Keep it secure (contains encrypted keys).

**Q: Can I self-host a relay?**
A: Yes, though relay configuration is not yet documented. Coming in v0.1.0.

**Q: Is Cipher audited?**
A: Not yet. Independent security audit planned once codebase stabilizes.

---

**Built with ❤️ by the Cipher community. Privacy is a human right.**
