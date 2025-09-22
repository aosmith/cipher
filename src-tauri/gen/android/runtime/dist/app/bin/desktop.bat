@echo off
rem Windows launcher for Cipher Desktop App

echo == Starting Cipher Desktop App ==

rem Check if Tauri is set up
if not exist "src-tauri\Cargo.toml" (
    echo Setting up Tauri...
    call npm install
    call npx tauri init --ci
)

rem Check dependencies
echo Checking dependencies...
call bundle check || call bundle install

rem Setup database if needed
if not exist "storage\desktop.sqlite3" (
    echo Setting up database...
    set RAILS_ENV=desktop
    call bundle exec rails db:prepare
)

rem Start the desktop app
echo Launching Cipher Desktop App...
call npm run tauri:dev