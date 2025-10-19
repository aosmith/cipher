#!/bin/bash

# Script to launch two Cipher instances side by side for P2P testing
# Each instance gets its own isolated data directory (database + localStorage)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Launching dual Cipher instances for P2P testing${NC}"

# Clean up any existing processes first
echo "Cleaning up any existing Cipher processes..."
pkill -f "cargo-tauri dev" || true
pkill -f "cipher" || true
sleep 2

# Clean up any existing test directories
echo "Cleaning up old test data..."
rm -rf /tmp/cipher_alice /tmp/cipher_bob

# Create separate data directories
mkdir -p /tmp/cipher_alice
mkdir -p /tmp/cipher_bob

echo -e "${GREEN}Created test directories:${NC}"
echo "  Alice: /tmp/cipher_alice"
echo "  Bob: /tmp/cipher_bob"

# Function to launch an instance with custom app data directory
launch_instance() {
    local name=$1
    local data_dir=$2
    local port=$3

    echo -e "${GREEN}Launching $name instance on port $port...${NC}"

    # Launch Tauri dev with completely isolated data directory
    # This gives each instance its own localStorage, database, and all app data
    CIPHER_TEST_DATA_DIR="$data_dir" \
    TAURI_DEV_PORT="$port" \
    cargo tauri dev > /tmp/cipher_${name}.log 2>&1 &

    # Store PID for cleanup
    local pid=$!
    echo $pid > /tmp/cipher_${name}_pid

    echo "  $name PID: $pid"
    echo "  $name data: $data_dir"
    echo "  $name port: $port"
    echo "  $name log: /tmp/cipher_${name}.log"
}

# Trap Ctrl+C to clean up both processes
cleanup() {
    echo -e "\n${YELLOW}Shutting down instances...${NC}"

    if [ -f /tmp/cipher_alice_pid ]; then
        alice_pid=$(cat /tmp/cipher_alice_pid)
        kill $alice_pid 2>/dev/null || true
        rm /tmp/cipher_alice_pid
        echo "  Stopped Alice (PID: $alice_pid)"
    fi

    if [ -f /tmp/cipher_bob_pid ]; then
        bob_pid=$(cat /tmp/cipher_bob_pid)
        kill $bob_pid 2>/dev/null || true
        rm /tmp/cipher_bob_pid
        echo "  Stopped Bob (PID: $bob_pid)"
    fi

    # Kill any remaining processes
    pkill -f "cargo-tauri dev" || true
    pkill -f "cipher" || true

    echo -e "${GREEN}Cleanup complete${NC}"
    exit 0
}

trap cleanup SIGINT SIGTERM EXIT

# Launch both instances on different dev ports
echo ""
launch_instance "alice" "/tmp/cipher_alice" "1420"
sleep 5  # Give first instance time to fully start and bind port
launch_instance "bob" "/tmp/cipher_bob" "1421"

echo ""
echo -e "${GREEN}Both instances launched!${NC}"
echo ""
echo "You can now:"
echo "  1. Sign up/login as 'alice' in the first window"
echo "  2. Sign up/login as 'bob' in the second window"
echo "  3. Add each other as friends using QR codes or friend codes"
echo "  4. Test P2P messaging and feed features"
echo ""
echo -e "${YELLOW}Important: Each instance has completely isolated storage${NC}"
echo "  - Separate databases"
echo "  - Separate localStorage (no shared sessions)"
echo "  - Separate P2P identities"
echo ""
echo -e "${YELLOW}Logs:${NC}"
echo "  Alice: tail -f /tmp/cipher_alice.log"
echo "  Bob: tail -f /tmp/cipher_bob.log"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop both instances${NC}"
echo ""

# Wait for both processes
wait
