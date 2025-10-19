#!/bin/bash

# Build iOS for TestFlight distribution
# Usage: ./scripts/build-ios-testflight.sh
# Note: Requires Xcode and iOS development tools installed

set -e

VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)
echo "Building Cipher iOS v${VERSION} for TestFlight..."

# Initialize iOS project if it doesn't exist
if [ ! -d "gen/apple" ]; then
    echo "Initializing iOS project..."
    cargo tauri ios init
else
    echo "iOS project already initialized"
fi

# Build iOS for release
echo ""
echo "Building iOS release..."
cargo tauri ios build --config tauri.ios.conf.json

# Create output directory
mkdir -p releases/ios/latest

# Clean up old builds (binaries NOT committed to git)
echo "Cleaning old iOS builds..."
rm -f releases/ios/latest/*.ipa 2>/dev/null || true

# Copy IPA to releases directory
if ls gen/apple/build/Build/Products/Release-iphoneos/*.ipa 1> /dev/null 2>&1; then
    cp gen/apple/build/Build/Products/Release-iphoneos/*.ipa "releases/ios/latest/Cipher.ipa"
    echo "✅ iOS IPA copied to releases/ios/latest/Cipher.ipa"

    # Generate SHA256 checksum
    cd releases/ios/latest
    shasum -a 256 Cipher.ipa > checksums.txt
    cd ../../..
    echo "✅ Checksums generated"
else
    echo "❌ iOS IPA not found"
    exit 1
fi

echo ""
echo "📊 iOS Build Summary for v${VERSION}:"
echo "================================"
ls -lh releases/ios/latest/*.ipa 2>/dev/null
echo ""
echo "SHA256 Checksums:"
cat releases/ios/latest/checksums.txt 2>/dev/null

echo ""
echo "✅ iOS build complete!"
echo "   Next steps:"
echo "   1. Test the build: open gen/apple/cipher-social.xcodeproj"
echo "   2. Upload to TestFlight: use Xcode Organizer or fastlane"
echo "   3. Upload binaries to GitHub Releases (binaries NOT committed to git)"
