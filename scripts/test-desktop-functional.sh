#!/usr/bin/env bash
set -euo pipefail

echo "🖥️  Testing Cipher Desktop App (Functional)"
echo "==========================================="

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

echo "🚀 Starting desktop app..."
open "src-tauri/target/release/bundle/macos/Cipher.app"

# Wait for app to start
echo "⏳ Waiting for desktop app to be ready..."
sleep 10

# Test that the server is responding
echo "🧪 Testing desktop app endpoints..."

# Test 1: Home page loads
echo "  📋 Testing home page..."
if curl -s -f http://127.0.0.1:3000 >/dev/null; then
    echo "  ✅ Home page loads"
else
    echo "  ❌ Home page failed"
    exit 1
fi

# Test 2: Account creation page loads
echo "  📋 Testing account creation page..."
if curl -s -f http://127.0.0.1:3000/users/new >/dev/null; then
    echo "  ✅ Account creation page loads"
else
    echo "  ❌ Account creation page failed"
    exit 1
fi

# Test 3: Can create an account via API
echo "  📋 Testing account creation..."
response=$(curl -s -X POST http://127.0.0.1:3000/users \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "user[username]=testuser&user[display_name]=Test User&password=securepass123&confirm_password=securepass123")

if echo "$response" | grep -q "Welcome to Cipher\|testuser\|Test User"; then
    echo "  ✅ Account creation works"
else
    echo "  ❌ Account creation failed"
    echo "Response: $response"
    exit 1
fi

# Cleanup
echo "🧹 Cleaning up..."
pkill -f "Cipher" 2>/dev/null || true

echo "🎉 Desktop app functional tests passed!"