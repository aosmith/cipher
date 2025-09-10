#!/bin/bash

# Build script for Cipher mobile applications
set -e

echo "🚀 Building Cipher mobile applications..."

# Function to build for specific platform
build_platform() {
    local platform=$1
    echo "📱 Building for $platform..."
    
    case $platform in
        "android")
            echo "🤖 Building Android APK for emulator and sideloading..."
            # Note: Requires Android SDK to be installed and ANDROID_HOME set
            if [ -z "$ANDROID_HOME" ]; then
                echo "⚠️  Warning: ANDROID_HOME not set. Please install Android SDK first."
                echo "   You can install it via Android Studio or the command line tools."
                return 1
            fi
            
            echo "📱 Building APK for Android emulator..."
            cargo tauri android build --apk
            
            echo "📦 APK built for:"
            echo "   • Android emulator testing"  
            echo "   • Sideloading on real devices"
            echo "   • Direct installation (requires 'Unknown Sources' enabled)"
            ;;
        "ios")
            echo "🍎 Building iOS app for Simulator..."
            # Note: Requires Xcode on macOS
            if [[ "$OSTYPE" != "darwin"* ]]; then
                echo "❌ iOS builds are only supported on macOS"
                return 1
            fi
            
            if ! command -v xcodegen &> /dev/null; then
                echo "❌ xcodegen is required but not installed. Installing..."
                brew install xcodegen
            fi
            
            echo "📱 Building for iOS Simulator (no code signing required)..."
            # Build for simulator to avoid code signing issues
            # Use aarch64-sim for Apple Silicon Macs, x86_64 for Intel Macs
            if [[ $(uname -m) == "arm64" ]]; then
                echo "🍎 Building for Apple Silicon iOS Simulator..."
                cargo tauri ios build --target aarch64-sim
            else
                echo "💻 Building for Intel iOS Simulator..."
                cargo tauri ios build --target x86_64
            fi
            
            echo "📦 iOS app built for:"
            echo "   • iOS Simulator testing"
            echo "   • Xcode development workflow"
            echo "   • No code signing required"
            ;;
        *)
            echo "❌ Unknown platform: $platform"
            return 1
            ;;
    esac
}

# Compile Rails assets first
echo "🎨 Compiling Rails assets..."
RAILS_ENV=production rails assets:precompile

# Build requested platforms
if [ $# -eq 0 ]; then
    echo "📱 Building for all available platforms..."
    
    # Build Android if SDK is available
    if [ -n "$ANDROID_HOME" ]; then
        build_platform android
    else
        echo "⚠️  Skipping Android build - ANDROID_HOME not set"
    fi
    
    # Build iOS if on macOS
    if [[ "$OSTYPE" == "darwin"* ]]; then
        build_platform ios
    else
        echo "⚠️  Skipping iOS build - not on macOS"
    fi
else
    # Build specific platforms
    for platform in "$@"; do
        build_platform "$platform"
    done
fi

echo "✅ Mobile build process completed!"