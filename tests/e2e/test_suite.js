// Centralized E2E test suite infrastructure for resource efficiency
const { remote } = require('webdriverio');
const path = require('path');
const fs = require('fs');
const { spawn } = require('child_process');

class E2ETestSuite {
    constructor() {
        this.driver = null;
        this.appProcess = null;
        this.appStarted = false;
        this.testUsers = new Map();
        this.currentUser = null;
        this.appPath = null;
        this.platform = process.platform;
        this.fixtureData = null;
    }

    /**
     * Initialize the test suite once for all tests
     */
    async setupOnce(options = {}) {
        console.log('🚀 Starting E2E Test Suite (Single App Instance)');

        // Determine app path based on platform
        this.appPath = options.appPath || this.getDefaultAppPath();

        // Clean previous test data
        await this.cleanTestData();

        // Start the app once
        await this.startApp();

        // Create WebDriver session
        await this.createDriver(options);

        // Load fixture data if provided
        if (options.fixtureData) {
            await this.loadFixtures(options.fixtureData);
        }

        console.log('✅ E2E Test Suite initialized');
    }

    /**
     * Get default app path based on platform
     */
    getDefaultAppPath() {
        const basePath = path.join(__dirname, '../../target/release');

        switch (this.platform) {
            case 'darwin':
                return path.join(basePath, 'bundle/macos/Cipher.app');
            case 'win32':
                return path.join(basePath, 'bundle/windows/Cipher.exe');
            case 'linux':
                return path.join(basePath, 'bundle/appimage/Cipher.AppImage');
            default:
                throw new Error(`Unsupported platform: ${this.platform}`);
        }
    }

    /**
     * Start the application once
     */
    async startApp() {
        if (this.appStarted) return;

        console.log(`Starting app at: ${this.appPath}`);

        // Platform-specific app launching
        if (this.platform === 'darwin') {
            this.appProcess = spawn('open', ['-W', this.appPath]);
        } else if (this.platform === 'win32') {
            this.appProcess = spawn(this.appPath);
        } else {
            this.appProcess = spawn(this.appPath);
        }

        // Wait for app to be ready
        await this.waitForApp();
        this.appStarted = true;
    }

    /**
     * Create WebDriver session (reusable)
     */
    async createDriver(options = {}) {
        if (this.driver) return this.driver;

        const capabilities = this.getPlatformCapabilities();

        this.driver = await remote({
            hostname: options.hostname || 'localhost',
            port: options.port || 4723,
            capabilities: {
                ...capabilities,
                ...options.additionalCapabilities
            },
            logLevel: options.logLevel || 'error'
        });

        return this.driver;
    }

    /**
     * Get platform-specific capabilities
     */
    getPlatformCapabilities() {
        const baseCapabilities = {
            platformName: this.platform === 'darwin' ? 'Mac' :
                         this.platform === 'win32' ? 'Windows' : 'Linux',
            'appium:automationName': this.platform === 'darwin' ? 'Mac2' : 'Windows',
            'appium:bundleId': 'com.cipher.app',
            'appium:noReset': true, // Don't reset app state between tests
            'appium:newCommandTimeout': 120
        };

        if (this.platform === 'darwin') {
            baseCapabilities['appium:showServerLogs'] = true;
        }

        return baseCapabilities;
    }

    /**
     * Wait for app to be ready
     */
    async waitForApp(timeout = 10000) {
        const startTime = Date.now();

        while (Date.now() - startTime < timeout) {
            try {
                // Try to connect to app
                if (this.driver) {
                    await this.driver.getTitle();
                    return;
                }
                await new Promise(resolve => setTimeout(resolve, 500));
            } catch (e) {
                // App not ready yet
                await new Promise(resolve => setTimeout(resolve, 500));
            }
        }

        throw new Error('App failed to start within timeout');
    }

