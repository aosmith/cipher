# Cipher for Linux

Linux desktop application releases for Cipher.

## Latest Release

- **Version**: v0.5.10
- **DEB Package**: `cipher_0.5.10_amd64.deb` (~15MB)
- **AppImage**: `cipher_0.5.10_amd64.AppImage` (~18MB)
- **Requirements**: Ubuntu 20.04+ / Debian 11+ or equivalent

## Installation Options

### DEB Package (Recommended for Ubuntu/Debian)

```bash
# Download
wget https://github.com/aosmith/cipher/releases/download/v0.5.10/cipher_0.5.10_amd64.deb

# Install
sudo dpkg -i cipher_0.5.10_amd64.deb

# Fix dependencies if needed
sudo apt-get install -f
```

### AppImage (Universal)

```bash
# Download
wget https://github.com/aosmith/cipher/releases/download/v0.5.10/cipher_0.5.10_amd64.AppImage

# Make executable
chmod +x cipher_0.5.10_amd64.AppImage

# Run
./cipher_0.5.10_amd64.AppImage
```

## File Structure

```
linux/
├── latest/                        # Current release
│   ├── cipher_0.5.10_amd64.deb        # DEB package
│   ├── cipher_0.5.10_amd64.AppImage   # AppImage
│   ├── checksums.txt
│   └── CHANGELOG.md
├── archive/                      # Previous versions
│   └── v1.0.0/
└── README.md                    # This file
```

## System Requirements

- **Architecture**: x86_64 (64-bit)
- **Minimum**: Ubuntu 20.04, Debian 11, or equivalent
- **Dependencies**: GTK 3.0+, WebKit2GTK
- **RAM**: 4GB minimum
- **Disk**: 100MB free space

## Dependencies

Most modern Linux distributions have the required dependencies pre-installed. If not:

```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.0-37 libgtk-3-0 libappindicator3-1

# Fedora/CentOS
sudo dnf install webkit2gtk3 gtk3 libappindicator-gtk3

# Arch Linux
sudo pacman -S webkit2gtk gtk3 libappindicator-gtk3
```

## Desktop Integration

### DEB Package
- Automatically creates desktop entry
- Adds to application menu
- Registers MIME types
- System tray integration

### AppImage
- Portable, no installation required
- Manual desktop integration available
- Can be integrated with AppImageLauncher

## Troubleshooting

**Missing dependencies:**
```bash
# Check missing libraries
ldd ./cipher_0.5.10_amd64.AppImage

# Install WebKit2GTK
sudo apt install libwebkit2gtk-4.0-37
```

**AppImage won't run:**
- Ensure FUSE is installed: `sudo apt install fuse`
- Check executable permissions: `chmod +x *.AppImage`
- Try running from terminal to see error messages

**DEB installation fails:**
- Update package list: `sudo apt update`
- Fix broken dependencies: `sudo apt-get install -f`
- Check architecture: `dpkg --print-architecture` (should be amd64)

**System tray not working:**
- Install system tray extension (GNOME)
- Use TopIcons Plus extension
- Check if your DE supports system tray

## Distribution-Specific Notes

### Ubuntu
- Works on 20.04 LTS and newer
- Snap version planned for Ubuntu Software

### Debian
- Requires Debian 11 (Bullseye) or newer
- Testing/Unstable branches supported

### Fedora
- Works on Fedora 35+
- RPM package planned for future releases

### Arch Linux
- Available in AUR (planned)
- AppImage works on all Arch installations