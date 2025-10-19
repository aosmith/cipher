#!/bin/bash

# Update version numbers across the Cipher project
# Usage: ./scripts/update-release-links.sh <version>

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.11.1"
    exit 1
fi

VERSION=$1

echo "Updating version to $VERSION..."

# Update Cargo.toml
echo "Updating Cargo.toml..."
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Update tauri.conf.json
echo "Updating tauri.conf.json..."
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" tauri.conf.json

# Update tauri.android.conf.json
echo "Updating tauri.android.conf.json..."
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" tauri.android.conf.json

# Update tauri.ios.conf.json
echo "Updating tauri.ios.conf.json..."
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" tauri.ios.conf.json

# Update tauri.mobile.conf.json
echo "Updating tauri.mobile.conf.json..."
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" tauri.mobile.conf.json

# Update releases/README.md
echo "Updating releases/README.md..."
sed -i '' "s/Current version: v[0-9]*\.[0-9]*\.[0-9]*/Current version: v$VERSION/" releases/README.md

echo "✅ Version updated to $VERSION in all files"
echo ""
echo "Files updated:"
echo "  - Cargo.toml"
echo "  - tauri.conf.json"
echo "  - tauri.android.conf.json"
echo "  - tauri.ios.conf.json"
echo "  - tauri.mobile.conf.json"
echo "  - releases/README.md"
