#!/bin/bash

# Desktop App Automated Test Script
# Tests account creation functionality

echo "🧪 Starting Desktop App Automated Test"

# Kill any existing instances
pkill -f "Cipher.app" || true
sleep 2

# Launch the desktop app
echo "📱 Launching Cipher Desktop App..."
open src-tauri/target/release/bundle/macos/Cipher.app

# Wait for app to start and server to initialize
echo "⏳ Waiting for app to initialize..."
sleep 8

# Test if the app is running and responding
echo "🔍 Testing if app is responding..."
response=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/ || echo "000")
echo "HTTP Response: $response"

if [ "$response" = "200" ]; then
    echo "✅ App is running and responding"

    # Get the home page content to see if Rails is working
    echo "🔍 Getting home page content..."
    home_content=$(curl -s http://localhost:3000/ | head -20)
    echo "Home page preview: $home_content"

    # Test account creation page
    echo "🔍 Testing account creation page..."
    signup_page=$(curl -s http://localhost:3000/signup)
    signup_response=$(echo "$signup_page" | wc -l)
    echo "Signup page lines: $signup_response"

    # Check for specific database error in signup page
    if echo "$signup_page" | grep -qi "table.*users\|database.*error\|ActiveRecord"; then
        echo "❌ Database error found in signup page:"
        echo "$signup_page" | grep -i "table\|database\|ActiveRecord\|error" | head -5
    else
        echo "✅ No obvious database errors in signup page"
    fi

    # Try to create a test account via web form
    echo "🧪 Testing account creation via form..."

    # Get CSRF token first
    csrf_token=$(echo "$signup_page" | grep -o 'csrf-token.*content="[^"]*"' | sed 's/.*content="\([^"]*\)".*/\1/')
    echo "CSRF token: ${csrf_token:0:20}..."

    if [ -n "$csrf_token" ]; then
        create_response=$(curl -s -X POST \
            -H "Content-Type: application/x-www-form-urlencoded" \
            -d "authenticity_token=$csrf_token&user[username]=testuser&user[email]=test@example.com&user[password]=testpass123&user[password_confirmation]=testpass123" \
            http://localhost:3000/signup)

        if echo "$create_response" | grep -qi "table.*users\|database.*error\|ActiveRecord"; then
            echo "❌ Account creation failed with database error:"
            echo "$create_response" | grep -i "table\|database\|ActiveRecord\|error" | head -3
        else
            echo "✅ Account creation response looks normal"
        fi
    else
        echo "❌ Could not find CSRF token in signup page"
    fi

else
    echo "❌ App is not responding at localhost:3000"
fi

# Check what processes are running
echo "🔍 Checking running processes..."
ps aux | grep -E "(rails|ruby|Cipher)" | grep -v grep

# Check if there are any log files we can examine
echo "🔍 Checking for app logs..."
if [ -f ~/Library/Logs/cipher/main.log ]; then
    echo "Found app logs:"
    tail -20 ~/Library/Logs/cipher/main.log
fi

echo "🧪 Test completed"