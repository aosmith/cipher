#!/bin/bash

# Build Cipher for TestFlight
# Usage: ./scripts/build-testflight.sh

set -e

echo "=== Building Cipher for TestFlight ==="
echo ""

cd "$(dirname "$0")/.."

# Clean previous builds
echo "Step 1: Cleaning previous builds..."
rm -rf gen/apple/build
rm -rf gen/apple/Externals
rm -rf ~/Library/Developer/Xcode/DerivedData/cipher-social-*

# Build using Tauri
echo ""
echo "Step 2: Building iOS app (release)..."
echo "This may take several minutes..."
echo ""

cargo tauri ios build

# Find outputs
echo ""
echo "Step 3: Locating build outputs..."

ARCHIVE=$(find gen/apple/build -name "*.xcarchive" -type d 2>/dev/null | head -1)
IPA=$(find gen/apple/build -name "*.ipa" -type f 2>/dev/null | head -1)
APP=$(find gen/apple/build -name "Cipher.app" -type d 2>/dev/null | head -1)

echo ""
echo "=== Build Complete ==="
echo ""

if [ -n "$IPA" ]; then
    echo "IPA ready for upload: $IPA"
    echo ""
    echo "Upload options:"
    echo ""
    echo "  1. Transporter app (easiest):"
    echo "     - Open Transporter (free on Mac App Store)"
    echo "     - Drag and drop the IPA file"
    echo "     - Click 'Deliver'"
    echo ""
    echo "  2. Command line:"
    echo "     xcrun altool --upload-app -f \"$IPA\" -t ios -u YOUR_APPLE_ID -p @keychain:AC_PASSWORD"
    echo ""
elif [ -n "$ARCHIVE" ]; then
    echo "Archive created: $ARCHIVE"
    echo ""
    echo "To export and upload:"
    echo "  1. Open Xcode"
    echo "  2. Window > Organizer"
    echo "  3. Select archive, click 'Distribute App'"
    echo "  4. Choose 'App Store Connect' > 'Upload'"
    echo ""
elif [ -n "$APP" ]; then
    echo "App bundle created: $APP"
    echo ""
    echo "Note: This is a debug build. For TestFlight, run without --debug flag."
else
    echo "Build outputs:"
    find gen/apple/build -type f \( -name "*.app" -o -name "*.ipa" -o -name "*.xcarchive" \) 2>/dev/null || echo "  No outputs found"
fi

echo ""
echo "=== Done ==="
