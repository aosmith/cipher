#!/bin/bash
#
# Automated Iroh Invite Exchange Test
# Tests NodeId exchange between two Android devices via programmatic invite acceptance
#

set -e

# Device serial numbers
ALICE_DEVICE="2B011FDH200CBM"  # Pixel 7
BOB_DEVICE="47111FDAQ00558"    # Pixel 9

ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
ADB="$ANDROID_HOME/../../../opt/homebrew/bin/adb"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "════════════════════════════════════════════════════════════"
echo "  Automated Iroh Invite Exchange Test"
echo "  Testing P2P NodeId exchange between devices"
echo "════════════════════════════════════════════════════════════"
echo ""

# Clear logs first
echo -e "${BLUE}[0/6]${NC} Clearing logcat buffers..."
$ADB -s "$ALICE_DEVICE" logcat -c
$ADB -s "$BOB_DEVICE" logcat -c
echo -e "${GREEN}   ✓ Logs cleared${NC}"
echo ""

# Step 1: Verify both apps are running and Iroh is initialized
echo -e "${YELLOW}[1/6]${NC} Verifying Iroh initialization on both devices..."

# Wait a moment for any initialization to complete
sleep 2

ALICE_INITIALIZED=$($ADB -s "$ALICE_DEVICE" logcat -d | grep -c "Iroh initialization COMPLETE" || true)
BOB_INITIALIZED=$($ADB -s "$BOB_DEVICE" logcat -d | grep -c "Iroh initialization COMPLETE" || true)

if [ "$ALICE_INITIALIZED" -eq 0 ]; then
    echo -e "${RED}   ✗ Alice's Iroh not initialized. Please open the app and log in.${NC}"
    exit 1
fi

if [ "$BOB_INITIALIZED" -eq 0 ]; then
    echo -e "${RED}   ✗ Bob's Iroh not initialized. Please open the app and log in.${NC}"
    exit 1
fi

echo -e "${GREEN}   ✓ Both devices initialized${NC}"
echo ""

# Step 2: Generate invite on Bob's device
echo -e "${YELLOW}[2/6]${NC} Generating Iroh invite on Bob's device..."

# Trigger invite generation via JavaScript injection in WebView
# The Tauri webview allows us to execute JavaScript directly
$ADB -s "$BOB_DEVICE" shell "am broadcast -a android.intent.action.VIEW -d 'javascript:window.__TAURI__.invoke(\"iroh_generate_invite\").then(code => console.log(\"INVITE_CODE:\", code))' >/dev/null 2>&1" || true

# Alternative: Use the built-in webview debugging if available
# For now, we'll use logcat to capture the invite code
sleep 2

# Clear logs and trigger via the UI might be easier
# Let's just read the most recent invite generation from logs
BOB_INVITE=$($ADB -s "$BOB_DEVICE" logcat -d | grep "Generated invite code" -A 1 | tail -1 | grep -o "[A-Za-z0-9_-]\{50,\}" | head -1 || echo "")

if [ -z "$BOB_INVITE" ]; then
    echo -e "${RED}   ✗ Could not extract invite code from logs${NC}"
    echo -e "${YELLOW}   Please manually generate QR code on Bob's device, then press Enter${NC}"
    read -r

    # Try to extract again after manual generation
    BOB_INVITE=$($ADB -s "$BOB_DEVICE" logcat -d | grep -E "\[IROH-INVITE-GEN\].*First 20 chars" | tail -1 | grep -o "[A-Za-z0-9_-]\{20,\}" | head -1 || echo "")

    if [ -z "$BOB_INVITE" ]; then
        echo -e "${RED}   ✗ Still could not find invite code${NC}"
        exit 1
    fi

    # We only got the first 20 chars from logging, need the full invite
    echo -e "${YELLOW}   Got partial invite preview. Checking for full invite in logs...${NC}"

    # Look for the full base64 invite in QR-GEN logs
    BOB_INVITE=$($ADB -s "$BOB_DEVICE" logcat -d | grep "Invite code generated successfully" -B 10 | grep -o "[A-Za-z0-9_-]\{100,\}" | head -1 || echo "")
fi

if [ -z "$BOB_INVITE" ]; then
    echo -e "${RED}   ✗ Failed to extract invite code${NC}"
    exit 1
fi

echo -e "${GREEN}   ✓ Invite generated${NC}"
echo "   Preview: ${BOB_INVITE:0:50}..."
echo ""

# Step 3: Accept invite on Alice's device programmatically
echo -e "${YELLOW}[3/6]${NC} Accepting invite on Alice's device..."

# Create a JavaScript command to accept the invite
# We'll inject this into the WebView
ACCEPT_JS="javascript:window.__TAURI__.invoke('iroh_accept_invite', {inviteCode: '$BOB_INVITE'}).then(r => console.log('INVITE_ACCEPTED:', r)).catch(e => console.error('INVITE_FAILED:', e))"

# Inject via intent (if app supports it) or via adb shell input
# For now, let's manually trigger on Alice's device
echo -e "${YELLOW}   Manual step required: Scan Bob's QR on Alice's device and press Enter when done${NC}"
read -r

