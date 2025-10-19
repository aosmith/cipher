#!/bin/bash
# Clean up build artifacts after compilation
# This should be run after copying final binaries to releases/

echo "Cleaning build artifacts..."

# Clean Rust build artifacts (except release binaries)
echo "Cleaning Rust target directory..."
cargo clean

# Clean Android build artifacts
if [ -d "gen/android" ]; then
    echo "Cleaning Android build artifacts..."
    rm -rf gen/android/app/build
    rm -rf gen/android/.gradle
fi

# Clean iOS build artifacts
if [ -d "gen/ios" ]; then
    echo "Cleaning iOS build artifacts..."
    rm -rf gen/ios/build
    rm -rf gen/ios/DerivedData
fi

echo "Build cleanup complete!"
echo "Recovered disk space. Run 'du -sh target gen' to verify."
