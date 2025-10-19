#!/bin/bash

# Test script to run two instances of Cipher for P2P testing
# This allows testing friend connections, messaging, and posts between two users

set -e

echo "🧪 Starting Two-Instance P2P Test for Cipher"
echo "=============================================="
echo ""

# Create temporary data directories
ALICE_DIR="/tmp/cipher-test-alice"
BOB_DIR="/tmp/cipher-test-bob"

# Clean up old test data
echo "🧹 Cleaning up old test data..."
pkill -f "cipher-social" || true
sleep 2
rm -rf "$ALICE_DIR" "$BOB_DIR"
mkdir -p "$ALICE_DIR" "$BOB_DIR"

echo "✅ Created test directories:"
echo "   Alice: $ALICE_DIR"
echo "   Bob:   $BOB_DIR"
echo ""

# Check if app is built
if [ ! -f "target/release/cipher-social" ]; then
    echo "📦 Building app in release mode..."
    cargo build --release
fi

echo "🚀 Launching two instances..."
echo ""
echo "📱 Alice's instance (data in $ALICE_DIR)"
CIPHER_TEST_DATA_DIR="$ALICE_DIR" ./target/release/cipher-social &
ALICE_PID=$!

sleep 3

echo "📱 Bob's instance (data in $BOB_DIR)"
CIPHER_TEST_DATA_DIR="$BOB_DIR" ./target/release/cipher-social &
BOB_PID=$!

echo ""
echo "✅ Both instances launched!"
echo ""
echo "📋 Test Instructions:"
echo "===================="
echo ""
echo "1. Create users:"
echo "   - Alice: username=alice, password=test123"
echo "   - Bob: username=bob, password=test123"
echo ""
echo "2. Copy public keys:"
echo "   - Click on the 🔑 key in the navbar to copy"
echo "   - Or copy from the dashboard identity section"
echo ""
echo "3. Add as friends:"
echo "   - Click hamburger menu (☰) → Add Friend"
echo "   - Paste friend's public key"
echo "   - Click 'Add Friend'"
echo ""
echo "4. Test messaging:"
echo "   - Go to Messages tab"
echo "   - Select friend from search"
echo "   - Send encrypted messages back and forth"
echo ""
echo "5. Test posts:"
echo "   - Create posts on both instances"
echo "   - Verify posts appear in feed"
echo ""
echo "💡 Tips:"
echo "   - Arrange windows side by side for easier testing"
echo "   - Theme toggle works on both instances"
echo "   - P2P status shows in navbar when connected"
echo ""
echo "Press Enter to stop both instances..."
read

echo "🛑 Stopping instances..."
kill $ALICE_PID $BOB_PID 2>/dev/null || true
pkill -f "cipher-social" || true
echo "✅ Both instances stopped"
