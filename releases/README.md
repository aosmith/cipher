# Cipher Releases

Pre-compiled binaries for all platforms. Current version: v0.1.11

## Directory Structure

```
releases/
├── macos/
│   └── latest/
│       └── Cipher.dmg           # macOS Universal Binary (Intel + Apple Silicon)
├── windows/
│   └── latest/
│       └── Cipher.msi           # Windows Installer (x64)
├── linux/
│   └── latest/
│       ├── Cipher.deb           # Debian/Ubuntu Package (ARM64)
│       └── Cipher.rpm           # Fedora/RedHat/CentOS Package (ARM64)
├── android/
│   └── latest/
│       └── Cipher.apk           # Android APK (Universal)
└── ios/
    └── latest/
        └── Cipher.ipa           # iOS App (Requires signing)
```

## Download Latest Version

- **macOS**: [Cipher.dmg](macos/latest/Cipher.dmg)
- **Windows**: [Cipher.msi](windows/latest/Cipher.msi) *(Coming soon)*
- **Linux Debian/Ubuntu (ARM64)**: [Cipher.deb](linux/latest/Cipher.deb)
- **Linux Fedora/RedHat (ARM64)**: [Cipher.rpm](linux/latest/Cipher.rpm)
- **Android**: [Cipher.apk](android/latest/Cipher.apk)
- **iOS**: [Cipher.ipa](ios/latest/Cipher.ipa)

## Installation Instructions

### macOS
1. Download `Cipher.dmg` from `macos/latest/`
2. Double-click the DMG file
3. Drag Cipher to Applications folder
4. On first launch, right-click and select "Open" to bypass Gatekeeper

### Windows
1. Download `Cipher.msi` from `windows/latest/`
2. Double-click the MSI installer
3. Follow the installation wizard
4. Launch from Start Menu or Desktop shortcut

### Linux (ARM64)
**Note**: Currently only ARM64/aarch64 packages are available. x86_64 packages require building on an x86_64 Linux machine.

**Debian/Ubuntu:**
1. Download `Cipher.deb` from `linux/latest/`
2. Install: `sudo dpkg -i Cipher.deb`
3. Launch from applications menu

**Fedora/RedHat/CentOS:**
1. Download `Cipher.rpm` from `linux/latest/`
2. Install: `sudo rpm -i Cipher.rpm` or `sudo dnf install Cipher.rpm`
3. Launch from applications menu

### Android
1. Download `Cipher.apk` from `android/latest/`
2. Enable "Install from Unknown Sources" in Settings
3. Open the APK file to install
4. Launch from app drawer

### iOS
1. Download `Cipher.ipa` from `ios/latest/`
2. Requires sideloading via:
   - AltStore/Sideloadly (Personal use)
   - TestFlight (Beta testing)
   - Enterprise distribution certificate

## System Requirements

### Desktop
- **macOS**: 10.15 Catalina or later
- **Windows**: Windows 10 1903 or later (64-bit)
- **Linux**: Ubuntu 20.04+ or equivalent

### Mobile
- **Android**: Android 7.0 (API 24) or later
- **iOS**: iOS 14.0 or later

## Building from Source

If you prefer to build from source, see the main [README.md](../README.md) for build instructions.

## Support

For issues or questions:
- GitHub Issues: [github.com/aosmith/cipher/issues](https://github.com/aosmith/cipher/issues)
- Main README: [../README.md](../README.md)