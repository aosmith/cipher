const path = require('path');

// Get platform from environment variable or default to mac
const platform = process.env.TEST_PLATFORM || 'mac';

const configs = {
  mac: {
    platformName: 'Mac',
    'appium:deviceName': 'Mac',
    'appium:app': path.join(__dirname, '../../../target/release/bundle/macos/Cipher.app'),
    'appium:automationName': 'Mac2',
    'appium:newCommandTimeout': 60000,
    'appium:connectHardwareKeyboard': true,
    'appium:showServerLogs': true,
    'appium:shouldTerminateApp': true,
    'appium:shouldRestartApp': false,
    'appium:suppressAutomationTooltip': true
  },
  android: {
    platformName: 'Android',
    'appium:deviceName': 'Android Emulator',
    'appium:app': path.join(__dirname, '../../../gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk'),
    'appium:automationName': 'UiAutomator2',
    'appium:newCommandTimeout': 30000,
    'appium:autoGrantPermissions': true,
    'appium:noReset': false
  },
  ios: {
    platformName: 'iOS',
    'appium:deviceName': 'iPhone Simulator',
    'appium:platformVersion': '17.0',
    'appium:app': path.join(__dirname, '../../../target/release/bundle/ios/Cipher.app'),
    'appium:automationName': 'XCUITest',
    'appium:newCommandTimeout': 30000,
    'appium:autoAcceptAlerts': true,
    'appium:noReset': false
  }
};

module.exports = {
  runner: 'local',
  port: 4723,
  path: '/',
  specs: [
    './tests/**/*.test.js'
  ],
  maxInstances: 1,
  capabilities: [{
    ...configs[platform]
  }],
  logLevel: 'info',
  bail: 0,
  baseUrl: 'http://localhost',
  waitforTimeout: 30000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000
  },

  // Hooks
  before: function (capabilities, specs) {
    console.log(`Starting tests for platform: ${platform}`);
  },

  after: function (result, capabilities, specs) {
    console.log('Tests completed');
  }
};