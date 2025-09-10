# Cipher Desktop App

A cross-platform native desktop application for Cipher, the end-to-end encrypted P2P social network. Available for Windows, macOS, and Linux.

## Overview

The Cipher Desktop App packages the full Rails application into a native macOS app using Tauri. This provides:

- **Native Performance**: Uses system WebView (Safari engine) instead of bundling Chromium
- **Small Bundle Size**: ~10-15MB vs ~100MB+ for Electron apps
- **System Integration**: Native window controls, system tray, notifications
- **Security**: Runs Rails server locally with desktop-specific security settings

## Prerequisites

### All Platforms
- **Rust** - Install from [rustup.rs](https://rustup.rs/)
- **Node.js 16+** - Install from [nodejs.org](https://nodejs.org/)
- **Ruby 3.0+** - Platform-specific installation

### Windows
- **Windows 10/11** (64-bit)
- **Visual Studio Build Tools** or Visual Studio Community
- **Ruby**: [RubyInstaller](https://rubyinstaller.org/) for Windows

### macOS  
- **macOS 10.13+** (High Sierra or later)
- **Xcode Command Line Tools** - `xcode-select --install`
- **Ruby**: Use rbenv, rvm, or system Ruby

### Linux
- **Ubuntu 20.04+** or similar
- **Build dependencies**:
  ```bash
  sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libappindicator3-dev librsvg2-dev
  ```

## Quick Start

### Development Mode

**Windows:**
```cmd
bin\desktop.bat
```

**macOS/Linux:**
```bash
bin/desktop
```

This will:
- Install dependencies
- Set up the database  
- Start Rails server on port 3001
- Launch the desktop app with hot reload

### Building for Distribution

**Windows:**
```powershell
.\scripts\build-desktop-windows.ps1
# or
.\scripts\build-desktop-windows.bat
```

**macOS:**
```bash
./scripts/build-desktop.sh
```

**Linux:**
```bash
npm run tauri:build:linux
```

**Find your built app:**
- **Windows**: `src-tauri/target/release/bundle/msi/`
- **macOS**: `src-tauri/target/release/bundle/macos/` and `dmg/`  
- **Linux**: `src-tauri/target/release/bundle/deb/` and `appimage/`

For detailed cross-platform instructions, see [README-CROSS-PLATFORM.md](README-CROSS-PLATFORM.md).

## Features

### Desktop-Specific Features

- **Custom Title Bar**: Integrated window controls with app branding
- **System Tray**: Minimize to system tray with quick actions
- **Hide vs Close**: Clicking X hides the app instead of closing it
- **Native Shortcuts**: Standard macOS keyboard shortcuts
- **External Link Handling**: Opens web links in default browser
- **Auto-start Rails**: Automatically starts and manages Rails server

### Rails Integration

- **Desktop Environment**: Special Rails environment with desktop optimizations
- **Local Database**: SQLite database stored locally
- **Asset Pipeline**: Precompiled assets for faster loading
- **CSRF Disabled**: Desktop app doesn't need CSRF protection
- **Custom CSP**: Content Security Policy optimized for Tauri

## Architecture

```
┌─────────────────────────────────┐
│           Tauri App             │
│  ┌─────────────────────────────┐│
│  │      WebView (Safari)       ││
│  │  ┌─────────────────────────┐││
│  │  │     Rails App           │││
│  │  │   (localhost:3001)      │││
│  │  │                         │││
│  │  │  - Hotwire Frontend     │││
│  │  │  - P2P Networking       │││
│  │  │  - Encryption Layer     │││
│  │  └─────────────────────────┘││
│  └─────────────────────────────┘│
│                                 │
│  Rust Backend:                  │
│  - Window Management            │
│  - System Tray                  │
│  - Rails Process Management     │
│  - File System Access          │
└─────────────────────────────────┘
```

## File Structure

```
cipher/
├── src-tauri/                 # Tauri desktop app
│   ├── Cargo.toml            # Rust dependencies
│   ├── tauri.conf.json       # App configuration
│   ├── src/main.rs           # Main Rust code
│   ├── icons/                # App icons
│   └── target/               # Build output
├── config/environments/
│   └── desktop.rb            # Desktop Rails environment
├── app/assets/stylesheets/
│   └── desktop.css           # Desktop-specific styles
├── scripts/
│   └── build-desktop.sh      # Build script
├── bin/desktop               # Development launcher
└── package.json              # Tauri CLI dependencies
```

## Configuration

### App Settings

Edit `src-tauri/tauri.conf.json` to customize:

- **Window size and behavior**
- **App metadata** (name, version, description)
- **System tray settings**
- **Security permissions**
- **Bundle configuration**

### Rails Settings

Edit `config/environments/desktop.rb` for:

- **Database configuration**
- **Asset pipeline settings**
- **Security policies**
- **Logging levels**

## Building Icons

The app requires several icon sizes. Create these from a 1024x1024 PNG:

```bash
# Required icons in src-tauri/icons/:
32x32.png           # Small icon
128x128.png         # Medium icon  
128x128@2x.png      # High-DPI medium
icon.icns           # macOS app icon
icon.ico            # Windows icon (if building for Windows)
```

**Online tools for icon generation:**
- [IconGenerator](https://icongenerator.net/)
- [App Icon Generator](https://appicon.co/)
- Command line: `iconutil` (included with Xcode)

## Distribution

### Code Signing (Optional)

For App Store or notarized distribution:

1. **Get Apple Developer Certificate**
2. **Update `tauri.conf.json`:**
   ```json
   "macOS": {
     "signingIdentity": "Your Developer ID",
     "providerShortName": "YourTeamID"
   }
   ```

### Manual Distribution

1. **Build the app** using the build script
2. **Test the `.app` bundle** on different Macs
3. **Distribute the `.dmg` file** for easy installation

## Development Tips

### Debugging

- **Rust logs**: Check terminal output when running `bin/desktop`
- **Rails logs**: `tail -f log/desktop.log`
- **WebView debugging**: Enable in `tauri.conf.json` → `build` → `devPath`

### Hot Reload

Development mode supports:
- **Rails hot reload**: Code changes refresh automatically
- **Frontend hot reload**: CSS/JS changes update instantly
- **Rust hot reload**: Tauri app restarts on Rust changes

### Performance

- **Database**: SQLite is optimized for desktop use
- **Assets**: Precompiled for faster loading
- **Memory**: Uses system WebView (lower RAM usage)
- **CPU**: Native Rust backend (efficient)

## Troubleshooting

### Common Issues

**Build fails with Xcode errors:**
```bash
xcode-select --install
sudo xcodebuild -license accept
```

**Rails server won't start:**
```bash
# Check Ruby version
ruby --version

# Reinstall gems
bundle install
```

**App won't launch:**
```bash
# Check permissions
chmod +x bin/desktop

# Check Tauri installation
npm run tauri --version
```

**Database issues:**
```bash
# Reset desktop database
rm storage/desktop.sqlite3
bin/rails db:prepare RAILS_ENV=desktop
```

### Getting Help

- **Tauri Documentation**: [tauri.app](https://tauri.app)
- **Rails Guides**: [guides.rubyonrails.org](https://guides.rubyonrails.org)
- **Project Issues**: [GitHub Issues](https://github.com/aosmith/cipher/issues)

## Security Notes

The desktop app includes several security considerations:

- **Local-only Rails server** (port 3001, localhost only)
- **Desktop-specific CSP** allows Tauri APIs
- **CSRF disabled** (not needed for desktop)
- **System tray** prevents accidental closure
- **External links** open in system browser for security

## License

Same as main Cipher project - see LICENSE file.