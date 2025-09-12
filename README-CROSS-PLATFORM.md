# Cipher Desktop App - Cross-Platform Guide

Build and distribute Cipher as a native desktop application on Windows, macOS, and Linux using Tauri.

## Overview

The Cipher Desktop App provides a native experience across all major platforms:

- **Windows**: Native .msi installer with Windows-style UI
- **macOS**: .dmg installer with macOS-style window controls  
- **Linux**: .deb and .AppImage packages with Linux integration

## Platform Support Matrix

| Feature | Windows | macOS | Linux |
|---------|---------|--------|-------|
| Native Window | ✅ | ✅ | ✅ |
| System Tray | ✅ | ✅ | ✅ |
| Auto-updater | ✅ | ✅ | ✅ |
| Code Signing | ✅ | ✅ | ✅ |
| Notifications | ✅ | ✅ | ✅ |
| File Associations | ✅ | ✅ | ✅ |

## Prerequisites by Platform

### Windows
- **Windows 10/11** (64-bit)
- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **Node.js 16+**: [nodejs.org](https://nodejs.org/)
- **Ruby 3.0+**: [RubyInstaller](https://rubyinstaller.org/) for Windows
- **Visual Studio Build Tools**: For native compilation
- **Git**: [git-scm.com](https://git-scm.com/)

### macOS
- **macOS 10.13+** (High Sierra or later)
- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **Node.js 16+**: [nodejs.org](https://nodejs.org/)
- **Ruby 3.0+**: Use rbenv, rvm, or system Ruby
- **Xcode Command Line Tools**: `xcode-select --install`

### Linux (Ubuntu/Debian)
- **Ubuntu 20.04+** or **Debian 11+**
- **Dependencies**:
  ```bash
  sudo apt update
  sudo apt install curl wget file build-essential libssl-dev libgtk-3-dev libwebkit2gtk-4.0-dev libappindicator3-dev librsvg2-dev
  ```
- **Rust**: Install from [rustup.rs](https://rustup.rs/)
- **Node.js 16+**: Use NodeSource or snap
- **Ruby 3.0+**: Use rbenv or system package

## Development Setup

### 1. Clone and Setup
```bash
git clone https://github.com/aosmith/cipher.git
cd cipher
npm install
bundle install
```

### 2. Platform-Specific Development

#### Windows (Command Prompt/PowerShell)
```cmd
# Start development server
bin\desktop.bat

# Or use npm script
npm run dev:windows
```

#### macOS/Linux (Terminal)
```bash
# Start development server  
bin/desktop

# Or use npm script
npm run dev:unix
```

## Building for Distribution

### Single Platform Builds

#### Windows
```powershell
# Using PowerShell script (recommended)
.\scripts\build-desktop-windows.ps1

# Using batch file
.\scripts\build-desktop-windows.bat

# Using npm
npm run build:windows
```

#### macOS
```bash
# Using shell script
./scripts/build-desktop.sh

# Using npm  
npm run build:macos
```

#### Linux
```bash
# Install Linux dependencies first
sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libappindicator3-dev librsvg2-dev

# Build
npm run tauri:build:linux
```

### Cross-Platform Builds

Build for multiple platforms from a single machine (requires appropriate toolchains):

```bash
# Add Rust targets
rustup target add x86_64-pc-windows-msvc
rustup target add x86_64-apple-darwin  
rustup target add aarch64-apple-darwin
rustup target add x86_64-unknown-linux-gnu

# Build for specific targets
npm run tauri:build:windows
npm run tauri:build:macos
npm run tauri:build:macos-arm  
npm run tauri:build:linux
```

## Platform-Specific Features

### Windows
- **Installer**: Creates .msi installer with Windows Installer
- **Start Menu**: Automatically adds start menu shortcuts
- **Registry**: Registers app for file associations
- **Windows Store**: Can be packaged for Microsoft Store
- **Auto-start**: Optional Windows startup integration

### macOS  
- **Bundle**: Creates .app bundle and .dmg installer
- **Gatekeeper**: Supports notarization for security
- **Dock**: Native dock integration
- **App Store**: Can be packaged for Mac App Store
- **Spotlight**: Searchable via Spotlight

### Linux
- **Packages**: Creates .deb (Debian/Ubuntu) and .AppImage (universal)
- **Desktop Files**: Integrates with desktop environments
- **System Tray**: Works with GNOME, KDE, XFCE
- **Package Managers**: Compatible with apt, flatpak, snap

## Output Files

After building, you'll find platform-specific installers:

### Windows
```
src-tauri/target/release/bundle/
├── msi/
│   └── Cipher_0.5.10_x64_en-US.msi
└── nsis/
    └── Cipher_0.5.10_x64-setup.exe
```

### macOS
```
src-tauri/target/release/bundle/
├── macos/
│   └── Cipher.app
└── dmg/
    └── Cipher_0.5.10_x64.dmg
```

### Linux
```
src-tauri/target/release/bundle/
├── deb/
│   └── cipher_0.5.10_amd64.deb
└── appimage/
    └── cipher_0.5.10_amd64.AppImage
```

## Code Signing & Distribution

### Windows Code Signing
```powershell
# Get a code signing certificate
# Update tauri.conf.json:
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "YOUR_CERT_THUMBPRINT",
      "timestampUrl": "http://timestamp.digicert.com"
    }
  }
}
```

### macOS Code Signing
```bash
# Get Apple Developer Certificate
# Update tauri.conf.json:
{
  "bundle": {
    "macOS": {
      "signingIdentity": "Developer ID Application: Your Name",
      "providerShortName": "TEAM_ID"
    }
  }
}

# Notarize for Gatekeeper
xcrun notarytool submit Cipher.dmg --apple-id YOUR_ID --password APP_PASSWORD --team-id TEAM_ID
```

### Linux Signing
```bash
# Sign .deb package
debsign -k YOUR_GPG_KEY cipher_0.5.10_amd64.deb

# Sign .AppImage  
gpg --detach-sign cipher_0.5.10_amd64.AppImage
```

## Platform UI Differences

The app automatically detects the platform and applies appropriate styling:

### Windows
- Windows 11 style titlebar (32px height)
- Flat window controls (minimize, maximize, close)
- Windows-style scrollbars
- Blue accent color (#0078d4)

### macOS
- macOS style titlebar (30px height)  
- Traffic light controls (red, yellow, green circles)
- Native scrollbars
- System accent colors

### Linux
- Generic titlebar with standard controls
- GTK-compatible styling
- System theme integration

## Troubleshooting

### Common Build Issues

**Windows: "MSVC toolchain not found"**
```powershell
# Install Visual Studio Build Tools
# Or install Visual Studio Community
rustup toolchain install stable-x86_64-pc-windows-msvc
rustup default stable-x86_64-pc-windows-msvc
```

**macOS: "Xcode license not accepted"**
```bash
sudo xcodebuild -license accept
```

**Linux: "webkit2gtk not found"**
```bash
sudo apt install libwebkit2gtk-4.0-dev libgtk-3-dev
```

### Platform-Specific Debugging

**Windows:**
- Check Windows Event Viewer for app crashes
- Use Process Monitor for file/registry access issues
- Enable Windows Developer Mode for easier debugging

**macOS:**
- Check Console.app for system logs
- Use Activity Monitor for performance issues
- Check codesigning: `codesign -dv --verbose=4 Cipher.app`

**Linux:**
- Check journalctl for system logs: `journalctl -f`
- Use strace for system call debugging
- Check desktop file: `desktop-file-validate cipher.desktop`

## CI/CD Pipeline

Example GitHub Actions workflow for cross-platform builds:

```yaml
name: Build Desktop Apps
on: [push, pull_request]

jobs:
  build:
    strategy:
      matrix:
        platform: [macos-latest, ubuntu-20.04, windows-latest]
    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-20.04'
        run: |
          sudo apt update
          sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libappindicator3-dev librsvg2-dev
      
      - name: Install dependencies
        run: |
          npm install
          bundle install
          
      - name: Build app
        run: npm run tauri:build
        
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: desktop-app-${{ matrix.platform }}
          path: src-tauri/target/release/bundle/
```

## Distribution Strategies

### Windows
1. **Direct Download**: Host .msi files on your website
2. **Microsoft Store**: Package as MSIX for Store distribution
3. **Chocolatey**: Create Chocolatey package for developers
4. **Winget**: Submit to Windows Package Manager

### macOS
1. **Direct Download**: Host .dmg files with notarization
2. **Mac App Store**: Package for App Store distribution  
3. **Homebrew**: Create Homebrew cask formula
4. **SetApp**: Distribute via SetApp subscription service

### Linux
1. **Direct Download**: Host .deb and .AppImage files
2. **Package Repositories**: Submit to Ubuntu/Debian repos
3. **Flatpak**: Package for Flathub distribution
4. **Snap Store**: Create snap package for Ubuntu Store

## Performance Considerations

### Bundle Size Comparison
- **Windows**: ~12-15MB (vs ~120MB for Electron)
- **macOS**: ~10-12MB (vs ~150MB for Electron)
- **Linux**: ~15-18MB (vs ~130MB for Electron)

### Memory Usage
- **Tauri**: ~30-50MB RAM (uses system WebView)
- **Electron**: ~100-200MB RAM (bundles Chromium)

### Startup Time
- **Tauri**: ~1-2 seconds (native performance)
- **Electron**: ~3-5 seconds (JS bootstrap overhead)

## Security Features

- **Sandboxing**: Uses platform security features
- **Code Signing**: Verifiable authenticity
- **Auto-updates**: Secure update mechanism via GitHub releases
- **CSP**: Content Security Policy prevents XSS
- **Local-only Rails**: Server only accessible to app

## Future Enhancements

- [ ] iOS/Android support via Tauri Mobile
- [ ] Auto-updater with differential patches
- [ ] Multiple window support
- [ ] Plugin system for extensions
- [ ] Background sync capabilities