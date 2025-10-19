#!/bin/bash

# Build all locally-available platforms for Cipher release
# Usage: ./scripts/build-all.sh
# Note: This builds platforms available on macOS. For Windows/Linux, use platform-specific machines.

set -e

# Disable ANSI color codes that break terminal
export CARGO_TERM_COLOR=never
export NO_COLOR=1

VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)
echo "Building Cipher v${VERSION} for locally-available platforms..."
echo "Note: This script builds macOS, Android, and iOS from macOS."
echo "      Windows and Linux require their native platforms."
echo ""

# Clean up any running processes
echo "Cleaning up running processes..."
pkill -f "cargo tauri" 2>/dev/null || true
pkill -f "Cipher" 2>/dev/null || true

# Build macOS (Apple Silicon)
echo ""
echo "Building macOS (Apple Silicon)..."
cargo tauri build --target aarch64-apple-darwin

# Build macOS (Intel) - cross-compilation on Apple Silicon
echo ""
echo "Building macOS (Intel)..."
cargo tauri build --target x86_64-apple-darwin

# Build Android
echo ""
echo "Building Android APK..."
export ANDROID_HOME=/Users/alex/Library/Android/sdk
export NDK_HOME=/Users/alex/Library/Android/sdk/ndk
export OPENSSL_STATIC=1
export OPENSSL_VENDORED=1
cargo tauri android build --target aarch64

# Build iOS (if configured)
echo ""
echo "Building iOS..."
# Initialize iOS project if it doesn't exist
if [ ! -d "gen/apple" ]; then
    echo "Initializing iOS project..."
    cargo tauri ios init || echo "iOS init failed"
fi
cargo tauri ios build --config tauri.ios.conf.json || echo "iOS build skipped (not configured)"

# Note about other platforms
echo ""
echo "================================"
echo "Platform Build Status:"
echo "================================"
echo "✅ macOS (ARM64 & x86_64) - Built locally"
echo "✅ Android - Built locally"
echo "✅ iOS - Built locally (if configured)"
echo "❌ Windows - Requires Windows machine or VM"
echo "❌ Linux - Requires Linux machine or Docker"

# Copy builds to releases directory
echo ""
echo "📦 Copying builds to releases directory..."

# Create directory structure
mkdir -p releases/macos/latest
mkdir -p releases/android/latest
mkdir -p releases/ios/latest
mkdir -p releases/windows/latest
mkdir -p releases/linux/latest

# Clean old release binaries (binaries are NOT committed to git)
echo "🧹 Cleaning old release binaries..."
rm -f releases/macos/latest/*.dmg 2>/dev/null || true
rm -f releases/android/latest/*.apk 2>/dev/null || true
rm -f releases/ios/latest/*.ipa 2>/dev/null || true

# Copy macOS build (use ARM64 as default)
if ls target/aarch64-apple-darwin/release/bundle/dmg/*.dmg 1> /dev/null 2>&1; then
    cp target/aarch64-apple-darwin/release/bundle/dmg/*.dmg "releases/macos/latest/Cipher.dmg"
    echo "✅ macOS ARM64 DMG copied to releases/macos/latest/Cipher.dmg"
    # Generate SHA256 checksum
    cd releases/macos/latest
    shasum -a 256 Cipher.dmg > checksums.txt
    cd ../../..
elif ls target/x86_64-apple-darwin/release/bundle/dmg/*.dmg 1> /dev/null 2>&1; then
    cp target/x86_64-apple-darwin/release/bundle/dmg/*.dmg "releases/macos/latest/Cipher.dmg"
    echo "✅ macOS x64 DMG copied to releases/macos/latest/Cipher.dmg"
    # Generate SHA256 checksum
    cd releases/macos/latest
    shasum -a 256 Cipher.dmg > checksums.txt
    cd ../../..
fi

# Copy Android build
if [ -f "gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk" ]; then
    cp "gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk" "releases/android/latest/Cipher.apk"
    echo "✅ Android APK copied to releases/android/latest/Cipher.apk"
    # Generate SHA256 checksum
    cd releases/android/latest
    shasum -a 256 Cipher.apk > checksums.txt
    cd ../../..
fi

# Copy iOS build (if exists)
if ls target/aarch64-apple-ios/release/bundle/ios/*.ipa 1> /dev/null 2>&1; then
    cp target/aarch64-apple-ios/release/bundle/ios/*.ipa "releases/ios/latest/Cipher.ipa"
    echo "✅ iOS IPA copied to releases/ios/latest/Cipher.ipa"
    # Generate SHA256 checksum
    cd releases/ios/latest
    shasum -a 256 Cipher.ipa > checksums.txt
    cd ../../..
fi

echo ""
echo "📊 Build Summary for v${VERSION}:"
echo "================================"
ls -lh releases/macos/latest/*.dmg 2>/dev/null || echo "❌ No macOS builds found"
ls -lh releases/android/latest/*.apk 2>/dev/null || echo "❌ No Android builds found"
ls -lh releases/ios/latest/*.ipa 2>/dev/null || echo "❌ No iOS builds found"
echo ""
echo "SHA256 Checksums:"
cat releases/macos/latest/checksums.txt 2>/dev/null || true
cat releases/android/latest/checksums.txt 2>/dev/null || true
cat releases/ios/latest/checksums.txt 2>/dev/null || true

echo ""
echo "🧹 Cleaning up build artifacts..."
# Run full cleanup (saves ~30GB+)
cargo clean 2>/dev/null || true
rm -rf gen 2>/dev/null || true

echo "   Cleaned target/ and gen/ directories"
echo "   Release binaries preserved in releases/"

echo ""
echo "✅ Build complete! Next steps:"
echo "   1. Test the builds in releases/"
echo "   2. Upload binaries to GitHub Releases (binaries NOT committed to git)"
echo "   3. Tag: git tag v${VERSION}"
echo "   4. Push: git push --tags"