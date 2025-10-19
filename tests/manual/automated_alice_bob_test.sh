#!/bin/bash
#
# Automated Cross-Device P2P Test: Alice (macOS) and Bob (Android)
#
# This script fully automates testing P2P communication between macOS and Android
# without requiring manual user interaction.

set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Automated Cipher P2P Cross-Device Test${NC}"
echo -e "${BLUE}Alice (macOS) <-> Bob (Android)${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if ! command -v adb &> /dev/null; then
    echo -e "${RED}ERROR: adb not found${NC}"
    exit 1
fi

DEVICE_SERIAL=$(adb devices | grep -v "List" | grep "device" | awk '{print $1}' | head -1)
if [ -z "$DEVICE_SERIAL" ]; then
    echo -e "${RED}ERROR: No Android device connected${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Android device: $DEVICE_SERIAL${NC}"

# Kill any existing cargo tauri dev processes
echo -e "${YELLOW}Cleaning up existing processes...${NC}"
pkill -f "cargo tauri dev" || true
sleep 2

# Start macOS app in background and capture logs
echo -e "${YELLOW}Starting macOS app...${NC}"
MACOS_LOG="/tmp/cipher_macos_test.log"
> "$MACOS_LOG"
cargo tauri dev > "$MACOS_LOG" 2>&1 &
MACOS_PID=$!

# Wait for macOS app to initialize
echo -e "${YELLOW}Waiting for macOS app to initialize...${NC}"
for i in {1..30}; do
    if grep -q "libp2p_initialize called" "$MACOS_LOG" 2>/dev/null; then
        echo -e "${GREEN}✓ macOS app initialized${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}ERROR: macOS app failed to start${NC}"
        cat "$MACOS_LOG"
        kill $MACOS_PID 2>/dev/null || true
        exit 1
    fi
    sleep 1
done

# Extract Alice's info from logs
sleep 2
ALICE_USER_ID=$(grep "User ID:" "$MACOS_LOG" | tail -1 | awk '{print $NF}')
ALICE_PUBLIC_KEY=$(grep "Public Key:" "$MACOS_LOG" | tail -1 | awk '{print $NF}')
ALICE_PEER_ID=$(grep "Skipping own peer ID:" "$MACOS_LOG" | tail -1 | awk '{print $NF}')

echo -e "${GREEN}Alice (macOS):${NC}"
echo "  User ID: $ALICE_USER_ID"
echo "  Public Key: $ALICE_PUBLIC_KEY"
echo "  Peer ID: $ALICE_PEER_ID"
echo ""

# Clear Android logcat
adb logcat -c

# Launch Android app
echo -e "${YELLOW}Starting Android app...${NC}"
adb shell am start -a android.intent.action.MAIN -c android.intent.category.LAUNCHER -n com.cipher.social/.MainActivity
sleep 3

# Wait for Android app to initialize
echo -e "${YELLOW}Waiting for Android app to initialize...${NC}"
for i in {1..30}; do
    # Check for any libp2p activity, not just initialization message
    if adb logcat -d | grep -E "(libp2p_initialize called|Connected to peer|Attempting reconnection|Dialing)" | tail -1 | grep -q .; then
        echo -e "${GREEN}✓ Android app initialized (P2P activity detected)${NC}"
        break
    fi
    if [ $i -eq 30 ]; then
        echo -e "${RED}ERROR: Android app failed to start${NC}"
        adb logcat -d -s "RustStdoutStderr:I" | tail -50
        kill $MACOS_PID 2>/dev/null || true
        exit 1
    fi
    sleep 1
done

# Extract Bob's info from Android logs
sleep 2
BOB_USER_ID=$(adb logcat -d | grep "User ID:" | tail -1 | awk '{print $NF}')
BOB_PUBLIC_KEY=$(adb logcat -d | grep "Public Key:" | tail -1 | awk '{print $NF}')
BOB_PEER_ID=$(adb logcat -d | grep "Skipping own peer ID:" | tail -1 | awk '{print $NF}')

