@echo off
rem Build script for Cipher Desktop App on Windows
rem Batch file version for compatibility

echo 🔐 Building Cipher Desktop App for Windows

rem Check prerequisites
echo 📋 Checking prerequisites...

rem Check Rust
cargo --version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Rust/Cargo not found. Please install from https://rustup.rs/
    exit /b 1
)

rem Check Node.js
node --version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Node.js not found. Please install from https://nodejs.org/
    exit /b 1
)

rem Check Ruby
ruby --version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Ruby not found. Please install Ruby for Windows
    exit /b 1
)

rem Check Bundler
bundle --version >nul 2>&1
if %errorlevel% neq 0 (
    echo ❌ Bundler not found. Installing...
    gem install bundler
)

echo ✅ Prerequisites checked

rem Install dependencies
echo 📦 Installing dependencies...
call npm install
if %errorlevel% neq 0 exit /b %errorlevel%

call bundle install
if %errorlevel% neq 0 exit /b %errorlevel%

rem Prepare Rails app
echo 🚂 Preparing Rails application...
set RAILS_ENV=desktop
call bundle exec rails db:prepare
if %errorlevel% neq 0 exit /b %errorlevel%

call bundle exec rails assets:precompile
if %errorlevel% neq 0 exit /b %errorlevel%

rem Check for app icons
echo 🎨 Checking app icons...
if not exist "src-tauri\icons\icon.ico" (
    echo ⚠️  App icons not found. You'll need to add icons to src-tauri/icons/
    echo    Required: 32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico
    echo    You can generate these from a 1024x1024 PNG using online tools
    echo    Continuing with build anyway...
)

rem Build desktop app
echo 🔨 Building desktop application...
call npm run tauri:build
if %errorlevel% neq 0 (
    echo ❌ Build failed. Check the output above for errors.
    exit /b 1
)

rem Check if build succeeded
if exist "src-tauri\target\release\bundle\msi" (
    echo ✅ Build completed successfully!
    echo 📱 Desktop app created at: src-tauri\target\release\bundle\msi
    echo 💾 You can find the .msi installer in the same directory
    echo 🎉 Cipher Desktop App build complete!
    echo 📝 To test the app, you can run: npm run tauri:dev
) else (
    echo ❌ Build failed. Check the output above for errors.
    exit /b 1
)