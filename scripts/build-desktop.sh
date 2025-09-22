#!/bin/bash

# Build script for Cipher Desktop App
set -e

echo "🔐 Building Cipher Desktop App for macOS"

# Check if we're on macOS
if [[ "$OSTYPE" != "darwin"* ]]; then
  echo "❌ This script is designed for macOS. Please run on a Mac."
  exit 1
fi

# Check prerequisites
echo "📋 Checking prerequisites..."

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install from https://rustup.rs/"
    exit 1
fi

# Check Node.js
if ! command -v node &> /dev/null; then
    echo "❌ Node.js not found. Please install from https://nodejs.org/"
    exit 1
fi

# Check Ruby
if ! command -v ruby &> /dev/null; then
    echo "❌ Ruby not found. Please install Ruby."
    exit 1
fi

# Check Bundler
if ! command -v bundle &> /dev/null; then
    echo "❌ Bundler not found. Installing..."
    gem install bundler
fi

echo "✅ Prerequisites checked"

# Install dependencies
echo "📦 Installing dependencies..."
npm install
bundle install

# Prepare Rails app
echo "🚂 Preparing Rails application..."
export RAILS_ENV=desktop

echo "🧹 Resetting desktop database..."
bundle exec rails db:reset

echo "🧼 Clearing build artifacts and storage..."
bundle exec rails tmp:clear
rm -rf storage/*
rm -rf public/assets

APP_SUPPORT_DIR="$HOME/Library/Application Support/com.cipher.social"
INSTALLED_APP="$HOME/Applications/Cipher.app"

echo "🗑️ Removing cached desktop data..."
rm -rf "$APP_SUPPORT_DIR"

if [ -d "$INSTALLED_APP" ]; then
  echo "🗑️ Removing previously installed Cipher.app..."
  rm -rf "$INSTALLED_APP"
fi

echo "🛠️ Running migrations and precompiling assets..."
bundle exec rails db:prepare
bundle exec rails assets:precompile

# Generate app icons if they don't exist
echo "🎨 Checking app icons..."
if [ ! -f "src-tauri/icons/icon.icns" ]; then
  echo "⚠️  App icons not found. You'll need to add icons to src-tauri/icons/"
  echo "   Required: 32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico"
  echo "   You can generate these from a 1024x1024 PNG using online tools"
  echo "   Continuing with build anyway..."
fi

# Build desktop app
echo "🔨 Building desktop application..."
npm run tauri:build

# Check if build succeeded
if [ -d "src-tauri/target/release/bundle/macos" ]; then
  echo "✅ Build completed successfully!"
  echo "📱 Desktop app created at: src-tauri/target/release/bundle/macos/"
  echo "💾 You can find the .dmg installer in the same directory"
  
  # Optional: Open the build directory
  read -p "🔍 Open build directory? (y/n): " -n 1 -r
  echo
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    open src-tauri/target/release/bundle/macos/
  fi
else
  echo "❌ Build failed. Check the output above for errors."
  exit 1
fi

echo "🎉 Cipher Desktop App build complete!"
