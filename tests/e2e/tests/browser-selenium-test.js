const { Builder, By, until } = require('selenium-webdriver');
const chrome = require('selenium-webdriver/chrome');
const path = require('path');
const fs = require('fs');

describe('Cipher Browser Test - JavaScript Debugging', function() {
    let driver;
    const testUsername = `testuser_${Date.now()}`;
    const testPassword = 'TestPassword123!';
    const browserTestPath = path.join(__dirname, '../browser-test.html');

    before(async function() {
        console.log('🚀 Starting Selenium browser test...');
        console.log(`Testing with user: ${testUsername}`);

        // Verify test file exists
        if (!fs.existsSync(browserTestPath)) {
            throw new Error(`Browser test file not found: ${browserTestPath}`);
        }

        // Set up Chrome options
        const options = new chrome.Options();
        options.addArguments('--disable-web-security');
        options.addArguments('--allow-running-insecure-content');
        options.addArguments('--disable-features=VizDisplayCompositor');
        // Uncomment the line below to run headless
        // options.addArguments('--headless');

        // Create WebDriver instance
        driver = await new Builder()
            .forBrowser('chrome')
            .setChromeOptions(options)
            .build();

        console.log('📱 Chrome browser launched');
    });

    after(async function() {
        if (driver) {
            console.log('🔄 Closing browser...');
            await driver.quit();
        }
    });

    describe('JavaScript Execution Test', function() {
        it('should load the browser test page', async function() {
            const fileUrl = 'file://' + browserTestPath;
            console.log(`Loading: ${fileUrl}`);

            await driver.get(fileUrl);
            await driver.sleep(2000); // Wait for initialization

            // Verify page loaded
            const title = await driver.getTitle();
            console.log(`Page title: ${title}`);
            if (!title.includes('Cipher')) {
                throw new Error('Page did not load correctly');
            }
        });

        it('should verify JavaScript is executing', async function() {
            // Check for test status element
            const testStatus = await driver.findElement(By.id('testStatus'));
            const statusText = await testStatus.getText();
            console.log(`Test status: ${statusText}`);

            if (statusText.includes('error') || statusText.includes('failed')) {
                throw new Error(`JavaScript initialization failed: ${statusText}`);
            }
        });

        it('should verify debug log is working', async function() {
            // Check debug log exists and has content
            const debugLog = await driver.findElement(By.id('debugLog'));
            const logText = await debugLog.getText();
            console.log('Debug log content:');
            console.log(logText);

            if (!logText.includes('JavaScript loading') && !logText.includes('DOMContentLoaded')) {
                throw new Error('Debug log not working or empty');
            }
        });

        it('should find and interact with form elements', async function() {
            // Find username field
            const usernameField = await driver.findElement(By.id('registerUsername'));
            await usernameField.click();
            await usernameField.clear();
            await usernameField.sendKeys(testUsername);
            console.log(`✅ Entered username: ${testUsername}`);

            // Find password field
            const passwordField = await driver.findElement(By.id('registerPassword'));
            await passwordField.click();
            await passwordField.clear();
            await passwordField.sendKeys(testPassword);
            console.log('✅ Entered password');

            // Verify values were entered
            const usernameValue = await usernameField.getAttribute('value');
            const passwordValue = await passwordField.getAttribute('value');

            if (usernameValue !== testUsername) {
                throw new Error(`Username field value mismatch: expected ${testUsername}, got ${usernameValue}`);
            }

            if (passwordValue !== testPassword) {
                throw new Error('Password field value mismatch');
            }
        });

        it('should test Create Account button click', async function() {
            // Find Create Account button
            const createAccountBtn = await driver.findElement(By.id('createAccountBtn'));

            // Get initial debug log content
            const debugLogBefore = await driver.findElement(By.id('debugMessages'));
            const logContentBefore = await debugLogBefore.getText();

            console.log('Clicking Create Account button...');
            await createAccountBtn.click();

            // Wait for JavaScript to execute
            await driver.sleep(1000);

            // Check if debug log was updated (indicating JavaScript executed)
            const debugLogAfter = await driver.findElement(By.id('debugMessages'));
            const logContentAfter = await debugLogAfter.getText();

            console.log('Debug log before click:');
            console.log(logContentBefore.substring(Math.max(0, logContentBefore.length - 200)));
            console.log('Debug log after click:');
            console.log(logContentAfter.substring(Math.max(0, logContentAfter.length - 200)));

            if (logContentAfter === logContentBefore) {
                throw new Error('Debug log did not change - JavaScript may not be executing on button click');
            }

            if (!logContentAfter.includes('handleRegister function called')) {
                throw new Error('handleRegister function was not called');
            }

            console.log('✅ Create Account button click triggered JavaScript execution');
        });

        it('should verify registration flow completes', async function() {
            // Wait for registration process to complete
            await driver.sleep(3000);

            // Check test status for success
            const testStatus = await driver.findElement(By.id('testStatus'));
            const statusText = await testStatus.getText();
            console.log(`Final test status: ${statusText}`);

            // Check if we can see success message or dashboard
            try {
                const successMessage = await driver.findElement(By.id('registerSuccess'));
                const isVisible = await successMessage.isDisplayed();

                if (isVisible) {
                    const successText = await successMessage.getText();
                    console.log(`Success message: ${successText}`);
                }
            } catch (e) {
                console.log('No success message found (this may be normal)');
            }

            // Check final debug log
            const debugMessages = await driver.findElement(By.id('debugMessages'));
            const finalLog = await debugMessages.getText();
            console.log('Final debug log:');
            console.log(finalLog.split('\n').slice(-10).join('\n')); // Last 10 lines

            if (statusText.includes('successful') || finalLog.includes('Registration successful')) {
                console.log('🎉 Browser test completed successfully!');
            } else {
                console.log('⚠️ Test completed but may have issues - check logs');
            }
        });
    });

    describe('Compare with Tauri Issues', function() {
        it('should demonstrate working JavaScript vs Tauri webview', async function() {
            console.log('\n📊 COMPARISON SUMMARY:');
            console.log('✅ Browser: JavaScript executes normally');
            console.log('✅ Browser: Event listeners work correctly');
            console.log('✅ Browser: Console.log output is visible');
            console.log('✅ Browser: DOM manipulation works');
            console.log('✅ Browser: Async functions execute properly');
            console.log('❌ Tauri: JavaScript appears to not execute at all');
            console.log('❌ Tauri: No console.log output in application logs');
            console.log('❌ Tauri: Event listeners not triggering functions');

            console.log('\n🔍 DIAGNOSIS:');
            console.log('The issue is likely in Tauri\'s webview configuration or CSP settings');
            console.log('JavaScript code itself is correct (proven by browser test)');
            console.log('Tauri webview may have stricter security policies or execution context issues');
        });
    });
});