#!/bin/bash

# Xcode Cloud pre-build script
# This runs before xcodebuild - most setup is done in ci_post_clone.sh

set -e

echo "=== Xcode Cloud Pre-Build: Verify Build Environment ==="
echo "Working directory: $(pwd)"
echo "User: $(whoami)"
echo "HOME: $HOME"
echo "CI_WORKSPACE: ${CI_WORKSPACE:-not set}"
echo "CI_PRIMARY_REPOSITORY_PATH: ${CI_PRIMARY_REPOSITORY_PATH:-not set}"

# Source cargo environment
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# Add cargo to PATH for this script
export PATH="$HOME/.cargo/bin:$PATH"

# Verify tools are installed
echo "=== Verifying Tools ==="
if ! command -v cargo &> /dev/null; then
    echo "ERROR: cargo not found in PATH"
    echo "PATH: $PATH"
    echo "HOME: $HOME"
    ls -la "$HOME/.cargo/bin" || echo "$HOME/.cargo/bin does not exist"
    exit 1
fi

if ! command -v cargo-tauri &> /dev/null; then
    echo "ERROR: cargo-tauri not found in PATH"
    echo "PATH: $PATH"
    exit 1
fi

echo "✓ Rust version: $(rustc --version)"
echo "✓ Cargo version: $(cargo --version)"
echo "✓ Cargo location: $(which cargo)"
echo "✓ Tauri CLI version: $(cargo tauri --version)"
echo "✓ Tauri CLI location: $(which cargo-tauri)"

# Verify iOS project exists
if [ ! -d "gen/apple/cipher-social.xcodeproj" ]; then
    echo "ERROR: gen/apple/cipher-social.xcodeproj not found"
    ls -la gen/apple/ || echo "gen/apple directory does not exist"
    exit 1
fi

echo "✓ iOS project found at gen/apple/cipher-social.xcodeproj"

# Write cargo path to /tmp for build phase to read
# -hideShellScriptEnvironment blocks PATH, so we pass it via file
CARGO_PATH=$(which cargo)
echo "$CARGO_PATH" > /tmp/cargo_path.txt
echo "✓ Cargo path written to /tmp/cargo_path.txt: $CARGO_PATH"

# Pre-build the Rust library for iOS
# This ensures libapp.a exists BEFORE xcodebuild runs, working around
# the -hideShellScriptEnvironment issue that strips PATH from build phases
echo ""
echo "=== Pre-building Rust library for iOS ==="

# Build frontend first (required by Tauri)
if [ -f "package.json" ]; then
    echo "Building frontend..."
    npm ci
    npm run build
fi

# Build the Rust iOS library
echo "Building Rust library for aarch64-apple-ios..."
cargo build --release --target aarch64-apple-ios

# Copy libapp.a to where Xcode expects it
mkdir -p gen/apple/Externals/arm64/release
cp target/aarch64-apple-ios/release/libapp.a gen/apple/Externals/arm64/release/
echo "✓ libapp.a copied to gen/apple/Externals/arm64/release/"

ls -lh gen/apple/Externals/arm64/release/libapp.a

echo ""
echo "=== Build environment ready ==="