    /**
     * Create a test user without recreating the app
     */
    async createTestUser(username) {
        const timestamp = Date.now();
        const testUsername = `${username}_${timestamp}`;

        console.log(`Creating test user: ${testUsername}`);

        // Navigate to signup if needed
        await this.navigateToSignup();

        // Create user
        await this.fillSignupForm(testUsername);

        // Store user info
        const userInfo = {
            username: testUsername,
            createdAt: new Date(),
            sessionActive: true
        };

        this.testUsers.set(username, userInfo);
        this.currentUser = userInfo;

        return userInfo;
    }

    /**
     * Switch between test users without restarting app
     */
    async switchUser(username) {
        const userInfo = this.testUsers.get(username);
        if (!userInfo) {
            throw new Error(`Test user ${username} not found`);
        }

        console.log(`Switching to user: ${userInfo.username}`);

        // Logout current user
        await this.logout();

        // Login as specified user
        await this.login(userInfo.username);

        this.currentUser = userInfo;
    }

    /**
     * Clean up test user data
     */
    async cleanupTestUser(username) {
        const userInfo = this.testUsers.get(username);
        if (!userInfo) return;

        console.log(`Cleaning up test user: ${userInfo.username}`);

        // Delete user data from database
        // This would be implemented based on your app's data structure

        this.testUsers.delete(username);

        if (this.currentUser === userInfo) {
            this.currentUser = null;
        }
    }

