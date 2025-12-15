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

# Write cargo path to MULTIPLE locations to ensure build phase can find it
# -hideShellScriptEnvironment blocks PATH, so we write absolute paths
CARGO_PATH=$(which cargo)
CARGO_DIR=$(dirname "$CARGO_PATH")

# Location 1: /tmp (traditional)
echo "$CARGO_PATH" > /tmp/cargo_path.txt
echo "✓ Written to /tmp/cargo_path.txt"

# Location 2: Workspace root (Xcode Cloud specific)
if [ -n "$CI_WORKSPACE" ]; then
    echo "$CARGO_PATH" > "$CI_WORKSPACE/cargo_path.txt"
    echo "✓ Written to $CI_WORKSPACE/cargo_path.txt"
fi

# Location 3: Inside the project directory (accessible via SRCROOT)
echo "$CARGO_PATH" > "gen/apple/cargo_path.txt"
echo "✓ Written to gen/apple/cargo_path.txt"

# Location 4: Repository root
echo "$CARGO_PATH" > "cargo_path.txt"
echo "✓ Written to ./cargo_path.txt (repo root)"

echo ""
echo "Cargo path: $CARGO_PATH"
echo "Cargo directory: $CARGO_DIR"
echo ""
echo "=== Build environment ready ==="
