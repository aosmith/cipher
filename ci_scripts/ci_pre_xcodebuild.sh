#!/bin/bash

# Xcode Cloud pre-build script
# Minimal version - just write cargo path for build phase

echo "=== ci_pre_xcodebuild.sh START ==="
echo "PWD: $(pwd)"
echo "HOME: $HOME"

# Source cargo
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
export PATH="$HOME/.cargo/bin:$PATH"

# Write cargo path
which cargo > /tmp/cargo_path.txt 2>&1 || echo "cargo not found"
cat /tmp/cargo_path.txt

echo "=== ci_pre_xcodebuild.sh END ==="
