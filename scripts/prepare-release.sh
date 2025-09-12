#!/bin/bash

# Prepare Release Script for Cipher Desktop App
# This script builds all platforms and prepares release files

set -e

VERSION=${1:-"1.0.0"}
echo "🔐 Preparing Cipher Desktop App Release v${VERSION}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper function for colored output
log() {
    echo -e "${BLUE}[$(date +'%H:%M:%S')] $1${NC}"
}

success() {
    echo -e "${GREEN}✅ $1${NC}"
}

warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

error() {
    echo -e "${RED}❌ $1${NC}"
    exit 1
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    # Check if we're in the right directory
    if [[ ! -f "package.json" ]] || [[ ! -d "src-tauri" ]]; then
        error "Must run from project root directory"
    fi
    
    # Check required tools
    command -v npm >/dev/null 2>&1 || error "npm not found"
    command -v cargo >/dev/null 2>&1 || error "cargo not found"
    command -v bundle >/dev/null 2>&1 || error "bundle not found"
    
    success "Prerequisites checked"
}

# Clean previous builds
clean_builds() {
    log "Cleaning previous builds..."
    
    if [[ -d "src-tauri/target" ]]; then
        rm -rf src-tauri/target/release/bundle
        log "Cleaned Tauri build directory"
    fi
    
    success "Build directories cleaned"
}

# Install dependencies
install_dependencies() {
    log "Installing dependencies..."
    
    npm install
    bundle install
    
    success "Dependencies installed"
}

# Prepare Rails assets
prepare_rails() {
    log "Preparing Rails application..."
    
    export RAILS_ENV=production
    bundle exec rails assets:clobber
    bundle exec rails assets:precompile
    
    success "Rails assets prepared"
}

# Build for all platforms (if tools available)
build_all_platforms() {
    log "Building desktop applications..."
    
    # Build for current platform
    npm run tauri:build
    
    # Try to build for other platforms if tools are available
    if command -v rustup >/dev/null 2>&1; then
        # Check available targets
        TARGETS=$(rustup target list --installed)
        
        if echo "$TARGETS" | grep -q "x86_64-pc-windows-msvc"; then
            log "Building for Windows..."
            npm run tauri:build:windows || warning "Windows build failed"
        fi
        
        if echo "$TARGETS" | grep -q "x86_64-apple-darwin"; then
            log "Building for macOS Intel..."
            npm run tauri:build:macos || warning "macOS Intel build failed"
        fi
        
        if echo "$TARGETS" | grep -q "aarch64-apple-darwin"; then
            log "Building for macOS Apple Silicon..."
            npm run tauri:build:macos-arm || warning "macOS Apple Silicon build failed"
        fi
        
        if echo "$TARGETS" | grep -q "x86_64-unknown-linux-gnu"; then
            log "Building for Linux..."
            npm run tauri:build:linux || warning "Linux build failed"
        fi
    fi
    
    success "Build process completed"
}

