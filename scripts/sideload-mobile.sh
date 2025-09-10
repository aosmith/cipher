#!/bin/bash

# Sideloading script for Cipher mobile applications
set -e

echo "📱 Cipher Mobile Sideloading Helper"

# Function to sideload to specific platform
sideload_platform() {
    local platform=$1
    echo "📱 Preparing sideload for $platform..."
    
    case $platform in
        "android")
            echo "🤖 Android Sideloading Instructions:"
            echo ""
            echo "📋 Prerequisites:"
            echo "   1. Enable 'Developer Options' on your Android device"
            echo "   2. Enable 'USB Debugging' in Developer Options"
            echo "   3. Enable 'Install unknown apps' for your file manager/browser"
            echo ""
            
            # Build APK
            if [ -z "$ANDROID_HOME" ]; then
                echo "❌ ANDROID_HOME not set. Please install Android SDK first."
                return 1
            fi
            
            echo "🔨 Building APK..."
            cargo tauri android build --apk
            
            # Find the built APK
            APK_PATH=$(find src-tauri/gen/android -name "*.apk" | head -1)
            if [ -z "$APK_PATH" ]; then
                echo "❌ APK not found. Build may have failed."
                return 1
            fi
            
            echo "✅ APK built: $APK_PATH"
            echo ""
            echo "📱 Sideload Options:"
            echo ""
            echo "Option 1 - USB Installation (ADB):"
            echo "   1. Connect your Android device via USB"
            echo "   2. Run: adb install \"$APK_PATH\""
            echo ""
            echo "Option 2 - Manual Installation:"
            echo "   1. Copy APK to your device: $APK_PATH"
            echo "   2. Open file manager on device"
            echo "   3. Tap the APK file and install"
            echo ""
            echo "Option 3 - Wireless Transfer:"
            echo "   1. Email the APK to yourself"
            echo "   2. Download on device and install"
            echo ""
            
            # Check if device is connected
            if command -v adb &> /dev/null && adb devices | grep -q "device"; then
                echo "🔗 Android device detected!"
                read -p "Install automatically via ADB? (y/n): " -n 1 -r
                echo
                if [[ $REPLY =~ ^[Yy]$ ]]; then
                    echo "📱 Installing on device..."
                    adb install "$APK_PATH"
                    echo "✅ Installation complete!"
                fi
            fi
            ;;
        "ios")
            echo "🍎 iOS Sideloading Instructions:"
            echo ""
            echo "📋 Prerequisites:"
            echo "   • macOS with Xcode installed"
            echo "   • iOS device connected via USB"
            echo "   • Apple ID (free developer account works)"
            echo ""
            
            echo "🔨 Building for device..."
            # Build for device (will require signing)
            cargo tauri ios build --target aarch64
            
            echo "📱 Sideload Options:"
            echo ""
            echo "Option 1 - Xcode (Recommended):"
            echo "   1. Open src-tauri/gen/apple/cipher-desktop.xcodeproj in Xcode"
            echo "   2. Connect your iOS device"
            echo "   3. Select your device as target"
            echo "   4. Sign with your Apple ID (Xcode → Preferences → Accounts)"
            echo "   5. Click 'Run' button to install and launch"
            echo ""
            echo "Option 2 - AltStore/Sideloadly:"
            echo "   1. Install AltStore on your device"
            echo "   2. Use AltStore to install the .ipa file"
            echo "   3. Refresh weekly to prevent expiration"
            echo ""
            echo "Option 3 - TestFlight (Future):"
            echo "   • TestFlight distribution coming soon"
            echo "   • Will allow easy installation without sideloading"
            echo ""
            
            # Try to open in Xcode
            XCODE_PROJECT="src-tauri/gen/apple/cipher-desktop.xcodeproj"
            if [ -d "$XCODE_PROJECT" ]; then
                read -p "Open project in Xcode now? (y/n): " -n 1 -r
                echo
                if [[ $REPLY =~ ^[Yy]$ ]]; then
                    open "$XCODE_PROJECT"
                fi
            fi
            ;;
        *)
            echo "❌ Unknown platform: $platform"
            return 1
            ;;
    esac
}

# Security warning
echo "🔒 Security Notice:"
echo "   • Only sideload from trusted sources"
echo "   • This APK/app is built from source code"
echo "   • Review code at: https://github.com/aosmith/cipher"
echo ""

# Build requested platforms
if [ $# -eq 0 ]; then
    echo "📱 Available platforms: android, ios"
    echo "Usage: $0 <platform>"
    echo "Example: $0 android"
    exit 1
else
    for platform in "$@"; do
        sideload_platform "$platform"
        echo ""
    done
fi

echo "✅ Sideloading instructions provided!"
echo ""
echo "📚 Troubleshooting:"
echo "   • Android: Ensure 'Unknown Sources' is enabled"
echo "   • iOS: Trust the developer profile in Settings → General → VPN & Device Management" 
echo "   • Both: Check device storage and restart if needed"