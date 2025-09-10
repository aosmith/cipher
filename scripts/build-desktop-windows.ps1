# Build script for Cipher Desktop App on Windows
# PowerShell script

param(
    [switch]$Debug,
    [switch]$SkipRails,
    [string]$Target = "x86_64-pc-windows-msvc"
)

Write-Host "🔐 Building Cipher Desktop App for Windows" -ForegroundColor Blue

# Check prerequisites
Write-Host "📋 Checking prerequisites..." -ForegroundColor Yellow

# Check Rust
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "❌ Rust/Cargo not found. Please install from https://rustup.rs/" -ForegroundColor Red
    exit 1
}

# Check Node.js
if (!(Get-Command node -ErrorAction SilentlyContinue)) {
    Write-Host "❌ Node.js not found. Please install from https://nodejs.org/" -ForegroundColor Red
    exit 1
}

# Check Ruby
if (!(Get-Command ruby -ErrorAction SilentlyContinue)) {
    Write-Host "❌ Ruby not found. Please install Ruby for Windows" -ForegroundColor Red
    exit 1
}

# Check Bundler
if (!(Get-Command bundle -ErrorAction SilentlyContinue)) {
    Write-Host "❌ Bundler not found. Installing..." -ForegroundColor Yellow
    gem install bundler
}

Write-Host "✅ Prerequisites checked" -ForegroundColor Green

# Install dependencies
Write-Host "📦 Installing dependencies..." -ForegroundColor Yellow
npm install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

bundle install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Prepare Rails app (unless skipped)
if (-not $SkipRails) {
    Write-Host "🚂 Preparing Rails application..." -ForegroundColor Yellow
    $env:RAILS_ENV = "desktop"
    bundle exec rails db:prepare
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    
    bundle exec rails assets:precompile
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

# Check for app icons
Write-Host "🎨 Checking app icons..." -ForegroundColor Yellow
if (!(Test-Path "src-tauri/icons/icon.ico")) {
    Write-Host "⚠️  App icons not found. You'll need to add icons to src-tauri/icons/" -ForegroundColor Yellow
    Write-Host "   Required: 32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico" -ForegroundColor Yellow
    Write-Host "   You can generate these from a 1024x1024 PNG using online tools" -ForegroundColor Yellow
    Write-Host "   Continuing with build anyway..." -ForegroundColor Yellow
}

# Add Rust target if not already added
Write-Host "🦀 Ensuring Rust target is available..." -ForegroundColor Yellow
rustup target add $Target

# Build desktop app
Write-Host "🔨 Building desktop application..." -ForegroundColor Yellow
if ($Debug) {
    npm run tauri:build:debug
} else {
    npm run tauri:build
}

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Build failed. Check the output above for errors." -ForegroundColor Red
    exit 1
}

# Check if build succeeded
$bundlePath = "src-tauri/target/release/bundle/msi"
if ($Debug) {
    $bundlePath = "src-tauri/target/debug/bundle/msi"
}

if (Test-Path $bundlePath) {
    Write-Host "✅ Build completed successfully!" -ForegroundColor Green
    Write-Host "📱 Desktop app created at: $bundlePath" -ForegroundColor Green
    Write-Host "💾 You can find the .msi installer in the same directory" -ForegroundColor Green
    
    # Optional: Open the build directory
    $choice = Read-Host "🔍 Open build directory? (y/n)"
    if ($choice -eq 'y' -or $choice -eq 'Y') {
        Start-Process $bundlePath
    }
} else {
    Write-Host "❌ Build failed. Check the output above for errors." -ForegroundColor Red
    exit 1
}

Write-Host "🎉 Cipher Desktop App build complete!" -ForegroundColor Green
Write-Host "📝 To test the app, you can run: npm run tauri:dev" -ForegroundColor Cyan