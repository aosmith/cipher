#!/bin/bash

# Build available platforms for Cipher release
# This script builds platforms that can be compiled on the current macOS system
# For Windows and Linux, consider using GitHub Actions or Docker

set -e

# Disable ANSI color codes that break terminal
export CARGO_TERM_COLOR=never
export NO_COLOR=1

VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)
echo "Building Cipher v${VERSION} for available platforms..."
echo ""

# Detect system architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    ARCH="aarch64"
fi

# Clean up any running processes
echo "Cleaning up running processes..."
pkill -f "cargo tauri" 2>/dev/null || true
pkill -f "Cipher" 2>/dev/null || true

# Track successful builds
BUILDS_COMPLETED=""

# Build native macOS (current architecture)
echo "================================"
echo "Building macOS (native $ARCH)..."
echo "================================"
if cargo tauri build --target ${ARCH}-apple-darwin; then
    BUILDS_COMPLETED="${BUILDS_COMPLETED}\n  ✅ macOS ($ARCH)"
else
    echo "⚠️  macOS native build failed"
fi

# Build macOS for other architecture if requested
if [ "$1" = "--cross-macos" ]; then
    if [ "$ARCH" = "aarch64" ]; then
        OTHER_ARCH="x86_64"
    else
        OTHER_ARCH="aarch64"
    fi

    echo ""
    echo "================================"
    echo "Building macOS ($OTHER_ARCH)..."
    echo "================================"
    if cargo tauri build --target ${OTHER_ARCH}-apple-darwin; then
        BUILDS_COMPLETED="${BUILDS_COMPLETED}\n  ✅ macOS ($OTHER_ARCH)"
    else
        echo "⚠️  macOS $OTHER_ARCH build failed"
    fi
fi

# Build Android
echo ""
echo "================================"
echo "Building Android APK..."
echo "================================"
export ANDROID_HOME=/Users/alex/Library/Android/sdk
export NDK_HOME=/Users/alex/Library/Android/sdk/ndk
export OPENSSL_STATIC=1
export OPENSSL_VENDORED=1

if cargo tauri android build --target aarch64; then
    BUILDS_COMPLETED="${BUILDS_COMPLETED}\n  ✅ Android (aarch64)"
else
    echo "⚠️  Android build failed"
fi

# Build iOS (if configured)
echo ""
echo "================================"
echo "Building iOS..."
echo "================================"
if cargo tauri ios build --config tauri.ios.conf.json; then
    BUILDS_COMPLETED="${BUILDS_COMPLETED}\n  ✅ iOS"
else
    echo "⚠️  iOS build skipped (not configured or failed)"
fi

# Windows and Linux notice
echo ""
echo "================================"
echo "Platform Limitations"
echo "================================"
echo "❌ Windows: Cannot build locally on macOS"
echo "   - Requires Windows SDK headers not available on macOS"
echo "   - Recommendation: Use a Windows VM or cloud build service"
echo ""
echo "❌ Linux: Cannot build locally on macOS"
echo "   - Requires Linux system libraries and pkg-config setup"
echo "   - Recommendation: Use a Linux VM or Docker container"

# Copy builds to releases directory
echo ""
echo "================================"
echo "Copying builds to releases..."
echo "================================"

mkdir -p releases/macos
mkdir -p releases/android
mkdir -p releases/ios

# Copy macOS builds
for arch_dir in target/*/release/bundle/dmg; do
    if ls $arch_dir/*.dmg 1> /dev/null 2>&1; then
        arch_name=$(basename $(dirname $(dirname $(dirname $arch_dir))))
        cp $arch_dir/*.dmg "releases/macos/Cipher-${VERSION}-${arch_name}.dmg"
        echo "  Copied macOS $arch_name build"
    fi
done

# Copy Android build
if [ -f "gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk" ]; then
    cp "gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk" "releases/android/Cipher-${VERSION}.apk"
    echo "  Copied Android build"
fi

# Copy iOS build if it exists
if ls target/*/release/bundle/ios/*.ipa 1> /dev/null 2>&1; then
    cp target/*/release/bundle/ios/*.ipa "releases/ios/Cipher-${VERSION}.ipa"
    echo "  Copied iOS build"
fi

echo ""
echo "================================"
echo "Build Summary for v${VERSION}"
echo "================================"
echo "Completed builds:"
echo -e "$BUILDS_COMPLETED"
echo ""
echo "Available releases:"
ls -lh releases/macos/*.dmg 2>/dev/null || echo "  No macOS releases"
ls -lh releases/android/*.apk 2>/dev/null || echo "  No Android releases"
ls -lh releases/ios/*.ipa 2>/dev/null || echo "  No iOS releases"

echo ""
echo "For Windows and Linux builds:"
echo "  1. Windows: Set up a Windows VM or use a cloud service"
echo "  2. Linux: Use Docker or a Linux VM"
echo "  3. Alternative: Ask collaborators on those platforms to build"

echo ""
echo "Next steps:"
echo "  1. Test the available builds"
echo "  2. Commit and tag: git tag v${VERSION}"
echo "  3. Push: git push --tags"