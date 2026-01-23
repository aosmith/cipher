# CLAUDE.md

## Project Overview

Cipher is a Tauri/Rust end-to-end encrypted peer-to-peer social network. Local-first architecture with no servers required.

**Stack:**
- Rust + Tauri framework
- SQLite database (local)
- Ed25519 + X25519 + ChaCha20Poly1305 encryption
- Iroh for P2P networking (DHT discovery, gossip protocol)
- Glassmorphism UI with dark theme

**Current Version**: 0.1.7

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

**IMPORTANT: Always wipe ALL devices before testing. When testing P2P sync between devices, BOTH macOS AND Android must be wiped together - never wipe just one.**

**Android - Build and Deploy:**
```bash
# Step 1: Build (run in foreground - background mode has PATH issues)
JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" \
ANDROID_HOME=~/Library/Android/sdk \
NDK_HOME=~/Library/Android/sdk/ndk/28.2.13676358 \
OPENSSL_STATIC=1 OPENSSL_VENDORED=1 \
cargo tauri android build --target aarch64 --debug

# Step 2: Wipe app data (ALWAYS do this before install)
# NOTE: Always use full path to adb - it's not in PATH
~/Library/Android/sdk/platform-tools/adb shell pm clear com.cipher.social

# Step 3: Install
~/Library/Android/sdk/platform-tools/adb install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk

# Step 4: Launch (optional)
~/Library/Android/sdk/platform-tools/adb shell am start -n com.cipher.social/.MainActivity
```

**macOS - Build and Deploy:**
```bash
# Step 1: Build
cargo tauri build --debug

# Step 2: Kill, wait, wipe ALL locations, THEN launch (must be sequential!)
# CRITICAL: All commands MUST be in a single chained command
pkill -9 Cipher cipher-social; sleep 2; \
rm -rf ~/Library/Application\ Support/com.cipher.social \
       ~/Library/Application\ Support/cipher-social \
       ~/Library/WebKit/com.cipher.social \
       ~/Library/WebKit/cipher-social \
       ~/Library/Caches/com.cipher.social \
       ~/Library/Caches/cipher-social \
       ~/Library/Saved\ Application\ State/com.cipher.social.savedState \
       ~/Library/Preferences/com.cipher.social.plist \
       ~/Library/Preferences/cipher-social.plist \
       ~/Library/HTTPStorages/com.cipher.social \
       ~/Library/HTTPStorages/cipher-social \
       ~/Library/HTTPStorages/com.cipher.social.binarycookies \
       ~/Library/HTTPStorages/cipher-social.binarycookies \
       /tmp/cipher*.log /tmp/cipher*.txt; \
defaults delete com.cipher.social 2>/dev/null; \
defaults delete cipher-social 2>/dev/null; \
open target/debug/bundle/macos/Cipher.app
```

**CRITICAL Wipe Rules:**
- **ALWAYS wipe BOTH platforms** when testing P2P features - never wipe just one
- Commands MUST be chained with `&&` or `;` in a single command
- Never launch the app in a separate tool call - it may start before wipe completes
- The `sleep 2` ensures pkill has fully terminated the process
- macOS stores data in MANY locations: Application Support, WebKit, Caches, Preferences, HTTPStorages, Saved Application State
- Both `com.cipher.social` AND `cipher-social` identifiers are used

**Combined Wipe and Deploy (use this for P2P testing):**
```bash
# Wipe and deploy BOTH platforms in one command
pkill -9 -f cipher; sleep 2; \
rm -rf ~/Library/Application\ Support/com.cipher.social ~/Library/WebKit/com.cipher.social ~/Library/Caches/com.cipher.social; \
~/Library/Android/sdk/platform-tools/adb shell pm clear com.cipher.social; \
~/Library/Android/sdk/platform-tools/adb install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk; \
/path/to/Cipher.app/Contents/MacOS/cipher-social > /tmp/cipher-logs.txt 2>&1 &
```

**Multi-Device Testing (P2P sync):**
```bash
# ALWAYS wipe BOTH devices together - never just one!
# Android:
adb shell pm clear com.cipher.social
# macOS: (run the full wipe command above)
```

**Why always wipe?**
- Beta software has frequent schema changes
- Stale data causes hard-to-debug issues
- P2P keypairs must match fresh state
- WebView localStorage persists even after uninstall
- **macOS stores data in MULTIPLE locations** (both `com.cipher.social` and `cipher-social`)
- **Running instances hold old data in memory** - must kill processes before wiping
- **Multi-device testing requires ALL devices wiped** - old data on one device breaks sync testing

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
- **ALL content must be encrypted** - posts, comments, reactions, messages all use SealedEnvelope (encrypted sealed boxes). No unencrypted content transport.

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
- **Reuse code** - everything should use the same code paths where possible. Posts, comments, reactions, messages should all use the same transport and sync mechanisms. Don't create separate code paths for similar functionality.
- **P2P testing** - requires 2+ instances running simultaneously
- **Private key security** - never log or transmit over P2P
- **Wipe before reinstall** - always clear app data/database before reinstalling to avoid schema conflicts
- We want to implement as many signaling routes as possible, they should fail over.
- The startup log should include the current version
- you need to calculate coorindates, the screenshots have different coordinates.
- For the time being we should keep working a single commit on main and the 0.0.1 tag.
## P2P Architecture Notes (for comment/reaction sync)

**Key Architecture Points:**

1. **Iroh is for handshake/hole-punching only** - message size is very limited
2. **Content delivery uses:**
   - `SealedEnvelope` - encrypted sealed boxes for each friend
   - `PostWithBlobs` - for posts when friends don't have encryption keys yet
   - iroh-blobs for large attachments

3. **Device sync (sync.rs) currently syncs:**
   - Posts
   - Messages  
   - Friends
   - **NOT comments or reactions!**

4. **Comments/reactions don't sync because they were never implemented for P2P**
   - The `PostComment` and `PostReaction` P2PMessage types exist but aren't used
   - Frontend event listeners exist but backend never sends these message types
   - My attempt to add `iroh_broadcast_post_comment` etc was wrong approach

**Correct Solution Options:**
1. Add comments/reactions to device sync mechanism
2. Create SealedEnvelope for comments/reactions (like posts do)
3. Use presence announcements to trigger sync requests

**Files involved:**
- `src/app/database/sync.rs` - SyncData struct, get_sync_data(), apply_sync_data()
- `src/app/iroh_commands.rs` - iroh_publish_post shows SealedEnvelope pattern
- `src/app/crypto/sealed_box.rs` - GossipEnvelope encryption
- `src/app/iroh_network.rs` - P2PMessage enum, handle_message() handlers
