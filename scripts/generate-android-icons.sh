#!/bin/bash

# Generate Android launcher icons from the source icon
# This script should be run whenever the icon.png is updated or when initializing Android build
#
# Usage: ./scripts/generate-android-icons.sh

set -e

ICON_SOURCE="icons/icon.png"
ANDROID_RES="gen/android/app/src/main/res"

if [ ! -f "$ICON_SOURCE" ]; then
    echo "Error: Source icon not found at $ICON_SOURCE"
    exit 1
fi

# Check if Android res directory exists
if [ ! -d "$ANDROID_RES" ]; then
    echo "Error: Android resources directory not found. Run 'cargo tauri android init' first."
    exit 1
fi

echo "Generating Android launcher icons from $ICON_SOURCE..."

# Generate ic_launcher for all densities
sips -z 48 48 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-mdpi/ic_launcher.png"
sips -z 72 72 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-hdpi/ic_launcher.png"
sips -z 96 96 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-xhdpi/ic_launcher.png"
sips -z 144 144 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-xxhdpi/ic_launcher.png"
sips -z 192 192 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-xxxhdpi/ic_launcher.png"

# Generate ic_launcher_round for all densities
sips -z 48 48 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-mdpi/ic_launcher_round.png"
sips -z 72 72 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-hdpi/ic_launcher_round.png"
sips -z 96 96 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-xhdpi/ic_launcher_round.png"
sips -z 144 144 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-xxhdpi/ic_launcher_round.png"
sips -z 192 192 "$ICON_SOURCE" --out "$ANDROID_RES/mipmap-xxxhdpi/ic_launcher_round.png"

echo "✓ Android launcher icons generated successfully!"
echo ""
echo "Icon sizes generated:"
echo "  mdpi:    48x48"
echo "  hdpi:    72x72"
echo "  xhdpi:   96x96"
echo "  xxhdpi:  144x144"
echo "  xxxhdpi: 192x192"
echo ""
echo "Note: icon.png should have no whitespace/padding. Use ImageMagick to trim:"
echo "  magick icons/icon.png -trim icons/icon.png"
