#!/bin/bash

# Update Release Links Script
# Updates download links throughout the documentation

set -e

VERSION=${1:-"1.0.0"}
GITHUB_REPO=${2:-"aosmith/cipher"}

echo "🔗 Updating release links for v${VERSION}"

# Function to update links in a file
update_file_links() {
    local file="$1"
    local temp_file="${file}.tmp"
    
    if [[ ! -f "$file" ]]; then
        echo "⚠️  File $file not found, skipping"
        return
    fi
    
    echo "📝 Updating $file"
    
    # Replace GitHub release URLs
    sed "s|https://github.com/${GITHUB_REPO}/releases/download/v[0-9.]*/|https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}/|g" "$file" > "$temp_file"
    
    # Replace version numbers in filenames
    sed -i.bak "s|cipher_[0-9.]*_|cipher_${VERSION}_|g" "$temp_file"
    sed -i.bak "s|Cipher_[0-9.]*_|Cipher_${VERSION}_|g" "$temp_file"
    
    # Replace version strings
    sed -i.bak "s|Version.*: v[0-9.]*|Version**: v${VERSION}|g" "$temp_file"
    sed -i.bak "s|Latest Version (v[0-9.]*)|Latest Version (v${VERSION})|g" "$temp_file"
    
    # Clean up backup files
    rm -f "${temp_file}.bak"
    
    # Replace original file
    mv "$temp_file" "$file"
    
    echo "✅ Updated $file"
}

# Update main releases README
update_file_links "releases/README.md"

# Update platform-specific READMEs
update_file_links "releases/windows/README.md"
update_file_links "releases/macos/README.md"
update_file_links "releases/linux/README.md"

# Update main project documentation
update_file_links "README-DESKTOP.md"
update_file_links "README-CROSS-PLATFORM.md"

# Update website documentation
if [[ -f "docs/index.html" ]]; then
    echo "📝 Updating docs/index.html"
    sed -i.bak "s|v[0-9.]*|v${VERSION}|g" docs/index.html
    rm -f docs/index.html.bak
    echo "✅ Updated docs/index.html"
fi

# Update package.json version
if [[ -f "package.json" ]]; then
    echo "📝 Updating package.json version"
    sed -i.bak "s|\"version\": \"[0-9.]*\"|\"version\": \"${VERSION}\"|g" package.json
    rm -f package.json.bak
    echo "✅ Updated package.json"
fi

# Update Cargo.toml version (only package version, not dependencies)
if [[ -f "src-tauri/Cargo.toml" ]]; then
    echo "📝 Updating src-tauri/Cargo.toml version"
    # Only update the package version in [package] section, not dependencies
    sed -i.bak "/^\[package\]/,/^\[/ s|^version = \"[0-9.]*\"|version = \"${VERSION}\"|" src-tauri/Cargo.toml
    rm -f src-tauri/Cargo.toml.bak
    echo "✅ Updated src-tauri/Cargo.toml"
fi

# Update tauri.conf.json version
if [[ -f "src-tauri/tauri.conf.json" ]]; then
    echo "📝 Updating src-tauri/tauri.conf.json version"
    sed -i.bak "s|\"version\": \"[0-9.]*\"|\"version\": \"${VERSION}\"|g" src-tauri/tauri.conf.json
    rm -f src-tauri/tauri.conf.json.bak
    echo "✅ Updated src-tauri/tauri.conf.json"
fi

echo ""
echo "🎉 Successfully updated all release links to v${VERSION}!"
echo ""
echo "📋 Updated files:"
echo "   • releases/README.md"
echo "   • releases/*/README.md"
echo "   • README-DESKTOP.md"
echo "   • README-CROSS-PLATFORM.md"
echo "   • docs/index.html"
echo "   • package.json"
echo "   • src-tauri/Cargo.toml"
echo "   • src-tauri/tauri.conf.json"
echo ""
echo "📝 Next steps:"
echo "   1. Review the changes"
echo "   2. Test the updated links"
echo "   3. Commit the version updates"