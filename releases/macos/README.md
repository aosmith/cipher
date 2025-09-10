# Cipher for macOS

macOS desktop application releases for Cipher.

## Latest Release

- **Version**: v0.5.1
- **Intel**: `Cipher_0.5.1_x64.dmg` (~10MB)
- **Apple Silicon**: `Cipher_0.5.1_aarch64.dmg` (~10MB)
- **Requirements**: macOS 10.13+ (High Sierra)

## Installation

1. Download the appropriate `.dmg` file for your Mac
2. Double-click to mount the disk image
3. Drag Cipher.app to Applications folder
4. Launch from Applications or Spotlight

## Architecture Support

- **Intel Macs**: Download `x64.dmg` version
- **Apple Silicon (M1/M2/M3)**: Download `aarch64.dmg` version
- **Universal**: Intel version runs on Apple Silicon via Rosetta 2

## File Structure

```
macos/
├── latest/                     # Current release
│   ├── Cipher_0.5.1_x64.dmg       # Intel Macs
│   ├── Cipher_0.5.1_aarch64.dmg   # Apple Silicon
│   ├── checksums.txt
│   └── CHANGELOG.md
├── archive/                   # Previous versions
│   └── v1.0.0/
└── README.md                 # This file
```

## Security Notes

- **Unsigned**: Current releases are not notarized or code-signed
- **Gatekeeper**: macOS will show security warnings
- **Bypass**: Right-click app → "Open" to run unsigned apps
- **Future**: Code signing and notarization planned for future releases

## Troubleshooting

**"App can't be opened" error:**
1. Right-click Cipher.app
2. Select "Open" from context menu
3. Click "Open" in the security dialog

**App won't start:**
- Check macOS version (10.13+)
- Ensure app is in Applications folder
- Check System Preferences → Security & Privacy

**Performance issues:**
- Intel apps on Apple Silicon use Rosetta 2
- Download native Apple Silicon version for best performance