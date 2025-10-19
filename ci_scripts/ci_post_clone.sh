#!/bin/bash

# Xcode Cloud post-clone script
# This runs immediately after cloning the repository

set -e

echo "=== Xcode Cloud Post-Clone: Initial Setup ==="
echo "Working directory: $(pwd)"
echo "HOME: $HOME"
echo "USER: $USER"
env | sort

# Install Rust
if ! command -v rustc &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
    source "$HOME/.cargo/env"
else
    echo "Rust already installed: $(rustc --version)"
fi

# Add cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Persist PATH for subsequent build phases
mkdir -p "$HOME"
echo "export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >> "$HOME/.bash_profile"
echo "export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >> "$HOME/.profile"
echo "export PATH=\"\$HOME/.cargo/bin:\$PATH\"" >> "$HOME/.bashrc"

# Source cargo env to ensure it's available
source "$HOME/.cargo/env"

# Install iOS targets
echo "Installing iOS targets..."
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios

# Install Tauri CLI
echo "Installing Tauri CLI..."
cargo install tauri-cli --version 2.8.4 --locked

# Verify installation
echo "=== Verification ==="
echo "Rust version: $(rustc --version)"
echo "Cargo version: $(cargo --version)"
echo "Cargo location: $(which cargo)"
echo "Tauri CLI version: $(cargo tauri --version)"
echo "Tauri CLI location: $(which cargo-tauri)"
echo "iOS targets installed:"
rustup target list --installed | grep apple

# Note: Xcode Cloud build phases will access cargo via PATH build setting
# PATH is set in gen/apple/cipher-social.xcodeproj/project.pbxproj as:
# PATH = "$(HOME)/.cargo/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";
# This allows the "Build Rust Code" phase to find cargo without needing symlinks

echo "=== Post-clone setup complete ==="
