#!/bin/bash

# Generate all app icons from the source icon
# This script regenerates icons for all platforms (macOS, Windows, Linux)
#
# Usage: ./scripts/generate_icons.sh

set -e

SOURCE="icons/icon.png"

if [ ! -f "$SOURCE" ]; then
    echo "Error: Source icon not found at $SOURCE"
    exit 1
fi

echo "Generating icons from $SOURCE..."

# Regenerate all PNG files in icons/ directory
for file in icons/*.png; do
    if [ "$file" != "$SOURCE" ]; then
        # Get dimensions from filename or existing file
        size=$(sips -g pixelWidth "$file" 2>/dev/null | tail -1 | awk '{print $2}')
        if [ -n "$size" ] && [ "$size" -gt 0 ]; then
            sips -z "$size" "$size" "$SOURCE" --out "$file" >/dev/null
            echo "Regenerated $file (${size}x${size})"
        fi
    fi
done

# Generate ico with multiple sizes (Windows)
echo "Generating icons/icon.ico..."
TEMP_DIR=$(mktemp -d)
sips -z 16 16 "$SOURCE" --out "$TEMP_DIR/16.png" >/dev/null
sips -z 32 32 "$SOURCE" --out "$TEMP_DIR/32.png" >/dev/null
sips -z 48 48 "$SOURCE" --out "$TEMP_DIR/48.png" >/dev/null
sips -z 64 64 "$SOURCE" --out "$TEMP_DIR/64.png" >/dev/null
sips -z 128 128 "$SOURCE" --out "$TEMP_DIR/128.png" >/dev/null
sips -z 256 256 "$SOURCE" --out "$TEMP_DIR/256.png" >/dev/null

# Use magick if available, otherwise skip ico generation
if command -v magick &> /dev/null; then
    magick "$TEMP_DIR/16.png" "$TEMP_DIR/32.png" "$TEMP_DIR/48.png" "$TEMP_DIR/64.png" "$TEMP_DIR/128.png" "$TEMP_DIR/256.png" -colors 256 icons/icon.ico
    echo "Generated icons/icon.ico"
else
    echo "Warning: ImageMagick not found, skipping .ico generation"
fi

rm -rf "$TEMP_DIR"

# Generate icns (macOS)
echo "Generating icons/icon.icns..."
ICONSET="temp.iconset"
mkdir -p "$ICONSET"

sips -z 16 16 "$SOURCE" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$SOURCE" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$SOURCE" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$SOURCE" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$SOURCE" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$SOURCE" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$SOURCE" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$SOURCE" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$SOURCE" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$SOURCE" --out "$ICONSET/icon_512x512@2x.png" >/dev/null

iconutil -c icns "$ICONSET" -o icons/icon.icns
rm -rf "$ICONSET"

echo "Generated icons/icon.icns"
echo ""
echo "Icon generation complete!"
echo "Note: For platform-specific icons, also run:"
echo "  ./scripts/generate-android-icons.sh"
echo "  ./scripts/generate-ios-icons.sh"
