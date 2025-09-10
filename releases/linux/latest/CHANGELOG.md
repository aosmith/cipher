# Cipher Desktop v0.5.0 - Linux

## What's New

🚀 **Cross-Platform Desktop Support**
- Added native Linux desktop application using Tauri
- Multiple distribution formats: DEB package and AppImage
- GTK-based native window styling
- Integrated Rails backend with desktop WebView frontend

🔧 **Technical Improvements**
- Rails 8.0.2 with desktop environment configuration
- Automated build and release pipeline
- Linux-specific system integrations

## Installation Requirements

### DEB Package (Ubuntu/Debian)
- Ubuntu 20.04+ or Debian 11+
- ~15MB free disk space

### AppImage (Universal Linux)
- Any modern Linux distribution
- glibc 2.31 or newer
- ~15MB free disk space

## Known Issues

- Build currently blocked by Rust dependency compatibility (see BUILD-ISSUES.md)
- Actual binaries will be available when build issues are resolved

## Previous Versions

See archive/ directory for older releases.