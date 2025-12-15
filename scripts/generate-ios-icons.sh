#!/bin/bash

# Generate iOS app icons from the source icon
# This script should be run whenever the icon.png is updated
#
# Usage: ./scripts/generate-ios-icons.sh

set -e

ICON_SOURCE="icons/icon.png"
IOS_ASSETS="gen/apple/Assets.xcassets/AppIcon.appiconset"

if [ ! -f "$ICON_SOURCE" ]; then
    echo "Error: Source icon not found at $ICON_SOURCE"
    exit 1
fi

if [ ! -d "$IOS_ASSETS" ]; then
    echo "Error: iOS assets directory not found at $IOS_ASSETS"
    exit 1
fi

echo "Generating iOS app icons from $ICON_SOURCE..."

# iPhone icons
sips -z 40 40 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-20x20@2x.png"
sips -z 40 40 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-20x20@2x-1.png"
sips -z 60 60 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-20x20@3x.png"
sips -z 58 58 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-29x29@2x.png"
sips -z 58 58 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-29x29@2x-1.png"
sips -z 87 87 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-29x29@3x.png"
sips -z 80 80 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-40x40@2x.png"
sips -z 80 80 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-40x40@2x-1.png"
sips -z 120 120 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-40x40@3x.png"
sips -z 120 120 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-60x60@2x.png"
sips -z 180 180 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-60x60@3x.png"

# iPad icons
sips -z 20 20 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-20x20@1x.png"
sips -z 29 29 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-29x29@1x.png"
sips -z 40 40 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-40x40@1x.png"
sips -z 76 76 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-76x76@1x.png"
sips -z 152 152 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-76x76@2x.png"
sips -z 167 167 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-83.5x83.5@2x.png"

# App Store icon (1024x1024)
sips -z 1024 1024 "$ICON_SOURCE" --out "$IOS_ASSETS/AppIcon-512@2x.png"

echo "iOS app icons generated successfully!"
