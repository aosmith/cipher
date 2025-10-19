#!/bin/bash

# Clean up build artifacts to reduce repository size
# Usage: ./scripts/cleanup-artifacts.sh [--keep-release-binaries]

set -e

KEEP_RELEASE_BINARIES=false
if [[ "$1" == "--keep-release-binaries" ]]; then
    KEEP_RELEASE_BINARIES=true
fi

echo "🧹 Cleaning up build artifacts..."

# Get initial size
INITIAL_SIZE=$(du -sh . 2>/dev/null | cut -f1)
echo "Initial size: $INITIAL_SIZE"

if [ "$KEEP_RELEASE_BINARIES" = true ]; then
    echo "📦 Keeping release binaries, cleaning debug builds only..."

    # Remove debug builds (21GB saved!)
    rm -rf target/debug 2>/dev/null || true

    # Clean Android debug artifacts
    rm -rf gen/android/app/build/intermediates 2>/dev/null || true
    rm -rf gen/android/app/build/tmp 2>/dev/null || true
    rm -rf gen/android/.gradle 2>/dev/null || true

    # Clean incremental compilation artifacts from release builds
    find target -type d -name "incremental" -exec rm -rf {} + 2>/dev/null || true
    find target -type d -name ".fingerprint" -exec rm -rf {} + 2>/dev/null || true
else
    echo "📦 Full cleanup with cargo clean (removes all build artifacts)..."

    # Clean Rust/Cargo artifacts (removes entire target directory)
    cargo clean 2>/dev/null || true

    # Clean Android build artifacts
    rm -rf gen/android/app/build 2>/dev/null || true
    rm -rf gen/android/build 2>/dev/null || true
    rm -rf gen/android/.gradle 2>/dev/null || true

    # Clean iOS build artifacts
    rm -rf gen/ios/App/build 2>/dev/null || true
    rm -rf gen/ios/App/DerivedData 2>/dev/null || true

    # Clean generated Android/iOS project files (optional, will be regenerated)
    rm -rf gen 2>/dev/null || true
fi

# Clean temporary files
echo "📄 Cleaning temporary files..."
find . -name "*.tmp" -type f -delete 2>/dev/null || true
find . -name "*.backup" -type f -delete 2>/dev/null || true
find . -name "*.bak" -type f -delete 2>/dev/null || true
find . -name "*~" -type f -delete 2>/dev/null || true
find . -name ".DS_Store" -type f -delete 2>/dev/null || true

# Get final size
FINAL_SIZE=$(du -sh . 2>/dev/null | cut -f1)
echo ""
echo "✅ Cleanup complete!"
echo "   Before: $INITIAL_SIZE"
echo "   After:  $FINAL_SIZE"

# Show largest remaining directories
echo ""
echo "📊 Largest directories:"
du -sh target gen releases 2>/dev/null | sort -rh