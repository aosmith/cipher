# Cipher for iOS

iOS mobile application releases for Cipher.

## Latest Release

- **Download**: [GitHub Releases (iOS builds)](https://github.com/aosmith/cipher/releases/latest)
- **Version**: v0.7.0
- **Format**: iOS App (`.ipa` for distribution, Xcode project for development)
- **Minimum iOS**: 13.0+
- **Devices**: iPhone, iPad, iPod Touch

## Installation

### Option 1: TestFlight (Beta)
*Coming soon - TestFlight distribution will be available once Apple Developer account is configured*

### Option 2: Xcode Development
1. Clone the repository and open in Xcode
2. Build and run on device/simulator
3. Requires Apple Developer account for device installation

```bash
# Clone and build
git clone https://github.com/aosmith/cipher.git
cd cipher

# Build iOS app (requires macOS + Xcode)
./scripts/build-mobile.sh ios
```

### Option 3: Sideloading (Advanced)
1. Use tools like AltStore or Sideloadly
2. Install the `.ipa` file directly
3. Requires periodic re-signing

## Requirements

- **iOS Version**: 13.0 or later
- **Devices**: iPhone 6s/SE or newer, iPad Air 2 or newer
- **Storage**: 50MB free space minimum
- **Network**: Wi-Fi or cellular data for P2P connections

## File Structure

```
ios/
├── latest/                     # Current release
│   ├── Cipher-0.7.0.ipa           # iOS App Bundle (when available)
│   ├── checksums.txt
│   └── CHANGELOG.md
├── archive/                   # Previous versions
└── README.md                 # This file
```

## Development

To build from source on macOS:

1. **Requirements**:
   - macOS with Xcode 14.0+
   - iOS SDK 13.0+
   - Apple Developer account (for device testing)

2. **Setup**:
   ```bash
   # Install Xcode command line tools
   xcode-select --install
   
   # Install CocoaPods (if needed)
   sudo gem install cocoapods
   ```

3. **Build**:
   ```bash
   ./scripts/dev-mobile.sh ios      # Development with simulator
   ./scripts/build-mobile.sh ios    # Production build
   ```

## Security & Privacy

- 🔐 **End-to-End Encryption**: All data encrypted with zero-knowledge architecture
- 🛡️ **Privacy First**: No data collection or tracking
- 📱 **iOS Security**: Leverages iOS security features (Keychain, App Sandbox)
- 🔒 **Biometric Auth**: Face ID/Touch ID support for wallet access
- 🚫 **No Analytics**: Zero telemetry or user tracking

## iOS-Specific Features

- 📱 **Native iOS Interface**: Following Human Interface Guidelines
- ⚡ **iOS Shortcuts**: Siri Shortcuts integration
- 📋 **iOS Share Sheet**: Native sharing capabilities  
- 🔔 **Push Notifications**: Real-time P2P message notifications
- 📱 **Widget Support**: Home screen widgets for quick access
- 🎨 **Dark Mode**: Automatic dark/light theme switching
- ♿ **Accessibility**: VoiceOver and Dynamic Type support

## App Store

*Note: App Store distribution is planned for future releases. Currently in development/beta phase.*

The app will be submitted to the App Store once:
- ✅ Core functionality is stable
- ✅ iOS Human Interface Guidelines compliance
- ✅ App Store Review Guidelines compliance
- ⏳ Apple Developer Program enrollment (in progress)

## Troubleshooting

**Installation issues:**
- Ensure iOS 13.0+ is installed
- Check available storage space
- For sideloading: verify signing certificate validity

**App crashes on startup:**
- Restart the device
- Check iOS version compatibility
- Clear app cache in Settings → General → iPhone Storage

**Connection problems:**
- Check network connectivity
- Verify cellular/Wi-Fi permissions
- Try different network (cellular ↔ Wi-Fi)
- Check firewall settings if on enterprise network

**Wallet not connecting:**
- Ensure crypto wallet app is installed
- Check WalletConnect compatibility
- Verify network selection (mainnet/testnet)

## Beta Testing

Interested in beta testing? Join our TestFlight beta:
*Link will be available once TestFlight is configured*

Provide feedback via:
- GitHub Issues
- In-app feedback feature
- Community Discord (link in app)
