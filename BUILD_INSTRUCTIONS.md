# Build Instructions for Cipher

Cipher is a cross-platform P2P social network. Since it's a local-first application with no external dependencies, builds must be created on their native platforms or using appropriate cross-compilation setups.

## Current Setup (macOS)

From macOS, you can build:
- ✅ **macOS** (both Apple Silicon and Intel)
- ✅ **Android** APK
- ✅ **iOS** (if configured with Apple Developer account)

From macOS, you **cannot** directly build:
- ❌ **Windows** - Requires Windows SDK headers
- ❌ **Linux** - Requires Linux system libraries

## Required Configuration & Secrets

Cipher requires some platform-specific configuration files that are **not committed to git** for security reasons:

### iOS Code Signing (Required for iOS builds)
1. **Apple Developer Certificate** (`aps.cer`):
   - Download from Apple Developer Portal
   - Place in project root: `aps.cer`
   - Already gitignored - will not be committed

2. **Development Team ID**:
   - Update `tauri.ios.conf.json`: Set `"developmentTeam"` to your Team ID
   - Find your Team ID at: https://developer.apple.com/account

### Android Signing (Optional - for release builds)
- For debug builds: No configuration needed
- For release builds: Configure keystore in Android Studio

### Local Development Files (Gitignored)
These directories are automatically created during development and **should not** be committed:
- `/uploads/` - User uploaded media files
- `/coverage/` - Test coverage reports
- `/tests/artifacts/` - Test screenshots and recordings
- `/.env*` - Environment configuration files

All of these are already in `.gitignore` and will not be tracked by git.

## Building on macOS

### Prerequisites
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Tauri CLI
cargo install tauri-cli

# For Android builds
# Install Android Studio and SDK from https://developer.android.com/studio
# Set environment variables in ~/.zshrc or ~/.bashrc:
export ANDROID_HOME=~/Library/Android/sdk
export NDK_HOME=$ANDROID_HOME/ndk
```

### Build Commands

```bash
# Build all available platforms (macOS, Android, iOS)
./scripts/build-all.sh

# Build only platforms that work on current system
./scripts/build-available.sh

# Build specific platform
cargo tauri build --target aarch64-apple-darwin  # macOS ARM64
cargo tauri build --target x86_64-apple-darwin   # macOS Intel
cargo tauri android build --target aarch64       # Android
cargo tauri ios build                           # iOS
```

## Building Windows Version

Since Windows builds require Windows SDK headers, you need a Windows machine:

### Option 1: Windows Machine or VM
1. Install Windows 10/11 (VM or physical machine)
2. Install prerequisites:
   ```powershell
   # Install Rust
   # Download from https://rustup.rs

   # Install Visual Studio Build Tools
   # Download from https://visualstudio.microsoft.com/downloads/
   # Select "Desktop development with C++"

   # Install Tauri dependencies
   cargo install tauri-cli
   ```
3. Clone and build:
   ```powershell
   git clone https://github.com/yourusername/cipher.git
   cd cipher
   cargo tauri build
   ```

### Option 2: Ask a Collaborator
Since this is a P2P app with no external dependencies, you can ask someone with Windows to:
1. Clone the repository
2. Run `cargo tauri build`
3. Share the resulting `.exe` or `.msi` installer

## Building Linux Version

Linux builds require GTK and other system libraries:

### Option 1: Linux Machine or VM
1. Install Linux (Ubuntu/Debian recommended)
2. Install prerequisites:
   ```bash
   # Install system dependencies
   sudo apt update
   sudo apt install libgtk-3-dev libwebkit2gtk-4.0-dev \
       libappindicator3-dev librsvg2-dev patchelf

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Install Tauri CLI
   cargo install tauri-cli
   ```
3. Clone and build:
   ```bash
   git clone https://github.com/yourusername/cipher.git
   cd cipher
   cargo tauri build
   ```

### Option 2: Docker (Experimental)
```bash
# Create a Docker container with Linux build environment
docker run -it -v $(pwd):/cipher ubuntu:22.04
# Inside container, install dependencies and build
```

### Option 3: Ask a Collaborator
Similar to Windows, ask someone with Linux to build and share the `.AppImage` or `.deb` file.

## Release Strategy

Since we want to keep things simple and P2P-focused:

1. **Primary Platforms** (built locally on macOS):
   - macOS (Universal Binary)
   - Android APK
   - iOS (TestFlight)

2. **Secondary Platforms** (community builds):
   - Windows: Request builds from Windows users
   - Linux: Request builds from Linux users

3. **Distribution**:
   - GitHub Releases for all platforms
   - Direct P2P sharing of binaries
   - No dependency on app stores (except iOS TestFlight)

## Automated Builds (Optional)

If you want automated builds without external dependencies, consider:

1. **Local Build Farm**: Set up VMs locally for each platform
2. **Friend's Machines**: Ask trusted friends to run build scripts
3. **Minimal CI**: Use GitHub Actions only for building, not hosting

## Quick Build Script

For the platforms you can build locally:

```bash
# This builds what's available on your current machine
./scripts/build-available.sh

# Results will be in:
# - releases/macos/   - macOS builds
# - releases/android/ - Android APK
# - releases/ios/     - iOS IPA (if configured)
```

## Notes

- All builds are self-contained with no external dependencies
- Each platform binary includes everything needed to run
- Users control their own data - no servers required
- P2P connectivity works across all platforms