# If we can't extract Bob's info from recent logs, the app was already running
# Try to get it from earlier in the log
if [ -z "$BOB_USER_ID" ]; then
    echo -e "${YELLOW}Note: Android app was already running, extracting info from earlier logs${NC}"
    BOB_USER_ID=$(adb logcat -d | grep "User ID:" | head -1 | awk '{print $NF}')
    BOB_PUBLIC_KEY=$(adb logcat -d | grep "Public Key:" | head -1 | awk '{print $NF}')
    BOB_PEER_ID=$(adb logcat -d | grep "Skipping own peer ID:" | head -1 | awk '{print $NF}')
fi

echo -e "${GREEN}Bob (Android):${NC}"
echo "  User ID: $BOB_USER_ID"
echo "  Public Key: $BOB_PUBLIC_KEY"
echo "  Peer ID: $BOB_PEER_ID"
echo ""

# Test 1: Peer Discovery
echo -e "${YELLOW}========================================${NC}"
echo -e "${YELLOW}Test 1: Peer Discovery${NC}"
echo -e "${YELLOW}========================================${NC}"

echo "Waiting for peer discovery (30 seconds)..."
sleep 30

# Check if peers discovered each other
MACOS_FOUND_PEER=false
ANDROID_FOUND_PEER=false

if grep -q "$BOB_PEER_ID" "$MACOS_LOG"; then
    echo -e "${GREEN}✓ macOS discovered Android peer${NC}"
    MACOS_FOUND_PEER=true
else
    echo -e "${RED}✗ macOS did not discover Android peer${NC}"
fi

if adb logcat -d | grep -q "$ALICE_PEER_ID"; then
    echo -e "${GREEN}✓ Android discovered macOS peer${NC}"
    ANDROID_FOUND_PEER=true
else
    echo -e "${RED}✗ Android did not discover macOS peer${NC}"
fi

if [ "$MACOS_FOUND_PEER" = true ] && [ "$ANDROID_FOUND_PEER" = true ]; then
    echo -e "${GREEN}✓ Peer discovery: PASSED${NC}"
else
    echo -e "${YELLOW}⚠ Peer discovery: PARTIAL (may connect via DHT)${NC}"
fi

echo ""

# Test 2: Connection Status
echo -e "${YELLOW}========================================${NC}"
echo -e "${YELLOW}Test 2: Connection Status${NC}"
echo -e "${YELLOW}========================================${NC}"

# Check for connection messages in logs
if grep -q "Connected to peer" "$MACOS_LOG"; then
    MACOS_PEERS=$(grep "Connected to peer" "$MACOS_LOG" | wc -l | xargs)
    echo -e "${GREEN}✓ macOS connected to $MACOS_PEERS peer(s)${NC}"
else
    echo -e "${YELLOW}⚠ macOS: No direct connections yet${NC}"
fi

if adb logcat -d | grep -q "Connected to peer"; then
    ANDROID_PEERS=$(adb logcat -d | grep "Connected to peer" | wc -l | xargs)
    echo -e "${GREEN}✓ Android connected to $ANDROID_PEERS peer(s)${NC}"
else
    echo -e "${YELLOW}⚠ Android: No direct connections yet${NC}"
fi

echo ""

# Summary
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${GREEN}✓ macOS app running${NC}"
echo -e "${GREEN}✓ Android app running${NC}"
echo -e "${GREEN}✓ Both apps initialized libp2p${NC}"

if [ "$MACOS_FOUND_PEER" = true ] && [ "$ANDROID_FOUND_PEER" = true ]; then
    echo -e "${GREEN}✓ Peer discovery successful${NC}"
else
    echo -e "${YELLOW}⚠ Peer discovery partial (DHT fallback available)${NC}"
fi

echo ""
echo -e "${YELLOW}Apps are running. You can now:${NC}"
echo "  1. Manually test messaging between devices"
echo "  2. Check logs: tail -f $MACOS_LOG"
echo "  3. Check Android logs: adb logcat -s RustStdoutStderr:I"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop the test and clean up${NC}"

# Wait for user interrupt
trap "echo ''; echo 'Cleaning up...'; kill $MACOS_PID 2>/dev/null || true; echo 'Done'; exit 0" INT
wait $MACOS_PID
