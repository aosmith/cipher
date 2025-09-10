#!/bin/bash

# Development script for Cipher mobile applications
set -e

echo "🚀 Starting Cipher mobile development..."

# Function to start dev for specific platform
dev_platform() {
    local platform=$1
    echo "📱 Starting development for $platform..."
    
    case $platform in
        "android")
            echo "🤖 Starting Android development..."
            if [ -z "$ANDROID_HOME" ]; then
                echo "⚠️  Warning: ANDROID_HOME not set. Please install Android SDK first."
                echo "   Install Android Studio and set ANDROID_HOME environment variable."
                return 1
            fi
            
            # Check if emulator is running
            echo "📱 Checking for running Android emulator..."
            if ! adb devices | grep -q "emulator"; then
                echo "🚀 No emulator detected. Please start an Android emulator first:"
                echo "   • Open Android Studio"
                echo "   • Go to Tools → AVD Manager"
                echo "   • Start an emulator"
                echo ""
                echo "Or run from command line:"
                echo "   emulator -avd <your_avd_name>"
                echo ""
                read -p "Press Enter when emulator is running..."
            fi
            
            # Android app runs Rails internally - no external server needed
            echo "🚂 Mobile app will run Rails internally..."
            
            # Start Tauri Android dev with Android config (will install and run on emulator)
            echo "📱 Starting Android app on emulator..."
            cargo tauri android dev --config src-tauri/tauri.android.conf.json
            ;;
        "ios")
            echo "🍎 Starting iOS Simulator development..."
            if [[ "$OSTYPE" != "darwin"* ]]; then
                echo "❌ iOS development is only supported on macOS"
                return 1
            fi
            
            # Check if iOS Simulator is available
            echo "📱 Checking iOS Simulator availability..."
            if ! xcrun simctl list devices | grep -q "Booted"; then
                echo "🚀 No iOS Simulator running. Starting iPhone simulator..."
                echo "   This will open in iOS Simulator"
                # List available simulators
                echo "📱 Available simulators:"
                xcrun simctl list devices available | grep "iPhone" | head -5
            fi
            
            # iOS app runs Rails internally - no external server needed
            echo "🚂 Mobile app will run Rails internally..."
            
            # Start Tauri iOS dev with mobile config
            echo "📱 Starting iOS app in Simulator..."
            # Auto-select first available iPhone simulator to avoid interactive prompt
            DEVICE_NAME=$(xcrun simctl list devices | grep "iPhone.*Pro.*iOS" | head -1 | sed 's/^[[:space:]]*\([^(]*\).*/\1/' | xargs)
            if [ -z "$DEVICE_NAME" ]; then
                # Fallback to any iPhone
                DEVICE_NAME=$(xcrun simctl list devices | grep "iPhone.*iOS" | head -1 | sed 's/^[[:space:]]*\([^(]*\).*/\1/' | xargs)
            fi
            echo "📱 Selected device: $DEVICE_NAME"
            # Use iOS-specific config with internal Rails server
            cargo tauri ios dev "$DEVICE_NAME" --config src-tauri/tauri.ios.conf.json
            ;;
        *)
            echo "❌ Unknown platform: $platform"
            return 1
            ;;
    esac
}

# Default to Android if no platform specified
PLATFORM=${1:-android}

echo "📱 Starting development for $PLATFORM..."
dev_platform "$PLATFORM"

echo "✅ Mobile development session ended!"