# CLAUDE.md

## Project Overview

Cipher is a Tauri/Rust end-to-end encrypted peer-to-peer social network. Local-first architecture with no servers required.

**Stack:**
- Rust + Tauri framework
- SQLite database (local)
- Ed25519 + X25519 + ChaCha20Poly1305 encryption
- Iroh for P2P networking (DHT discovery, gossip protocol)
- Glassmorphism UI with dark theme

**Current Version**: 0.22.0

## Key Commands

**Development:**
```bash
cargo tauri dev                    # Run with hot reload
cargo test                         # Run tests
cargo fmt && cargo clippy          # Format and lint
```

**Building:**
```bash
./scripts/build-all.sh                                                    # All platforms
cargo tauri build                                                         # Desktop
env ANDROID_HOME=~/Library/Android/sdk NDK_HOME=~/Library/Android/sdk/ndk \
  OPENSSL_STATIC=1 OPENSSL_VENDORED=1 cargo tauri android build \
  --target aarch64 --debug                                                # Android
cargo tauri ios build                                                     # iOS
```

**Android Installation:**
```bash
adb install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

**Release Management:**
```bash
./scripts/update-release-links.sh [version]    # Update version numbers
./scripts/build-all.sh                         # Build everything
./scripts/build-experimental.sh                # Build for GitHub Pages
```

## Architecture

**File Structure:**
- `src/` - Frontend (HTML/CSS/JS) and Rust backend
- `src/app/` - Core Rust modules (database, crypto, P2P)
- `gen/` - Generated iOS/Android projects
- `target/` - Build artifacts
- `releases/` - Distribution binaries

**Security Model:**
- Private keys derived from recovery phrase (BIP39 mnemonic)
- Keys stored in local SQLite (user's device only)
- Public keys are primary identifier (prevents username collisions)
- Private keys NEVER transmitted over P2P channels
- Only public keys shared between peers

**P2P Networking (Iroh):**
- DHT + DNS + Relay discovery for peer finding
- Gossip protocol for message propagation
- Router accepts incoming connections and routes to protocol handlers
- Deterministic user_id generation (UUID v5 from public key)

## Important Rules

- **Quality over speed** - do it right, not fast
- **Fix root causes** - never remove/comment out tests
- **Version management** - use sequential semver, update all references
- **Build everything** - desktop AND mobile when releasing
- **Clean up** - kill processes, remove artifacts regularly
- **Foreground tasks** - run in foreground unless there's a reason
- **No extra work** - do what's asked, nothing more
- **P2P testing** - requires 2+ instances running simultaneously
- **Private key security** - never log or transmit over P2P
- wipe the devices cache and local storage before reinstalling.
- We want to implement as many signaling routes as possible, they should fail over.
- The startup log should include the current version
- you need to calculate coorindates, the screenshots have different coordinates.
- For the time being we should keep working a single commit on main and the 0.0.1 tag.