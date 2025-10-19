#!/bin/bash
set -e

echo "Building iOS Simulator app for Cipher..."

# Set environment variables
export IPHONEOS_DEPLOYMENT_TARGET=17.0
export SDKROOT=$(xcrun --sdk iphonesimulator --show-sdk-path)

# Clean previous builds
echo "Cleaning previous builds..."
cargo clean
rm -rf ~/Library/Developer/Xcode/DerivedData/cipher-social-*

# Build Rust library for iOS simulator
echo "Building Rust library for iOS simulator (arm64)..."
cargo build --target aarch64-apple-ios-sim --lib --release

# The Cargo `[lib]` name is `app`, so the static archive is `libapp.a`
LIB_PATH="target/aarch64-apple-ios-sim/release/libapp.a"

if [ -f "$LIB_PATH" ]; then
    echo "✅ Rust library built successfully!"

    # Copy library to the correct location for Xcode
    mkdir -p gen/apple/Externals/lib
    cp "$LIB_PATH" gen/apple/Externals/lib/libapp.a
    echo "✅ Library copied to Xcode project"
else
    echo "❌ Failed to build Rust library"
    exit 1
fi

# Build with Xcode
echo "Building iOS app with Xcode..."
cd gen/apple
xcodebuild -project cipher-social.xcodeproj \
           -scheme cipher-social_iOS \
           -sdk iphonesimulator \
           -configuration Release \
           -destination 'platform=iOS Simulator,name=iPhone 15 Pro,OS=17.0' \
           build

echo "✅ iOS app built successfully!"
# Copy the built simulator bundle to a stable location for automation
APP_BUNDLE=$(find ~/Library/Developer/Xcode/DerivedData -path '*cipher-social*/Build/Products/*-iphonesimulator/Cipher.app' -maxdepth 10 -type d -print -quit)
if [ -d "$APP_BUNDLE" ]; then
    DEST_DIR="target/ios-sim"
    DEST_APP="$DEST_DIR/Cipher.app"
    mkdir -p "$DEST_DIR"
    rm -rf "$DEST_APP"
    cp -R "$APP_BUNDLE" "$DEST_APP"
    echo "✅ Copied simulator bundle to $DEST_APP"
else
    echo "⚠️ Could not locate built Cipher.app in DerivedData"
fi
echo ""
echo "To run the app in the simulator:"
echo "1. Open Xcode: open gen/apple/cipher-social.xcodeproj"
echo "2. Select iPhone 15 Pro simulator"
echo "3. Press Cmd+R to run"
