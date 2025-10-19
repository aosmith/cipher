#!/bin/bash

set -e

SOURCE="icon.png"

# Regenerate all PNG files in icons/

find icons -type f -name "*.png" | while read file; do
  size=$(magick identify -format "%[width]x%[height]" "$file")
  magick convert "$SOURCE" -resize "$size" "$file"
  echo "Regenerated $file to $size"
done

# Generate ico with multiple sizes
magick convert "$SOURCE" \
  -resize 16x16 temp16.png \
  -resize 32x32 temp32.png \
  -resize 48x48 temp48.png \
  -resize 64x64 temp64.png \
  -resize 128x128 temp128.png \
  -resize 256x256 temp256.png

magick convert temp16.png temp32.png temp48.png temp64.png temp128.png temp256.png -colors 256 icons/icon.ico

rm temp*.png

echo "Generated icons/icon.ico"

# Generate icns
ICONSET="temp.iconset"
mkdir -p "$ICONSET"

magick convert "$SOURCE" -resize 16x16 "$ICONSET/icon_16x16.png"
magick convert "$SOURCE" -resize 32x32 "$ICONSET/icon_16x16@2x.png"
magick convert "$SOURCE" -resize 32x32 "$ICONSET/icon_32x32.png"
magick convert "$SOURCE" -resize 64x64 "$ICONSET/icon_32x32@2x.png"
magick convert "$SOURCE" -resize 128x128 "$ICONSET/icon_128x128.png"
magick convert "$SOURCE" -resize 256x256 "$ICONSET/icon_128x128@2x.png"
magick convert "$SOURCE" -resize 256x256 "$ICONSET/icon_256x256.png"
magick convert "$SOURCE" -resize 512x512 "$ICONSET/icon_256x256@2x.png"
magick convert "$SOURCE" -resize 512x512 "$ICONSET/icon_512x512.png"
magick convert "$SOURCE" -resize 1024x1024 "$ICONSET/icon_512x512@2x.png"

iconutil -c icns "$ICONSET" -o icons/icon.icns

rm -rf "$ICONSET"

echo "Generated icons/icon.icns"