#!/bin/bash

# Test P2P connection with two desktop instances
# This allows us to debug connection issues much faster than on mobile

set -e

echo "=== Cipher P2P Desktop Test ==="
echo "Starting two instances to test symmetric connection state..."

# Kill any existing instances
pkill -f "cipher-social" || true

# Clean up databases
rm -rf /tmp/cipher_alice /tmp/cipher_bob

# Build desktop app
echo "Building desktop app..."
cargo build --release

# Start Alice in background
echo "Starting Alice..."
CIPHER_DB_PATH=/tmp/cipher_alice/db.sqlite3 RUST_LOG=cipher_social=debug ./target/release/cipher-social &
ALICE_PID=$!
sleep 3

# Start Bob in background
echo "Starting Bob..."
CIPHER_DB_PATH=/tmp/cipher_bob/db.sqlite3 RUST_LOG=cipher_social=debug ./target/release/cipher-social &
BOB_PID=$!
sleep 3

echo "Alice PID: $ALICE_PID"
echo "Bob PID: $BOB_PID"
echo ""
echo "Both instances running. Open http://localhost:1420 in two browser tabs."
echo "Create Alice in tab 1, Bob in tab 2"
echo "Generate QR on Alice, scan with Bob"
echo "Press Ctrl+C when done testing"

# Wait for user interrupt
trap "kill $ALICE_PID $BOB_PID 2>/dev/null; exit" INT TERM
wait
