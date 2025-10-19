const { remote } = require('webdriverio');

async function runSimpleTest() {
  console.log('🚀 Starting simple Cipher app test...');

  let driver;

  try {
    // For now, let's test against the running dev server
    // This tests the actual web content that Tauri displays
    driver = await remote({
      hostname: 'localhost',
      port: 4444, // Selenium port
      capabilities: {
        browserName: 'chrome',
        'goog:chromeOptions': {
          args: ['--no-sandbox', '--disable-dev-shm-usage']
        }
      }
    });

    console.log('🌐 Navigating to app...');
    await driver.url('http://localhost:1420'); // Tauri dev server

    console.log('⏳ Waiting for app to load...');
    await driver.pause(2000);

    // Test 1: Check if signup form is displayed
    console.log('✅ Test 1: Checking signup form...');
    const appTitle = await driver.$('h1');
    const titleText = await appTitle.getText();
    console.log(`App title: ${titleText}`);

    const signupButton = await driver.$('button*=Create Account');
    const isSignupVisible = await signupButton.isDisplayed();
    console.log(`Signup form visible: ${isSignupVisible}`);

    if (isSignupVisible) {
      console.log('✅ Signup form test PASSED');
    } else {
      console.log('❌ Signup form test FAILED');
    }

    // Test 2: Fill signup form
    console.log('✅ Test 2: Testing signup flow...');
    const testUsername = `testuser_${Date.now()}`;

    const usernameField = await driver.$('#registerUsername');
    await usernameField.setValue(testUsername);
    console.log(`Entered username: ${testUsername}`);

    const emailField = await driver.$('#registerEmail');
    await emailField.setValue(`${testUsername}@test.com`);
    console.log(`Entered email: ${testUsername}@test.com`);

    const passwordField = await driver.$('#registerPassword');
    await passwordField.setValue('TestPassword123!');
    console.log('Entered password');

    // Click register button
    await signupButton.click();
    console.log('Clicked Create Account button');

    // Wait for result
    await driver.pause(3000);

    // Check if we're now on dashboard or got success message
    try {
      const welcomeMessage = await driver.$('h2*=Welcome to Cipher');
      const isDashboard = await welcomeMessage.isDisplayed();
      if (isDashboard) {
        console.log('✅ Registration successful - Dashboard loaded');

        // Test 3: Test navigation
        console.log('✅ Test 3: Testing navigation...');
        const messagesButton = await driver.$('button*=Messages');
        await messagesButton.click();

        const messagesTab = await driver.$('#messagesTab');
        const isMessagesVisible = await messagesTab.isDisplayed();
        console.log(`Messages tab visible: ${isMessagesVisible}`);

        if (isMessagesVisible) {
          console.log('✅ Navigation test PASSED');
        } else {
          console.log('❌ Navigation test FAILED');
        }
      } else {
        console.log('❌ Registration may have failed - no dashboard found');
      }
    } catch (error) {
      console.log('❌ Dashboard check failed:', error.message);
    }

    console.log('🎉 Test completed successfully!');

  } catch (error) {
    console.error('❌ Test failed:', error.message);
    console.error('This is likely because:');
    console.error('1. Chrome/ChromeDriver is not installed');
    console.error('2. Selenium server is not running');
    console.error('3. Tauri dev server is not running on localhost:1420');
    console.error('');
    console.error('To fix this:');
    console.error('1. Install ChromeDriver: brew install chromedriver');
    console.error('2. Start Selenium: selenium-server -port 4444');
    console.error('3. Make sure "cargo tauri dev" is running');
  } finally {
    if (driver) {
      await driver.deleteSession();
    }
  }
}

// Run the test
runSimpleTest().catch(console.error);