# Copy builds to releases directory
copy_to_releases() {
    log "Copying builds to releases directory..."
    
    local bundle_dir="src-tauri/target/release/bundle"
    local release_base="releases"
    
    # Create version directories
    mkdir -p "${release_base}/windows/archive/v${VERSION}"
    mkdir -p "${release_base}/macos/archive/v${VERSION}"
    mkdir -p "${release_base}/linux/archive/v${VERSION}"
    
    # Copy Windows builds
    if [[ -d "${bundle_dir}/msi" ]]; then
        cp "${bundle_dir}/msi"/*.msi "${release_base}/windows/latest/" 2>/dev/null || true
        cp "${bundle_base}/msi"/*.msi "${release_base}/windows/archive/v${VERSION}/" 2>/dev/null || true
        log "Copied Windows MSI files"
    fi
    
    if [[ -d "${bundle_dir}/nsis" ]]; then
        cp "${bundle_dir}/nsis"/*.exe "${release_base}/windows/latest/" 2>/dev/null || true
        cp "${bundle_dir}/nsis"/*.exe "${release_base}/windows/archive/v${VERSION}/" 2>/dev/null || true
        log "Copied Windows NSIS files"
    fi
    
    # Copy macOS builds
    if [[ -d "${bundle_dir}/dmg" ]]; then
        cp "${bundle_dir}/dmg"/*.dmg "${release_base}/macos/latest/" 2>/dev/null || true
        cp "${bundle_dir}/dmg"/*.dmg "${release_base}/macos/archive/v${VERSION}/" 2>/dev/null || true
        log "Copied macOS DMG files"
    fi
    
    if [[ -d "${bundle_dir}/macos" ]]; then
        # Create zip of .app bundle using absolute paths
        local current_dir=$(pwd)
        cd "${bundle_dir}/macos"
        for app in *.app; do
            if [[ -d "$app" ]]; then
                zip -r "${current_dir}/${release_base}/macos/latest/${app%.app}_${VERSION}.zip" "$app" 2>/dev/null || true
                zip -r "${current_dir}/${release_base}/macos/archive/v${VERSION}/${app%.app}_${VERSION}.zip" "$app" 2>/dev/null || true
            fi
        done
        cd - >/dev/null
        log "Copied macOS app bundles"
    fi
    
    # Copy Linux builds
    if [[ -d "${bundle_dir}/deb" ]]; then
        cp "${bundle_dir}/deb"/*.deb "${release_base}/linux/latest/" 2>/dev/null || true
        cp "${bundle_dir}/deb"/*.deb "${release_base}/linux/archive/v${VERSION}/" 2>/dev/null || true
        log "Copied Linux DEB files"
    fi
    
    if [[ -d "${bundle_dir}/appimage" ]]; then
        cp "${bundle_dir}/appimage"/*.AppImage "${release_base}/linux/latest/" 2>/dev/null || true
        cp "${bundle_dir}/appimage"/*.AppImage "${release_base}/linux/archive/v${VERSION}/" 2>/dev/null || true
        log "Copied Linux AppImage files"
    fi
    
    success "Files copied to releases directory"
}

# Generate checksums
generate_checksums() {
    log "Generating checksums..."
    
    local platforms=("windows" "macos" "linux")
    
    for platform in "${platforms[@]}"; do
        local latest_dir="releases/${platform}/latest"
        local archive_dir="releases/${platform}/archive/v${VERSION}"
        
        if [[ -d "$latest_dir" ]]; then
            cd "$latest_dir"
            if ls * >/dev/null 2>&1; then
                shasum -a 256 * > checksums.txt
                log "Generated checksums for ${platform}/latest"
            fi
            cd - >/dev/null
        fi
        
        if [[ -d "$archive_dir" ]]; then
            cd "$archive_dir"
            if ls * >/dev/null 2>&1; then
                shasum -a 256 * > checksums.txt
                log "Generated checksums for ${platform}/archive/v${VERSION}"
            fi
            cd - >/dev/null
        fi
    done
    
    success "Checksums generated"
}

# Create release notes
create_release_notes() {
    log "Creating release notes..."
    
    local platforms=("windows" "macos" "linux")
    local release_date=$(date "+%Y-%m-%d")
    
    for platform in "${platforms[@]}"; do
        local latest_dir="releases/${platform}/latest"
        
        if [[ -d "$latest_dir" ]]; then
            cat > "${latest_dir}/CHANGELOG.md" <<EOF
# Cipher Desktop App v${VERSION} - ${platform^}

**Release Date**: ${release_date}

## What's New

- Cross-platform desktop application using Tauri
- Native window controls and system integration
- End-to-end encrypted P2P social networking
- Lightweight bundle size (~10-18MB vs 100MB+ Electron apps)
- System tray integration
- Platform-specific UI optimizations

## Installation

See [README.md](README.md) for installation instructions.

## System Requirements

$(cat "releases/${platform}/README.md" | grep -A 10 "## System Requirements" | tail -n +2)

## Files in This Release

$(cd "${latest_dir}" && ls -la | grep -v "^total" | grep -v "^d" | awk '{print "- " $9 " (" $5 " bytes)"}')

## Checksums

See \`checksums.txt\` for SHA256 verification hashes.
EOF
            log "Created CHANGELOG.md for ${platform}"
        fi
    done
    
    success "Release notes created"
}

# Main execution
main() {
    log "Starting release preparation for v${VERSION}"
    
    check_prerequisites
    clean_builds
    install_dependencies
    prepare_rails
    build_all_platforms
    copy_to_releases
    generate_checksums
    create_release_notes
    
    success "Release v${VERSION} prepared successfully!"
    echo ""
    echo "📁 Release files are in the releases/ directory"
    echo "📝 Next steps:"
    echo "   1. Test the applications on each platform"
    echo "   2. Update version numbers if needed"
    echo "   3. Create GitHub release with these files"
    echo "   4. Update download links in documentation"
}

# Run main function
main "$@"