    /**
     * Load fixture data
     */
    async loadFixtures(fixtureFile) {
        console.log(`Loading fixtures from: ${fixtureFile}`);

        const fixturePath = path.join(__dirname, 'fixtures', fixtureFile);
        this.fixtureData = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));

        // Apply fixtures to app
        await this.applyFixtures();
    }

    /**
     * Apply fixture data to the app
     */
    async applyFixtures() {
        if (!this.fixtureData) return;

        // Create fixture users
        for (const user of this.fixtureData.users || []) {
            await this.createTestUser(user.username);
        }

        // Create fixture posts
        for (const post of this.fixtureData.posts || []) {
            await this.createPost(post.content, post.author);
        }

        // Create fixture friendships
        for (const friendship of this.fixtureData.friendships || []) {
            await this.createFriendship(friendship.user1, friendship.user2);
        }
    }

    /**
     * Clean test data without restarting app
     */
    async cleanTestData() {
        console.log('Cleaning test data...');

        // Platform-specific data cleaning
        const dataPath = this.getDataPath();

        if (fs.existsSync(dataPath)) {
            // Clean database files but keep app config
            const dbFiles = fs.readdirSync(dataPath)
                .filter(f => f.endsWith('.db') || f.includes('test_'));

            for (const file of dbFiles) {
                try {
                    fs.unlinkSync(path.join(dataPath, file));
                } catch (e) {
                    // File in use, skip
                }
            }
        }
    }

    /**
     * Get app data path based on platform
     */
    getDataPath() {
        const home = process.env.HOME || process.env.USERPROFILE;

        switch (this.platform) {
            case 'darwin':
                return path.join(home, 'Library/Application Support/com.cipher.app');
            case 'win32':
                return path.join(home, 'AppData/Roaming/com.cipher.app');
            case 'linux':
                return path.join(home, '.config/com.cipher.app');
            default:
                return './test_data';
        }
    }

    // Page Object Methods (shared element interactions)

    async navigateToSignup() {
        const signupButton = await this.findElement('signup-button', 'Sign Up');
        if (signupButton) {
            await signupButton.click();
        }
    }

    async fillSignupForm(username) {
        const usernameInput = await this.findElement('username-input', 'Username');
        await usernameInput.setValue(username);

        const submitButton = await this.findElement('submit-button', 'Create Account');
        await submitButton.click();

        // Wait for dashboard
        await this.waitForElement('dashboard', 'Dashboard');
    }

    async logout() {
        const menuButton = await this.findElement('menu-button', 'Menu');
        await menuButton.click();

        const logoutButton = await this.findElement('logout-button', 'Logout');
        await logoutButton.click();

        // Wait for login screen
        await this.waitForElement('login-form', 'Login');
    }

    async login(username) {
        const usernameInput = await this.findElement('username-input', 'Username');
        await usernameInput.setValue(username);

        const loginButton = await this.findElement('login-button', 'Login');
        await loginButton.click();

        // Wait for dashboard
        await this.waitForElement('dashboard', 'Dashboard');
    }

    async createPost(content, author = null) {
        if (author && this.currentUser?.username !== author) {
            await this.switchUser(author);
        }

        const postInput = await this.findElement('post-input', 'What\'s on your mind?');
        await postInput.setValue(content);

        const postButton = await this.findElement('post-button', 'Post');
        await postButton.click();

        // Wait for post to appear
        await this.waitForText(content);
    }

    async createFriendship(user1, user2) {
        // Implementation depends on your app's friendship flow
        await this.switchUser(user1);
        await this.searchUser(user2);
        await this.sendFriendRequest();

        await this.switchUser(user2);
        await this.acceptFriendRequest(user1);
    }

    /**
     * Find element with multiple strategies
     */
    async findElement(id, text = null, timeout = 5000) {
        const strategies = [
            () => this.driver.$(`#${id}`),
            () => this.driver.$(`[data-testid="${id}"]`),
            () => this.driver.$(`[aria-label="${id}"]`),
            () => text ? this.driver.$(`//*[contains(text(), "${text}")]`) : null,
            () => text ? this.driver.$(`//button[contains(text(), "${text}")]`) : null,
        ];

        for (const strategy of strategies) {
            try {
                const element = await strategy();
                if (element && await element.isExisting()) {
                    return element;
                }
            } catch (e) {
                // Try next strategy
            }
        }

        throw new Error(`Element not found: ${id} ${text ? `(${text})` : ''}`);
    }

    /**
     * Wait for element to appear
     */
    async waitForElement(id, text = null, timeout = 10000) {
        const startTime = Date.now();

        while (Date.now() - startTime < timeout) {
            try {
                const element = await this.findElement(id, text, 500);
                if (element) return element;
            } catch (e) {
                await new Promise(resolve => setTimeout(resolve, 500));
            }
        }

        throw new Error(`Timeout waiting for element: ${id}`);
    }

    /**
     * Wait for text to appear
     */
    async waitForText(text, timeout = 10000) {
        const startTime = Date.now();

        while (Date.now() - startTime < timeout) {
            try {
                const element = await this.driver.$(`//*[contains(text(), "${text}")]`);
                if (await element.isExisting()) return element;
            } catch (e) {
                await new Promise(resolve => setTimeout(resolve, 500));
            }
        }

        throw new Error(`Timeout waiting for text: ${text}`);
    }

    /**
     * Take screenshot for debugging
     */
    async takeScreenshot(name) {
        const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
        const filename = `${name}_${timestamp}.png`;
        const filepath = path.join(__dirname, 'screenshots', filename);

        await this.driver.saveScreenshot(filepath);
        console.log(`Screenshot saved: ${filepath}`);
    }

    /**
     * Teardown - called once after all tests
     */
    async teardownOnce() {
        console.log('🔚 Tearing down E2E Test Suite');

        // Clean up all test users
        for (const [username, userInfo] of this.testUsers) {
            await this.cleanupTestUser(username);
        }

        // Close WebDriver session
        if (this.driver) {
            await this.driver.deleteSession();
            this.driver = null;
        }

        // Stop the app
        if (this.appProcess) {
            this.appProcess.kill();
            this.appProcess = null;
        }

        // Final cleanup
        await this.cleanTestData();

        console.log('✅ E2E Test Suite teardown complete');
    }

    /**
     * Get test statistics
     */
    getStats() {
        return {
            platform: this.platform,
            testUsers: this.testUsers.size,
            currentUser: this.currentUser?.username,
            appStarted: this.appStarted,
            sessionActive: this.driver !== null
        };
    }
}

// Singleton instance
let testSuite = null;

/**
 * Get or create the test suite instance
 */
function getTestSuite() {
    if (!testSuite) {
        testSuite = new E2ETestSuite();
    }
    return testSuite;
}

module.exports = {
    E2ETestSuite,
    getTestSuite
};