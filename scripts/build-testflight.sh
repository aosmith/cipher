#!/bin/bash
set -e

echo "Building Cipher for TestFlight..."

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
SCHEME="cipher-social_iOS"
PROJECT="gen/apple/cipher-social.xcodeproj"
ARCHIVE_PATH="build/cipher-social.xcarchive"
EXPORT_PATH="build/testflight"
EXPORT_OPTIONS="gen/apple/ExportOptions.plist"

echo -e "${BLUE}Step 1: Cleaning previous builds...${NC}"
rm -rf build/
mkdir -p build/testflight

echo -e "${BLUE}Step 2: Building Rust library...${NC}"
export PATH="/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$HOME/.cargo/bin"
export SDKROOT=$(xcrun --sdk iphoneos --show-sdk-path)
export IPHONEOS_DEPLOYMENT_TARGET=14.0
cargo build --target aarch64-apple-ios --lib --release

echo -e "${BLUE}Step 3: Copying library to Xcode...${NC}"
mkdir -p gen/apple/Externals/arm64/Release
cp target/aarch64-apple-ios/release/libapp.a gen/apple/Externals/arm64/Release/libapp.a

echo -e "${BLUE}Step 4: Building archive...${NC}"
xcodebuild archive \
  -project "$PROJECT" \
  -scheme "$SCHEME" \
  -archivePath "$ARCHIVE_PATH" \
  -destination "generic/platform=iOS" \
  -configuration Release \
  -allowProvisioningUpdates \
  CODE_SIGN_STYLE=Automatic \
  DEVELOPMENT_TEAM=2AYYQP7AV8

if [ $? -ne 0 ]; then
    echo -e "${RED}Archive build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}Archive created successfully!${NC}"

echo -e "${BLUE}Step 5: Exporting IPA for TestFlight...${NC}"
xcodebuild -exportArchive \
  -archivePath "$ARCHIVE_PATH" \
  -exportPath "$EXPORT_PATH" \
  -exportOptionsPlist "$EXPORT_OPTIONS"

if [ $? -ne 0 ]; then
    echo -e "${RED}Export failed!${NC}"
    exit 1
fi

echo -e "${GREEN}IPA exported successfully!${NC}"
echo -e "${BLUE}Location: $EXPORT_PATH/cipher-social.ipa${NC}"

echo ""
echo -e "${GREEN}Build complete! Next steps:${NC}"
echo "1. Upload to TestFlight using Xcode or Transporter app"
echo "2. Or use: xcrun altool --upload-app -f build/testflight/cipher-social.ipa -t ios --apiKey YOUR_API_KEY --apiIssuer YOUR_ISSUER_ID"
echo ""
