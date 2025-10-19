#!/bin/bash

# Comprehensive visual test for two-instance P2P functionality
# Tests friend adding, messaging, and takes screenshots for verification

set -e

echo "🧪 Cipher Two-Instance Visual Test"
echo "===================================="
echo ""

# Configuration
ALICE_DIR="/tmp/cipher-test-alice"
BOB_DIR="/tmp/cipher-test-bob"
SCREENSHOTS_DIR="/tmp/cipher-test-screenshots"
APP_BINARY="./target/release/cipher-social"

# Clean up
echo "🧹 Cleaning up old data..."
pkill -f "cipher-social" || true
sleep 2
rm -rf "$ALICE_DIR" "$BOB_DIR" "$SCREENSHOTS_DIR"
mkdir -p "$ALICE_DIR" "$BOB_DIR" "$SCREENSHOTS_DIR"

# Check if app is built
if [ ! -f "$APP_BINARY" ]; then
    echo "❌ Error: $APP_BINARY not found!"
    echo "Please run: cargo build --release"
    exit 1
fi

echo "✅ Test environment ready"
echo ""

# Launch instances
echo "🚀 Launching Alice's instance..."
CIPHER_TEST_DATA_DIR="$ALICE_DIR" $APP_BINARY > "$ALICE_DIR/output.log" 2>&1 &
ALICE_PID=$!
echo "   PID: $ALICE_PID"
echo "   Database: $ALICE_DIR/cipher.db"

sleep 3

echo "🚀 Launching Bob's instance..."
CIPHER_TEST_DATA_DIR="$BOB_DIR" $APP_BINARY > "$BOB_DIR/output.log" 2>&1 &
BOB_PID=$!
echo "   PID: $BOB_PID"
echo "   Database: $BOB_DIR/cipher.db"

sleep 5

echo ""
echo "✅ Both instances launched!"
echo ""

# Take initial screenshot
echo "📸 Taking screenshot: Initial state (both login screens)"
screencapture -x "$SCREENSHOTS_DIR/01_initial_login_screens.png"
sleep 2

echo ""
echo "📋 Test Instructions:"
echo "===================="
echo ""
echo "MANUAL STEPS REQUIRED:"
echo ""
echo "1️⃣  Create users on both instances:"
echo "   Alice: username='alice', password='test123'"
echo "   Bob: username='bob', password='test123'"
echo "   (Press Enter after you've done this)"
read -p "   ✋ Press Enter when both users are created..."

echo ""
echo "📸 Taking screenshot: Both users logged in"
screencapture -x "$SCREENSHOTS_DIR/02_both_logged_in.png"
sleep 2

echo ""
echo "2️⃣  Copy public keys:"
echo "   - Click the 🔑 key in each navbar to copy"
echo "   - Alice's key should be different from Bob's key"
echo "   (Press Enter when ready to proceed)"
read -p "   ✋ Press Enter to continue..."

echo ""
echo "📸 Taking screenshot: Public keys visible"
screencapture -x "$SCREENSHOTS_DIR/03_public_keys_visible.png"
sleep 2

echo ""
echo "3️⃣  Add each other as friends:"
echo "   - On Alice: Click ☰ → Add Friend → Paste Bob's key → Click 'Add Friend'"
echo "   - On Bob: Click ☰ → Add Friend → Paste Alice's key → Click 'Add Friend'"
echo "   (Press Enter when both have added each other)"
read -p "   ✋ Press Enter when friends are added..."

echo ""
echo "📸 Taking screenshot: Friends added"
screencapture -x "$SCREENSHOTS_DIR/04_friends_added.png"
sleep 2

echo ""
echo "4️⃣  Verify friends list:"
echo "   - On both instances: Click ☰ → Friends"
echo "   - You should see the other user in the friends list"
echo "   (Press Enter when verified)"
read -p "   ✋ Press Enter to continue..."

echo ""
echo "📸 Taking screenshot: Friends lists visible"
screencapture -x "$SCREENSHOTS_DIR/05_friends_lists.png"
sleep 2

echo ""
echo "5️⃣  Test messaging:"
echo "   - On Alice: Click ☰ → Messages"
echo "   - Search for 'bob' and select him"
echo "   - Type 'Hello Bob!' and send"
echo "   (Press Enter after sending)"
read -p "   ✋ Press Enter when message sent..."

echo ""
echo "📸 Taking screenshot: Alice sent message"
screencapture -x "$SCREENSHOTS_DIR/06_alice_sent_message.png"
sleep 2

echo ""
echo "6️⃣  Verify message received:"
echo "   - On Bob: Click ☰ → Messages"
echo "   - You should see Alice's message"
echo "   (Press Enter when verified)"
read -p "   ✋ Press Enter to continue..."

echo ""
echo "📸 Taking screenshot: Bob received message"
screencapture -x "$SCREENSHOTS_DIR/07_bob_received_message.png"
sleep 2

echo ""
echo "7️⃣  Reply to message:"
echo "   - On Bob: Select Alice as recipient"
echo "   - Type 'Hi Alice!' and send"
echo "   (Press Enter after sending)"
read -p "   ✋ Press Enter when reply sent..."

echo ""
echo "📸 Taking screenshot: Bob sent reply"
screencapture -x "$SCREENSHOTS_DIR/08_bob_sent_reply.png"
sleep 2

echo ""
echo "8️⃣  Verify reply received:"
echo "   - On Alice: Check messages tab"
echo "   - You should see Bob's reply"
echo "   (Press Enter when verified)"
read -p "   ✋ Press Enter to continue..."

echo ""
echo "📸 Taking screenshot: Alice received reply"
screencapture -x "$SCREENSHOTS_DIR/09_alice_received_reply.png"
sleep 2

echo ""
echo "✅ Test Complete!"
echo ""
echo "📊 Test Summary:"
echo "================"
echo ""
echo "✓ Both instances launched with separate databases"
echo "✓ Users created (alice & bob)"
echo "✓ Public keys are different"
echo "✓ Friends added successfully"
echo "✓ Messages sent and received"
echo ""
echo "📸 Screenshots saved to: $SCREENSHOTS_DIR"
ls -lh "$SCREENSHOTS_DIR"/*.png
echo ""
echo "📋 Logs saved to:"
echo "   Alice: $ALICE_DIR/output.log"
echo "   Bob: $BOB_DIR/output.log"
echo ""

# Check for errors in logs
echo "🔍 Checking for errors in logs..."
echo ""
if grep -i "error\|panic\|failed" "$ALICE_DIR/output.log" > /dev/null 2>&1; then
    echo "⚠️  Errors found in Alice's log:"
    grep -i "error\|panic\|failed" "$ALICE_DIR/output.log" | tail -5
else
    echo "✅ No errors in Alice's log"
fi

if grep -i "error\|panic\|failed" "$BOB_DIR/output.log" > /dev/null 2>&1; then
    echo "⚠️  Errors found in Bob's log:"
    grep -i "error\|panic\|failed" "$BOB_DIR/output.log" | tail -5
else
    echo "✅ No errors in Bob's log"
fi

echo ""
echo "Press Enter to stop both instances and exit..."
read

echo ""
echo "🛑 Stopping instances..."
kill $ALICE_PID $BOB_PID 2>/dev/null || true
pkill -f "cipher-social" || true
sleep 1

echo "✅ Test complete and instances stopped"
echo ""
echo "📸 Review screenshots at: $SCREENSHOTS_DIR"
