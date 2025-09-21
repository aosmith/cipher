# Cipher for Android

Android mobile application releases for Cipher.

## Latest Release

- **Download**: [GitHub Releases (APK / AAB)](https://github.com/aosmith/cipher/releases/latest)
- **Version**: v0.6.5
- **APK**: Build from source using commands below
- **Minimum Android**: API level 24 (Android 7.0)

## Installation

### Option 1: Direct APK Install
1. Build the APK using the development instructions below
2. Enable "Unknown sources" in Android Settings → Security
3. Open the APK file and tap "Install"
4. Launch from your app drawer

### Option 2: Development Install
```bash
# Clone the repository
git clone https://github.com/aosmith/cipher.git
cd cipher

# Install Android SDK and set ANDROID_HOME
# Then build and install
./scripts/build-mobile.sh android
```

## Requirements

- **Android Version**: 7.0 (API level 24) or higher
- **Permissions**: 
  - Internet access (for P2P communication)
  - Storage access (for encrypted data)
  - Network state (for connection status)

## File Structure

```
android/
├── latest/                     # Current release
│   ├── build-instructions.md      # Android build guide
│   ├── checksums.txt
│   └── CHANGELOG.md
├── archive/                   # Previous versions
└── README.md                 # This file
```

## Development

To build from source:

1. **Install Android SDK**:
   - Download Android Studio or command line tools
   - Set `ANDROID_HOME` environment variable
   - Install API level 24+ SDK

2. **Build**:
   ```bash
   ./scripts/dev-mobile.sh android    # Development
   ./scripts/build-mobile.sh android  # Production build
   ```

## Security Notes

- **Unsigned APK**: Current releases are not signed with a production certificate
- **Unknown Sources**: Android will show security warnings for unsigned APKs
- **Data Encryption**: All user data is encrypted end-to-end
- **P2P Network**: Communication is peer-to-peer with zero-knowledge architecture

## Troubleshooting

**Installation fails:**
- Enable "Install unknown apps" for your browser/file manager
- Check that your Android version is 7.0+
- Clear cache and try again

**App won't start:**
- Check that you have internet connectivity
- Ensure sufficient storage space (50MB+)
- Restart your device if needed

**Connection issues:**
- Check firewall/network settings
- Try connecting to a different network
- Verify P2P ports aren't blocked

## Features

- 📱 **Native Mobile UI**: Touch-optimized interface designed for mobile
- 🔐 **End-to-End Encryption**: Zero-knowledge security
- 👥 **P2P Social Network**: Direct peer connections
- 💰 **Blockchain Integration**: Cryptocurrency wallet support
- 📄 **File Sharing**: Encrypted document and media sharing
- 🌐 **Offline-First**: Works without constant internet connection