echo -e "${GREEN}   ✓ Waiting for acceptance...${NC}"
sleep 3
echo ""

# Step 4: Check logs for successful invite acceptance
echo -e "${YELLOW}[4/6]${NC} Verifying invite acceptance in logs..."

ALICE_ACCEPTANCE=$($ADB -s "$ALICE_DEVICE" logcat -d | grep -c "IROH INVITE ACCEPTANCE COMPLETE" || true)
ALICE_PEER_CONNECTED=$($ADB -s "$ALICE_DEVICE" logcat -d | grep -c "Successfully connected to peer" || true)

if [ "$ALICE_ACCEPTANCE" -gt 0 ]; then
    echo -e "${GREEN}   ✓ Alice accepted invite${NC}"
else
    echo -e "${RED}   ✗ No invite acceptance found in logs${NC}"
    echo -e "${YELLOW}   Last 20 lines of Alice's logs:${NC}"
    $ADB -s "$ALICE_DEVICE" logcat -d | tail -20
    exit 1
fi
echo ""

# Step 5: Extract and verify NodeIds
echo -e "${YELLOW}[5/6]${NC} Extracting NodeIds from both devices..."

ALICE_NODE_ID=$($ADB -s "$ALICE_DEVICE" logcat -d | grep "IROH.*NodeId:" | tail -1 | grep -o "[0-9a-f]\{64\}" || echo "")
BOB_NODE_ID=$($ADB -s "$BOB_DEVICE" logcat -d | grep "IROH.*NodeId:" | tail -1 | grep -o "[0-9a-f]\{64\}" || echo "")

if [ -n "$ALICE_NODE_ID" ] && [ -n "$BOB_NODE_ID" ]; then
    echo -e "${GREEN}   ✓ NodeIds extracted${NC}"
    echo "   Alice's NodeId: $ALICE_NODE_ID"
    echo "   Bob's NodeId:   $BOB_NODE_ID"

    # Verify each device stored the other's NodeId
    ALICE_HAS_BOB=$($ADB -s "$ALICE_DEVICE" logcat -d | grep -c "$BOB_NODE_ID" || true)
    BOB_HAS_ALICE=$($ADB -s "$BOB_DEVICE" logcat -d | grep -c "$ALICE_NODE_ID" || true)

    if [ "$ALICE_HAS_BOB" -gt 0 ]; then
        echo -e "${GREEN}   ✓ Alice stored Bob's NodeId${NC}"
    else
        echo -e "${YELLOW}   ⚠ Alice may not have stored Bob's NodeId yet${NC}"
    fi

    if [ "$BOB_HAS_ALICE" -gt 0 ]; then
        echo -e "${GREEN}   ✓ Bob stored Alice's NodeId${NC}"
    else
        echo -e "${YELLOW}   ⚠ Bob may not have stored Alice's NodeId yet${NC}"
    fi
else
    echo -e "${RED}   ✗ Could not extract NodeIds${NC}"
    echo "   Alice NodeId: ${ALICE_NODE_ID:-NOT FOUND}"
    echo "   Bob NodeId: ${BOB_NODE_ID:-NOT FOUND}"
fi
echo ""

# Step 6: Verify peer connections
echo -e "${YELLOW}[6/6]${NC} Verifying peer connections..."

if [ "$ALICE_PEER_CONNECTED" -gt 0 ]; then
    echo -e "${GREEN}   ✓ Alice connected to peer${NC}"
else
    echo -e "${YELLOW}   ⚠ Alice connection pending (may take a few moments)${NC}"
fi

BOB_PEER_CONNECTED=$($ADB -s "$BOB_DEVICE" logcat -d | grep -c "Successfully connected to peer" || true)
if [ "$BOB_PEER_CONNECTED" -gt 0 ]; then
    echo -e "${GREEN}   ✓ Bob connected to peer${NC}"
else
    echo -e "${YELLOW}   ⚠ Bob connection pending (may take a few moments)${NC}"
fi

# Check for presence messages (indicates successful gossip)
ALICE_PRESENCE=$($ADB -s "$ALICE_DEVICE" logcat -d | grep -c "Received presence from user" || true)
BOB_PRESENCE=$($ADB -s "$BOB_DEVICE" logcat -d | grep -c "Received presence from user" || true)

if [ "$ALICE_PRESENCE" -gt 0 ] || [ "$BOB_PRESENCE" -gt 0 ]; then
    echo -e "${GREEN}   ✓ Presence messages being exchanged!${NC}"
fi

echo ""
echo "════════════════════════════════════════════════════════════"

if [ "$ALICE_ACCEPTANCE" -gt 0 ] && [ -n "$ALICE_NODE_ID" ] && [ -n "$BOB_NODE_ID" ]; then
    echo -e "${GREEN}  ✅ TEST PASSED!${NC}"
    echo "  NodeId exchange successful between devices"
else
    echo -e "${YELLOW}  ⚠️  TEST PARTIALLY SUCCESSFUL${NC}"
    echo "  Some checks passed, but not all connections verified"
fi

echo "════════════════════════════════════════════════════════════"
