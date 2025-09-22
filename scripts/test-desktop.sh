#!/usr/bin/env bash
set -euo pipefail

echo "🖥️  Testing Cipher Desktop App"
echo "================================"

# Build the desktop app if it doesn't exist or is older than source
if [[ ! -f "src-tauri/target/release/bundle/macos/Cipher.app/Contents/MacOS/cipher-desktop" ]] || [[ "src-tauri/src/main.rs" -nt "src-tauri/target/release/bundle/macos/Cipher.app/Contents/MacOS/cipher-desktop" ]]; then
    echo "📦 Building desktop app..."
    ./scripts/build-desktop.sh
fi

# Kill any existing processes on port 3000
echo "🧹 Cleaning up existing processes..."
pkill -f "Cipher" 2>/dev/null || true
lsof -ti tcp:3000 | xargs kill -9 2>/dev/null || true
sleep 2

# Start the desktop app in the background
echo "🚀 Starting desktop app..."
open src-tauri/target/release/bundle/macos/Cipher.app

# Wait for the app to start
echo "⏳ Waiting for desktop app to be ready..."
sleep 5

# Wait for the app to be responding on port 3000
attempts=0
while ! curl -s -f http://127.0.0.1:3000 >/dev/null 2>&1; do
    sleep 1
    attempts=$((attempts + 1))
    if (( attempts > 30 )); then
        echo "❌ Desktop app failed to start within 30 seconds"
        exit 1
    fi
done

echo "✅ Desktop app is ready!"

# Run the desktop tests
echo "🧪 Running desktop app tests..."
DESKTOP_TEST=true bin/rails test test/system/desktop_app_test.rb

# Cleanup
echo "🧹 Cleaning up..."
pkill -f "Cipher" 2>/dev/null || true

echo "🎉 Desktop app tests completed!"