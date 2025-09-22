#!/usr/bin/env bash
set -euo pipefail

echo "📱 Testing Cipher Android App"
echo "=============================="

# Export required environment variables
export ANDROID_HOME=~/Library/Android/sdk
export NDK_HOME=$ANDROID_HOME/ndk

# Check if Android device/emulator is connected
echo "🔍 Checking for connected Android devices..."
devices=$($ANDROID_HOME/platform-tools/adb devices | grep "device$" | wc -l)
if [ $devices -eq 0 ]; then
    echo "❌ No Android device or emulator found!"
    echo "Please connect a device via USB or start an emulator"
    exit 1
elif [ $devices -gt 1 ]; then
    echo "📱 Multiple devices found, using physical device"
    export ANDROID_SERIAL="47111FDAQ00558"
else
    echo "✅ Android device found"
fi

# Build the Android app in debug mode if needed
echo "📦 Building Android app for testing..."
if [[ ! -f "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk" ]]; then
    echo "Building debug APK..."
    OPENSSL_STATIC=1 OPENSSL_VENDORED=1 npx tauri android build --target aarch64 --debug
fi

# Install the app on the device
echo "📲 Installing app on device..."
$ANDROID_HOME/platform-tools/adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk

# Wait for installation
sleep 2

# Run the Espresso tests
echo "🧪 Running Android Espresso tests..."
cd src-tauri/gen/android
./gradlew connectedAndroidTest

echo "🎉 Android tests completed!"