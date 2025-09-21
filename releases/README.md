# Cipher Desktop App Releases

Pre-compiled desktop applications for Windows, macOS, and Linux.

> ✅ **Build Status Update**: macOS ARM64 build is now working! Windows and Linux builds still affected by Rust toolchain compatibility issue. See [BUILD-ISSUES.md](../BUILD-ISSUES.md) for technical details.

## Quick Download

### Latest Version

Grab the newest installers from [GitHub Releases](https://github.com/aosmith/cipher/releases/latest).

| Platform | Download | Size | SHA256 |
|----------|----------|------|---------|
| **Windows** | [Latest Windows installer](https://github.com/aosmith/cipher/releases/latest) | ~12MB | `placeholder` |
| **macOS (ARM64)** | [Latest macOS build](https://github.com/aosmith/cipher/releases/latest) | 2.7MB | `55ae616b236c...` |
| **Linux (DEB)** | [Latest Linux DEB](https://github.com/aosmith/cipher/releases/latest) | ~15MB | `placeholder` |
| **Linux (AppImage)** | [Latest Linux AppImage](https://github.com/aosmith/cipher/releases/latest) | ~18MB | `placeholder` |

## Installation Instructions

### Windows
1. Download the `.msi` file
2. Double-click to run the installer
3. Follow the installation wizard
4. Launch from Start Menu or Desktop

### macOS
1. Download the `.dmg` file for your architecture (currently ARM64/Apple Silicon only)
2. Double-click to mount the disk image
3. Drag Cipher.app to your Applications folder
4. Launch from Applications or Spotlight

**Note**: macOS may show a security warning for unsigned apps. Right-click the app and select "Open" to bypass Gatekeeper.

**Current Release**: ARM64 (Apple Silicon) only. Intel x64 support requires cross-compilation setup.

### Linux

#### DEB Package (Ubuntu/Debian)
1. Download the `.deb` package from the [latest release](https://github.com/aosmith/cipher/releases/latest)
2. Install with `sudo dpkg -i cipher_<version>_amd64.deb`
3. Fix missing dependencies with `sudo apt-get install -f`

#### AppImage (Universal)
```bash
# Download and make executable
# Get the latest AppImage from https://github.com/aosmith/cipher/releases/latest
chmod +x Cipher_<version>_x86_64.AppImage

# Run directly
./Cipher_<version>_x86_64.AppImage
```

## System Requirements

### Windows
- Windows 10 (64-bit) or later
- 4GB RAM minimum
- 100MB free disk space

### macOS
- macOS 10.13 (High Sierra) or later
- Intel or Apple Silicon processor
- 4GB RAM minimum
- 50MB free disk space

### Linux
- Ubuntu 20.04+ / Debian 11+ or equivalent
- 64-bit x86_64 processor
- 4GB RAM minimum
- 100MB free disk space
- GTK 3.0+ and WebKit2GTK (usually pre-installed)

## Features

- **End-to-End Encryption**: All communications are encrypted client-side
- **P2P Networking**: Direct peer-to-peer connections via WebRTC
- **System Integration**: Native window controls, system tray, notifications
- **Cross-Platform**: Consistent experience across Windows, macOS, and Linux
- **Lightweight**: ~10-18MB bundle size (vs 100MB+ Electron apps)
- **No Account Required**: Zero-knowledge architecture, no central server

## Security Notes

- **Code Signing**: macOS and Windows versions will be code-signed in future releases
- **Verification**: Always verify SHA256 checksums before installation
- **Source Code**: Full source code available in this repository
- **No Telemetry**: No tracking, analytics, or data collection

## Version History

### v0.5.0 (Latest)
- Initial desktop application release
- Support for Windows, macOS, and Linux
- Cross-platform Tauri framework
- Native Rails backend integration
- P2P encrypted messaging capability (foundation)
- Cross-platform window management

**Note**: macOS ARM64 binary is functional. Windows/Linux binaries are placeholders due to build issues. See [BUILD-ISSUES.md](../BUILD-ISSUES.md) for details.

## Troubleshooting

### Common Issues

**Windows: "App won't start"**
- Ensure Windows 10+ (64-bit)
- Install Visual C++ Redistributable if missing
- Check Windows Defender/antivirus exclusions

**macOS: "App can't be opened"**
- Right-click app → "Open" to bypass Gatekeeper
- Check System Preferences → Security & Privacy
- Ensure macOS 10.13+ compatibility

**Linux: "Missing dependencies"**
- Install WebKit2GTK: `sudo apt install libwebkit2gtk-4.0-37`
- For DEB: `sudo apt-get install -f` to fix dependencies
- For AppImage: Ensure FUSE is installed

### Getting Help

- **Issues**: [GitHub Issues](https://github.com/aosmith/cipher/issues)
- **Documentation**: See [README-DESKTOP.md](../README-DESKTOP.md)
- **Source Code**: Build from source using [README-CROSS-PLATFORM.md](../README-CROSS-PLATFORM.md)

## Building from Source

If you prefer to build from source or need a different platform:

1. **Clone repository**: `git clone https://github.com/aosmith/cipher.git`
2. **Follow build guide**: See [README-CROSS-PLATFORM.md](../README-CROSS-PLATFORM.md)
3. **Platform-specific builds**: Use provided build scripts in `scripts/`

## License

Same as main Cipher project - see [LICENSE](../LICENSE) file.

---

**Disclaimer**: These are pre-release builds. While functional, they may contain bugs. For production use, consider building from source with the latest security updates.
