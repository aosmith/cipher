#!/usr/bin/env bash
set -euo pipefail

echo "🖥️  Testing Cipher Desktop App (Native)"
echo "======================================="

# Kill any existing processes
echo "🧹 Cleaning up existing processes..."
pkill -f "Cipher" 2>/dev/null || true
lsof -ti tcp:3000 | xargs kill -9 2>/dev/null || true
sleep 2

# Build the app first if needed
if [[ ! -f "src-tauri/target/release/bundle/macos/Cipher.app/Contents/MacOS/cipher-desktop" ]]; then
    echo "📦 Building desktop app..."
    npm run tauri:build
fi

echo "🧪 Running native desktop app tests..."
cd src-tauri
cargo test desktop_app_native_test --release

echo "🎉 Native desktop app tests completed!"