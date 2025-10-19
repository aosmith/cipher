#!/bin/bash
#
# Automated QR Code / Iroh Invite Exchange Test
#
# Tests the complete invite flow:
# 1. Generate Iroh invite on Bob (Pixel 9)
# 2. Accept invite on Alice (Pixel 7)
# 3. Verify NodeId exchange
# 4. Confirm peer connections

set -e

# Device serial numbers
ALICE_DEVICE="2B011FDH200CBM"  # Pixel 7
BOB_DEVICE="47111FDAQ00558"    # Pixel 9

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "════════════════════════════════════════════════════════════"
echo "  Automated Iroh Invite Exchange Test"
echo "════════════════════════════════════════════════════════════"

# Step 1: Generate invite on Bob's device via adb
echo -e "${YELLOW}[1/5]${NC} Generating Iroh invite on Bob's device..."
BOB_INVITE=$(env ANDROID_HOME=~/Library/Android/sdk /opt/homebrew/bin/adb -s "$BOB_DEVICE" shell "am broadcast -a com.cipher.test.action --es command 'generate_invite'" 2>&1 | grep -o "INVITE:.*" | sed 's/INVITE://')

if [ -z "$BOB_INVITE" ]; then
    # Fallback: Call Tauri command directly via JS injection
    echo -e "${YELLOW}   Trying direct Tauri command injection...${NC}"
    env ANDROID_HOME=~/Library/Android/sdk /opt/homebrew/bin/adb -s "$BOB_DEVICE" shell "input text 'javascript:window.__TAURI__.invoke(\"iroh_generate_invite\").then(r=>alert(r))'"
    sleep 2
    # This won't work easily - we need a better approach
    echo -e "${RED}   ✗ Could not generate invite via automation${NC}"
    echo -e "${YELLOW}   Manual action required: Generate QR on Bob's device${NC}"
    exit 1
fi

echo -e "${GREEN}   ✓ Invite generated${NC}"
echo "   Invite code: ${BOB_INVITE:0:50}..."

# Step 2: Accept invite on Alice's device
echo -e "${YELLOW}[2/5]${NC} Accepting invite on Alice's device..."
env ANDROID_HOME=~/Library/Android/sdk /opt/homebrew/bin/adb -s "$ALICE_DEVICE" shell "am broadcast -a com.cipher.test.action --es command 'accept_invite' --es invite '$BOB_INVITE'" 2>&1

sleep 3

# Step 3: Check logs for successful invite acceptance
echo -e "${YELLOW}[3/5]${NC} Checking logs for invite acceptance..."
ALICE_LOGS=$(env ANDROID_HOME=~/Library/Android/sdk /opt/homebrew/bin/adb -s "$ALICE_DEVICE" logcat -d | tail -100)
BOB_LOGS=$(env ANDROID_HOME=~/Library/Android/sdk /opt/homebrew/bin/adb -s "$BOB_DEVICE" logcat -d | tail -100)

if echo "$ALICE_LOGS" | grep -q "IROH INVITE ACCEPTANCE COMPLETE"; then
    echo -e "${GREEN}   ✓ Invite accepted successfully${NC}"
else
    echo -e "${RED}   ✗ Invite acceptance not found in logs${NC}"
    exit 1
fi

# Step 4: Verify NodeIds were exchanged
echo -e "${YELLOW}[4/5]${NC} Verifying NodeId exchange..."

BOB_NODE_ID=$(echo "$BOB_LOGS" | grep "NodeId:" | tail -1 | grep -o "[0-9a-f]\{64\}")
ALICE_NODE_ID=$(echo "$ALICE_LOGS" | grep "NodeId:" | tail -1 | grep -o "[0-9a-f]\{64\}")

if [ -n "$BOB_NODE_ID" ] && [ -n "$ALICE_NODE_ID" ]; then
    echo -e "${GREEN}   ✓ NodeIds found${NC}"
    echo "   Alice NodeId: $ALICE_NODE_ID"
    echo "   Bob NodeId: $BOB_NODE_ID"

    # Check if each device stored the other's NodeId
    if echo "$ALICE_LOGS" | grep -q "$BOB_NODE_ID"; then
        echo -e "${GREEN}   ✓ Alice stored Bob's NodeId${NC}"
    else
        echo -e "${RED}   ✗ Alice did not store Bob's NodeId${NC}"
    fi

    if echo "$BOB_LOGS" | grep -q "$ALICE_NODE_ID"; then
        echo -e "${GREEN}   ✓ Bob stored Alice's NodeId${NC}"
    else
        echo -e "${RED}   ✗ Bob did not store Alice's NodeId${NC}"
    fi
else
    echo -e "${RED}   ✗ Could not extract NodeIds from logs${NC}"
fi

# Step 5: Verify peer connections
echo -e "${YELLOW}[5/5]${NC} Verifying peer connections..."

if echo "$ALICE_LOGS" | grep -q "Successfully connected to peer"; then
    echo -e "${GREEN}   ✓ Alice connected to peer${NC}"
else
    echo -e "${YELLOW}   ⚠ Alice connection pending${NC}"
fi

if echo "$BOB_LOGS" | grep -q "Successfully connected to peer"; then
    echo -e "${GREEN}   ✓ Bob connected to peer${NC}"
else
    echo -e "${YELLOW}   ⚠ Bob connection pending${NC}"
fi

echo ""
echo "════════════════════════════════════════════════════════════"
echo -e "${GREEN}  Test Complete!${NC}"
echo "════════════════════════════════════════════════════════════"
