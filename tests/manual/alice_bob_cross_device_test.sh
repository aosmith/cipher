#!/bin/bash
#
# Manual Cross-Device P2P Test: Alice and Bob
#
# This script helps test P2P communication between two real devices:
# - Alice on macOS Desktop
# - Bob on Android (Pixel 9)
#
# Prerequisites:
# - macOS app built and running (cargo tauri dev)
# - Android APK installed on Pixel 9
# - Both devices on same local network (for mDNS) OR RSA enabled (for internet DHT)
# - ADB connected to Android device

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Cipher P2P Cross-Device Test${NC}"
echo -e "${BLUE}Alice (macOS) <-> Bob (Android)${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"

if ! command -v adb &> /dev/null; then
    echo -e "${RED}ERROR: adb not found. Please install Android SDK Platform Tools.${NC}"
    exit 1
fi

# Check if device is connected
DEVICE_SERIAL=$(adb devices | grep -v "List" | grep "device" | awk '{print $1}' | head -1)
if [ -z "$DEVICE_SERIAL" ]; then
    echo -e "${RED}ERROR: No Android device connected via ADB.${NC}"
    echo "Please connect your Pixel 9 and try again."
    exit 1
fi

echo -e "${GREEN}✓ ADB connected to device: $DEVICE_SERIAL${NC}"

# Check if macOS app is running
if ! pgrep -f "cipher-social" > /dev/null; then
    echo -e "${YELLOW}WARNING: macOS app (cipher-social) not running.${NC}"
    echo "Please start it with: cargo tauri dev"
    echo ""
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test Plan${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "This test will verify:"
echo "  1. Peer discovery (mDNS + DHT)"
echo "  2. Direct message exchange"
echo "  3. Post publishing and syncing"
echo "  4. Connection resilience"
echo ""

echo -e "${YELLOW}Step 1: Setup Users${NC}"
echo "On macOS app:"
echo "  - Create or login as 'alice'"
echo "  - Note Alice's public key/invite code"
echo ""
echo "On Android app:"
echo "  - Create or login as 'bob'"
echo "  - Note Bob's public key/invite code"
echo ""
read -p "Press Enter when both users are created and logged in..."

echo ""
echo -e "${YELLOW}Step 2: Exchange Friend Codes${NC}"
echo "Exchange public keys/invite codes between Alice and Bob"
echo "  - Alice: Add Bob as friend using his public key"
echo "  - Bob: Add Alice as friend using her public key"
echo ""
read -p "Press Enter when friend requests are sent..."

echo ""
echo -e "${YELLOW}Step 3: Monitor Peer Discovery${NC}"
echo "Starting log monitors for both devices..."
echo ""

# Function to monitor macOS logs
monitor_macos() {
    echo -e "${BLUE}=== macOS (Alice) Logs ===${NC}"
    # Monitor most recent cargo tauri dev output
    # Look for peer discovery and connection messages
    tail -n 50 /tmp/cipher_macos.log 2>/dev/null || echo "No macOS logs found. Check cargo tauri dev output."
}

# Function to monitor Android logs
monitor_android() {
    echo -e "${BLUE}=== Android (Bob) Logs ===${NC}"
    adb logcat -d -s "RustStdoutStderr:I" "chromium:E" "*:E" 2>/dev/null | tail -50 || echo "No Android logs available"
}

# Show current state
monitor_macos
echo ""
monitor_android

echo ""
echo -e "${YELLOW}Step 4: Verify Peer Discovery${NC}"
echo "Check the logs above for:"
echo "  - 'Connected to peer' messages"
echo "  - 'Found N Cipher peers via DHT'"
echo "  - mDNS peer discovery events"
echo ""
read -p "Did both devices discover each other? (y/n): " discovered

if [ "$discovered" != "y" ]; then
    echo -e "${RED}Peer discovery failed!${NC}"
    echo ""
    echo "Troubleshooting:"
    echo "  - Ensure both devices are on same WiFi (for mDNS)"
    echo "  - Check that RSA feature is enabled (for internet DHT)"
    echo "  - Verify firewall settings allow UDP/TCP on ports 4001, etc."
    echo "  - Check libp2p initialization in logs"
    exit 1
fi

echo -e "${GREEN}✓ Peer discovery successful${NC}"

echo ""
echo -e "${YELLOW}Step 5: Test Direct Messaging${NC}"
echo "Alice: Send a message to Bob: 'Hello Bob from Alice!'"
echo "Bob: Check if message appears in inbox"
echo ""
read -p "Did Bob receive Alice's message? (y/n): " received

if [ "$received" != "y" ]; then
    echo -e "${RED}Message delivery failed!${NC}"
    echo ""
    echo "Check logs for:"
    echo "  - Message sending confirmation"
    echo "  - Encryption/decryption errors"
    echo "  - Network connectivity issues"

    # Show recent logs
    echo ""
    echo "Recent logs:"
    monitor_android
    exit 1
fi

echo -e "${GREEN}✓ Direct messaging works${NC}"

echo ""
echo -e "${YELLOW}Step 6: Test Reply${NC}"
echo "Bob: Send a reply to Alice: 'Hi Alice, this is Bob!'"
echo "Alice: Check if message appears in inbox"
echo ""
read -p "Did Alice receive Bob's reply? (y/n): " reply_received

if [ "$reply_received" != "y" ]; then
    echo -e "${RED}Reply failed!${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Bidirectional messaging works${NC}"

echo ""
echo -e "${YELLOW}Step 7: Test Post Publishing${NC}"
echo "Alice: Create a public post: 'Testing P2P posts from Alice'"
echo "Bob: Check feed to see if Alice's post appears"
echo ""
read -p "Did Bob see Alice's post? (y/n): " post_seen

if [ "$post_seen" != "y" ]; then
    echo -e "${YELLOW}Note: Post syncing may take a moment. Check again.${NC}"
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Cross-Device P2P Test Results${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "${GREEN}✓ Peer discovery${NC}"
echo -e "${GREEN}✓ Direct messaging (Alice -> Bob)${NC}"
echo -e "${GREEN}✓ Reply messaging (Bob -> Alice)${NC}"
if [ "$post_seen" = "y" ]; then
    echo -e "${GREEN}✓ Post publishing and sync${NC}"
else
    echo -e "${YELLOW}⚠ Post sync pending verification${NC}"
fi
echo ""

echo -e "${BLUE}Additional Tests to Try:${NC}"
echo "  - Disconnect and reconnect WiFi"
echo "  - Send multiple rapid messages"
echo "  - Share media attachments"
echo "  - Test on different networks (internet DHT)"
echo "  - Check 'Online (N)' status updates"
echo ""

echo -e "${GREEN}Test completed! Check logs for any warnings or errors.${NC}"

# Offer to show detailed logs
read -p "Show detailed logs? (y/n): " show_logs
if [ "$show_logs" = "y" ]; then
    echo ""
    echo -e "${BLUE}=== Detailed macOS Logs ===${NC}"
    monitor_macos | tail -100

    echo ""
    echo -e "${BLUE}=== Detailed Android Logs ===${NC}"
    adb logcat -d -s "RustStdoutStderr:I" | tail -100
fi

echo ""
echo -e "${GREEN}Done!${NC}"
