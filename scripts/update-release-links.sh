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

# Update iOS project.yml (XcodeGen config)
echo "Updating gen/apple/project.yml..."
sed -i '' "s/CFBundleShortVersionString: [0-9]*\.[0-9]*\.[0-9]*/CFBundleShortVersionString: $VERSION/" gen/apple/project.yml
sed -i '' "s/CFBundleVersion: \"[0-9]*\.[0-9]*\.[0-9]*\"/CFBundleVersion: \"$VERSION\"/" gen/apple/project.yml

# Update iOS Info.plist
echo "Updating gen/apple/cipher-social_iOS/Info.plist..."
sed -i '' "s/<string>[0-9]*\.[0-9]*\.[0-9]*<\/string>/<string>$VERSION<\/string>/g" gen/apple/cipher-social_iOS/Info.plist

echo "✅ Version updated to $VERSION in all files"
echo ""
echo "Files updated:"
echo "  - Cargo.toml"
echo "  - tauri.conf.json"
echo "  - tauri.android.conf.json"
echo "  - tauri.ios.conf.json"
echo "  - tauri.mobile.conf.json"
echo "  - releases/README.md"
echo "  - gen/apple/project.yml"
echo "  - gen/apple/cipher-social_iOS/Info.plist"
