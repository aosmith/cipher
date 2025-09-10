# Cipher for Windows

Windows desktop application releases for Cipher.

## Latest Release

- **Version**: v0.5.0
- **File**: `Cipher_0.5.0_x64.msi`
- **Size**: ~12MB
- **Requirements**: Windows 10+ (64-bit)

## Installation

1. Download the `.msi` installer
2. Double-click to run
3. Follow the installation wizard
4. Launch from Start Menu

## File Structure

```
windows/
├── latest/                 # Current release
│   ├── Cipher_0.5.0_x64.msi
│   ├── checksums.txt
│   └── CHANGELOG.md
├── archive/               # Previous versions
│   └── v1.0.0/
└── README.md             # This file
```

## Security

- Files are scanned for malware before upload
- SHA256 checksums provided in `checksums.txt`
- Future releases will be code-signed

## Troubleshooting

**Installation fails:**
- Run as Administrator
- Disable antivirus temporarily
- Ensure Windows 10+ (64-bit)

**App won't start:**
- Install Visual C++ Redistributable
- Check Windows Defender exclusions
- Restart after installation