# CLAUDE.md

## Project Overview

Cipher is a Tauri/Rust end-to-end encrypted peer-to-peer social network. Local-first architecture with no servers required.

**Stack:**
- Rust + Tauri framework
- SQLite database (local)
- Ed25519 + X25519 + ChaCha20Poly1305 encryption
- Iroh for P2P networking (DHT discovery, gossip protocol)
- Glassmorphism UI with dark theme

**Current Version**: 0.1.3

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

## Beta Testing Flow

**IMPORTANT: Always wipe data before installing during beta testing.**

**Android - Build and Deploy:**
```bash
# Step 1: Build
PATH="/bin:/usr/bin:$HOME/.cargo/bin:$PATH" \
JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" \
ANDROID_HOME=~/Library/Android/sdk \
NDK_HOME=~/Library/Android/sdk/ndk/26.1.10909125 \
OPENSSL_STATIC=1 OPENSSL_VENDORED=1 \
cargo tauri android build --target aarch64 --debug

# Step 2: Wipe app data (ALWAYS do this before install)
adb shell pm clear com.cipher.social

# Step 3: Install
adb install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

**macOS - Build and Deploy:**
```bash
# Step 1: Build
cargo tauri build --debug

# Step 2: Wipe app data (ALWAYS do this before launch)
rm -rf ~/Library/Application\ Support/com.cipher.social
rm -rf ~/Library/WebKit/com.cipher.social
rm -rf ~/Library/Caches/com.cipher.social

# Step 3: Launch
open target/debug/bundle/macos/Cipher.app
```

**Why always wipe?**
- Beta software has frequent schema changes
- Stale data causes hard-to-debug issues
- P2P keypairs must match fresh state
- WebView localStorage persists even after uninstall

**Full Rebuild (when code changes aren't picked up):**
```bash
# Clear ALL build caches first
rm -rf gen/android/app/build           # Gradle build cache
rm -rf target/aarch64-linux-android    # Rust target cache

# Then run normal build - should take ~50+ seconds
# If it finishes in <5 seconds, cache wasn't cleared
```

**IMPORTANT:** The `pm clear` command is essential for Android - it wipes WebView localStorage which stores the user session. Without it, the app will auto-login with stale cached credentials even after reinstall.

**Icon Management:**
```bash
# After updating icons/icon.png, regenerate Android launcher icons:
./scripts/generate-android-icons.sh

# To remove whitespace from icon:
magick icons/icon.png -trim icons/icon.png
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

- **ANDROID DATA WIPE** - ALWAYS use `adb shell pm clear com.cipher.social` to wipe Android data. NEVER rely on `adb uninstall` - it does NOT clear WebView localStorage. This is the #1 testing mistake.
- **Quality over speed** - do it right, not fast
- **Fix root causes** - never remove/comment out tests
- **Version management** - use sequential semver, update all references. Use `./scripts/update-release-links.sh <version>` to update ALL version files including iOS.
- **iOS version sync** - iOS versions are in `gen/apple/project.yml` and `gen/apple/cipher-social_iOS/Info.plist`. These MUST be kept in sync with tauri.conf.json. The update-release-links.sh script handles this automatically.
- **Release process** - When pushing to prod: 1) bump version in Cargo.toml AND tauri.conf.json, 2) rebuild ALL platforms (macOS, iOS, Android), 3) commit and push. Version bump MUST come first.
- **Build everything** - desktop AND mobile when releasing
- **Clean up** - kill processes, remove artifacts regularly
- **Foreground tasks** - run in foreground unless there's a reason
- **No extra work** - do what's asked, nothing more
- **P2P testing** - requires 2+ instances running simultaneously
- **Private key security** - never log or transmit over P2P
- **Wipe before reinstall** - always clear app data/database before reinstalling to avoid schema conflicts
- We want to implement as many signaling routes as possible, they should fail over.
- The startup log should include the current version
- you need to calculate coorindates, the screenshots have different coordinates.
- For the time being we should keep working a single commit on main and the 0.0.1 tag.