#!/bin/bash

# P2P Post Synchronization Test - Alice & Bob on Two Android Emulators
# Tests: Alice creates post -> Bob sees it, Bob creates post -> Alice sees it

set -e

ALICE_DEVICE="emulator-5554"
BOB_DEVICE="emulator-5556"
ANDROID_HOME=/Users/alex/Library/Android/sdk
ADB=/opt/homebrew/bin/adb

echo "========================================"
echo "P2P POST SYNC TEST: Alice & Bob"
echo "========================================"
echo ""

# Clear app data for fresh start
echo "1. Clearing app data on both devices..."
$ADB -s $ALICE_DEVICE shell pm clear com.cipher.social
$ADB -s $BOB_DEVICE shell pm clear com.cipher.social
echo "   ✓ App data cleared"
echo ""

# Launch apps
echo "2. Launching Cipher on both devices..."
$ADB -s $ALICE_DEVICE shell am start -n com.cipher.social/.MainActivity
sleep 2
$ADB -s $BOB_DEVICE shell am start -n com.cipher.social/.MainActivity
sleep 3
echo "   ✓ Apps launched"
echo ""

# Take screenshots of initial state
echo "3. Capturing initial state..."
$ADB -s $ALICE_DEVICE shell screencap -p /sdcard/alice_01_start.png
$ADB -s $ALICE_DEVICE pull /sdcard/alice_01_start.png /tmp/alice_01_start.png 2>/dev/null
$ADB -s $BOB_DEVICE shell screencap -p /sdcard/bob_01_start.png
$ADB -s $BOB_DEVICE pull /sdcard/bob_01_start.png /tmp/bob_01_start.png 2>/dev/null
echo "   ✓ Initial screenshots saved to /tmp/"
echo ""

# Get screen dimensions
echo "4. Getting screen dimensions..."
SCREEN_SIZE=$($ADB -s $ALICE_DEVICE shell wm size | grep -o '[0-9]*x[0-9]*$')
WIDTH=$(echo $SCREEN_SIZE | cut -d'x' -f1)
HEIGHT=$(echo $SCREEN_SIZE | cut -d'x' -f2)
CENTER_X=$((WIDTH / 2))
echo "   Screen: ${WIDTH}x${HEIGHT}, Center X: ${CENTER_X}"
echo ""

# Alice signs up
echo "5. Alice signing up..."
# Tap username field (approximately 1/3 down the screen)
$ADB -s $ALICE_DEVICE shell input tap $CENTER_X $((HEIGHT / 3))
sleep 1
$ADB -s $ALICE_DEVICE shell input text "alice-p2p"
sleep 1

# Tab to password field
$ADB -s $ALICE_DEVICE shell input keyevent KEYCODE_TAB
sleep 1
$ADB -s $ALICE_DEVICE shell input text "password123"
sleep 1

# Screenshot before signin
$ADB -s $ALICE_DEVICE shell screencap -p /sdcard/alice_02_ready.png
$ADB -s $ALICE_DEVICE pull /sdcard/alice_02_ready.png /tmp/alice_02_ready.png 2>/dev/null

# Tap sign in button (approximately 2/3 down the screen)
$ADB -s $ALICE_DEVICE shell input tap $CENTER_X $((HEIGHT * 2 / 3))
sleep 3
echo "   ✓ Alice signed in"

$ADB -s $ALICE_DEVICE shell screencap -p /sdcard/alice_03_signedin.png
$ADB -s $ALICE_DEVICE pull /sdcard/alice_03_signedin.png /tmp/alice_03_signedin.png 2>/dev/null
echo ""

# Bob signs up
echo "6. Bob signing up..."
$ADB -s $BOB_DEVICE shell input tap $CENTER_X $((HEIGHT / 3))
sleep 1
$ADB -s $BOB_DEVICE shell input text "bob-p2p"
sleep 1
$ADB -s $BOB_DEVICE shell input keyevent KEYCODE_TAB
sleep 1
$ADB -s $BOB_DEVICE shell input text "password123"
sleep 1

$ADB -s $BOB_DEVICE shell screencap -p /sdcard/bob_02_ready.png
$ADB -s $BOB_DEVICE pull /sdcard/bob_02_ready.png /tmp/bob_02_ready.png 2>/dev/null

$ADB -s $BOB_DEVICE shell input tap $CENTER_X $((HEIGHT * 2 / 3))
sleep 3
echo "   ✓ Bob signed in"

$ADB -s $BOB_DEVICE shell screencap -p /sdcard/bob_03_signedin.png
$ADB -s $BOB_DEVICE pull /sdcard/bob_03_signedin.png /tmp/bob_03_signedin.png 2>/dev/null
echo ""

# Wait for P2P network to stabilize
echo "7. Waiting for P2P network to discover peers..."
sleep 10
echo "   ✓ P2P network ready"
echo ""

# Get libp2p logs to find peer IDs
echo "8. Checking P2P peer discovery..."
echo "   Alice's logs:"
$ADB -s $ALICE_DEVICE logcat -d -s "RustStdoutStderr:I" | grep -i "peer id\|listening\|connected" | tail -5 || true
echo ""
echo "   Bob's logs:"
$ADB -s $BOB_DEVICE logcat -d -s "RustStdoutStderr:I" | grep -i "peer id\|listening\|connected" | tail -5 || true
echo ""

# TODO: Extract public keys and add as friends
# For now, this would require implementing friend invite functionality via adb

echo "9. Alice creating a post..."
# This would require navigating to the create post UI
# For now, we'll document the manual steps needed
echo "   ⚠️  Manual step required: Create post via UI"
echo "   - Tap 'New Post' button"
echo "   - Type: 'Hello from Alice!'"
echo "   - Submit post"
echo ""

echo "10. Checking if Bob sees Alice's post..."
echo "   ⚠️  Manual verification required"
echo "   - Check Bob's feed for Alice's post"
echo ""

echo "11. Bob creating a post..."
echo "   ⚠️  Manual step required: Create post via UI"
echo ""

echo "12. Checking if Alice sees Bob's post..."
echo "   ⚠️  Manual verification required"
echo ""

# Final screenshots
echo "13. Capturing final state..."
$ADB -s $ALICE_DEVICE shell screencap -p /sdcard/alice_99_final.png
$ADB -s $ALICE_DEVICE pull /sdcard/alice_99_final.png /tmp/alice_99_final.png 2>/dev/null
$ADB -s $BOB_DEVICE shell screencap -p /sdcard/bob_99_final.png
$ADB -s $BOB_DEVICE pull /sdcard/bob_99_final.png /tmp/bob_99_final.png 2>/dev/null
echo "   ✓ Final screenshots saved to /tmp/"
echo ""

echo "========================================"
echo "TEST SUMMARY"
echo "========================================"
echo "Screenshots saved to:"
echo "  Alice: /tmp/alice_*.png"
echo "  Bob:   /tmp/bob_*.png"
echo ""
echo "Next steps:"
echo "  1. Both users are signed in"
echo "  2. P2P network is active"
echo "  3. Need to add each other as friends (friend invite feature)"
echo "  4. Create posts and verify sync"
echo ""
echo "To view logs:"
echo "  Alice: adb -s $ALICE_DEVICE logcat -s 'RustStdoutStderr:I'"
echo "  Bob:   adb -s $BOB_DEVICE logcat -s 'RustStdoutStderr:I'"
echo "========================================"
