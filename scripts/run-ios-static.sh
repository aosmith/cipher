#!/bin/bash

set -e

echo "🚀 Building Cipher for iOS (Static Mode)..."

# Set up environment
export TAURI_PLATFORM=ios
export TAURI_FAMILY=mobile

# Get list of available iOS simulators
echo "📱 Getting available iOS simulators..."
AVAILABLE_DEVICES=$(xcrun simctl list devices available | grep -E "iPhone.*iOS" | tail -5)
echo "Available devices:"
echo "$AVAILABLE_DEVICES"

# Auto-select iPhone 15 Pro if available, otherwise first iPhone
DEVICE_NAME=$(xcrun simctl list devices available | grep "iPhone 15 Pro iOS" | head -1 | sed 's/.*(\(.*\)).*/\1/')

if [ -z "$DEVICE_NAME" ]; then
    echo "iPhone 15 Pro not found, selecting first available iPhone..."
    DEVICE_NAME=$(xcrun simctl list devices available | grep -E "iPhone.*iOS" | head -1 | sed 's/.*(\(.*\)).*/\1/')
fi

echo "📱 Selected device: $DEVICE_NAME"

# Ensure iOS assets are compiled
echo "📦 Compiling iOS assets..."
RAILS_ENV=ios rails assets:precompile

echo "🏗️ Building iOS app..."
# Use build mode with iOS configuration
cargo tauri ios build --config src-tauri/tauri.ios.conf.json --debug

echo "📱 Installing and running app on simulator..."
# Install the built app
xcrun simctl install "$DEVICE_NAME" "$(find ./src-tauri/gen/apple/target/debug -name "*.app" | head -1)"

# Get bundle ID and launch
BUNDLE_ID="com.cipher.social"
xcrun simctl launch "$DEVICE_NAME" "$BUNDLE_ID"

echo "✅ Cipher iOS app launched successfully"
echo "📱 App is running on device: $DEVICE_NAME"