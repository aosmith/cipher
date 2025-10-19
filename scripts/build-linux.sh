#!/bin/bash

# Build Linux packages using Docker
# Usage: ./scripts/build-linux.sh
# Requires: Docker installed and running

set -e

# Disable ANSI color codes
export CARGO_TERM_COLOR=never
export NO_COLOR=1

VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d'"' -f2)
echo "Building Cipher v${VERSION} for Linux..."

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Error: Docker is not running. Please start Docker and try again."
    exit 1
fi

# Create output directory structure (matches existing pattern)
mkdir -p releases/linux/latest

# Clean up any existing Linux builds (binaries NOT committed to git)
echo "Cleaning old Linux builds..."
rm -f releases/linux/latest/*.AppImage 2>/dev/null || true
rm -f releases/linux/latest/*.deb 2>/dev/null || true
rm -f releases/linux/latest/*.rpm 2>/dev/null || true

# Clean up build artifacts
echo "Cleaning old Linux build artifacts..."
rm -rf target/aarch64-unknown-linux-gnu 2>/dev/null || true
rm -rf target/x86_64-unknown-linux-gnu 2>/dev/null || true

# Create Dockerfile for building
echo "Creating build environment..."
cat > Dockerfile.linux-build << 'EOF'
FROM rust:slim-bookworm

ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies for Tauri + RPM tools + AppImage tools
RUN apt-get update && apt-get install -y \
    wget \
    build-essential \
    pkg-config \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    libjavascriptcoregtk-4.1-dev \
    libsoup-3.0-dev \
    file \
    rpm \
    fuse \
    libfuse2 \
    && rm -rf /var/lib/apt/lists/*

# Rust is already installed in rust:latest
# Install Tauri CLI for building (without --locked to avoid edition2024 issues)
RUN cargo install tauri-cli --version ^2.0

WORKDIR /app
EOF

# Build Docker image
echo "Building Docker image (this may take a few minutes)..."
docker build -f Dockerfile.linux-build -t cipher-linux-builder .

# Build Linux packages in Docker (native arch - ARM64 or x86_64)
echo ""
echo "Building Linux packages in Docker container (native architecture)..."
docker run --rm \
    -v "$(pwd)":/app \
    -e CARGO_TERM_COLOR=never \
    -e NO_COLOR=1 \
    cipher-linux-builder \
    bash -c "cargo tauri build"

# Copy builds to releases directory (matches build-all.sh pattern)
echo ""
echo "Copying builds to releases/linux/latest/..."

# Copy Deb package (builds go to target/release when not cross-compiling)
if ls target/release/bundle/deb/*.deb 1> /dev/null 2>&1; then
    cp target/release/bundle/deb/*.deb "releases/linux/latest/Cipher.deb"
    echo "✅ Deb package copied to releases/linux/latest/Cipher.deb"
else
    echo "❌ Deb package not found"
fi

# Copy RPM package
if ls target/release/bundle/rpm/*.rpm 1> /dev/null 2>&1; then
    cp target/release/bundle/rpm/*.rpm "releases/linux/latest/Cipher.rpm"
    echo "✅ RPM package copied to releases/linux/latest/Cipher.rpm"
else
    echo "❌ RPM package not found"
fi

# Copy AppImage
if ls target/release/bundle/appimage/*.AppImage 1> /dev/null 2>&1; then
    cp target/release/bundle/appimage/*.AppImage "releases/linux/latest/Cipher.AppImage"
    echo "✅ AppImage copied to releases/linux/latest/Cipher.AppImage"
else
    echo "❌ AppImage not found"
fi

# Generate SHA256 checksums (matches build-all.sh pattern)
if [ -f "releases/linux/latest/Cipher.deb" ] || [ -f "releases/linux/latest/Cipher.rpm" ] || [ -f "releases/linux/latest/Cipher.AppImage" ]; then
    echo ""
    echo "Generating SHA256 checksums..."
    cd releases/linux/latest
    shasum -a 256 Cipher.* > checksums.txt 2>/dev/null || true
    cd ../../..
    echo "✅ Checksums generated"
fi

# Clean up Docker image and Dockerfile
echo ""
echo "Cleaning up Docker image..."
docker rmi cipher-linux-builder 2>/dev/null || true
rm Dockerfile.linux-build

echo ""
echo "📊 Linux Build Summary for v${VERSION}:"
echo "================================"
ls -lh releases/linux/latest/ 2>/dev/null
echo ""
echo "SHA256 Checksums:"
cat releases/linux/latest/checksums.txt 2>/dev/null || true

echo ""
echo "Cleaning up build artifacts..."
rm -rf target/aarch64-unknown-linux-gnu 2>/dev/null || true
rm -rf target/x86_64-unknown-linux-gnu 2>/dev/null || true
echo "   Cleaned target/ build directories"
echo "   Release binaries preserved in releases/linux/latest/"

echo ""
echo "✅ Linux builds complete!"
echo "   Next steps:"
echo "   1. Test the builds in releases/linux/latest/"
echo "   2. Upload binaries to GitHub Releases (binaries NOT committed to git)"
echo "   3. Tag: git tag v${VERSION}"
echo "   4. Push: git push --tags"
