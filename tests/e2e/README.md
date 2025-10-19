# Cipher App - End-to-End Testing with Appium

This directory contains end-to-end tests for the Cipher app using Appium to simulate real user interactions.

## Setup

### Prerequisites

1. **Node.js and npm** (already available)
2. **Appium** - Will be installed via npm
3. **Platform-specific drivers**:
   - **macOS**: Mac2 driver (included)
   - **Android**: Android SDK and emulator/device
   - **iOS**: Xcode and iOS Simulator (macOS only)

### Installation

```bash
cd tests/e2e
npm install
```

### Install Appium globally (optional but recommended)
```bash
npm install -g appium
```

### Install platform drivers
```bash
# For macOS testing
appium driver install mac2

# For Android testing
appium driver install uiautomator2

# For iOS testing (macOS only)
appium driver install xcuitest
```

## Running Tests

### Start Appium Server
```bash
appium server --port 4723
```

### Run Tests

#### macOS Desktop App
```bash
# Build the desktop app first
cargo tauri build

# Run tests
npm run test:mac
```

#### Android App
```bash
# Build Android APK first
ANDROID_HOME=~/Library/Android/sdk NDK_HOME=~/Library/Android/sdk/ndk OPENSSL_STATIC=1 OPENSSL_VENDORED=1 cargo tauri android build --target aarch64 --debug

# Start Android emulator or connect device
# Run tests
npm run test:android
```

#### iOS App (macOS only)
```bash
# Build iOS app first
cargo tauri ios build

# Start iOS Simulator
# Run tests
npm run test:ios
```

## Test Structure

### Current Tests

- **`signup-and-posting.test.js`**: Complete user flow testing
  - App startup and default signup form display
  - User registration flow
  - Dashboard navigation
  - Tab switching (Posts, Messages, Friends)
  - Logout flow
  - Login flow with existing account

### Test Flow

1. **App Launch**: Verifies app starts and shows signup form
2. **User Registration**:
   - Fills out signup form with generated test data
   - Verifies account creation and auto-login
   - Confirms dashboard appears
3. **Dashboard Navigation**:
   - Tests all navigation buttons are present
   - Verifies Posts tab is default
   - Tests navigation between tabs
4. **Logout/Login**:
   - Tests logout returns to signup form
   - Tests switching between signup and login forms
   - Tests login with previously created account

## Configuration

### Platform Configuration

The tests automatically detect the target platform via the `TEST_PLATFORM` environment variable:

- `mac` (default): Tests the macOS desktop app
- `android`: Tests the Android APK
- `ios`: Tests the iOS app

### App Paths

The configuration automatically points to the correct app bundle for each platform:

- **macOS**: `target/release/bundle/macos/Cipher.app`
- **Android**: `gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`
- **iOS**: `target/release/bundle/ios/Cipher.app`

## Troubleshooting

### Common Issues

1. **App not found**: Make sure you've built the app for the target platform first
2. **Appium server not running**: Start the Appium server before running tests
3. **Driver not installed**: Install the appropriate driver for your platform
4. **Timeout errors**: The app may take longer to start; increase timeouts if needed

### Debug Mode

Add more verbose logging by setting the log level in the config:

```javascript
logLevel: 'debug'
```

### Manual Testing

You can also use Appium Inspector to manually explore the app's UI elements:

1. Install Appium Inspector
2. Connect to your running Appium server
3. Use the same capabilities as in the test config

## Adding New Tests

1. Create new test files in the `tests/` directory
2. Follow the existing pattern for WebDriverIO + Mocha + Chai
3. Use the shared configuration from `config/appium.config.js`
4. Add descriptive test names and console logging for easy debugging

## Future Enhancements

- Add visual testing with screenshot comparison
- Add performance testing
- Add network request mocking/validation
- Add accessibility testing
- Add cross-platform test reports