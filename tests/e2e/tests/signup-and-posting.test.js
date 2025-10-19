const { remote } = require('webdriverio');
const { expect } = require('chai');
const config = require('../config/appium.config');

describe('Cipher App - Signup and Posting Flow', function() {
  let driver;
  const testUsername = `testuser_${Date.now()}`;
  const testEmail = `${testUsername}@test.com`;
  const testPassword = 'TestPassword123!';

  before(async function() {
    // Create WebDriverIO instance with Appium
    driver = await remote({
      hostname: 'localhost',
      port: 4723,
      path: '/wd/hub',
      capabilities: config.capabilities[0],
      logLevel: 'info'
    });

    console.log(`Testing with user: ${testUsername}`);
  });

  after(async function() {
    if (driver) {
      await driver.deleteSession();
    }
  });

  describe('User Signup Flow', function() {
    it('should display the signup form by default', async function() {
      // Wait for app to load
      await driver.pause(3000);

      // Check if the signup form is visible
      // Look for the "Create Account" button which indicates signup form
      const signupButton = await driver.$('button*=Create Account');
      await signupButton.waitForDisplayed({ timeout: 10000 });

      expect(await signupButton.isDisplayed()).to.be.true;
      console.log('✅ Signup form is displayed by default');
    });

    it('should allow user to register a new account', async function() {
      // Find and fill username field
      const usernameField = await driver.$('input[id="registerUsername"]');
      await usernameField.waitForDisplayed({ timeout: 5000 });
      await usernameField.setValue(testUsername);
      console.log(`✅ Entered username: ${testUsername}`);

      // Find and fill email field
      const emailField = await driver.$('input[id="registerEmail"]');
      await emailField.setValue(testEmail);
      console.log(`✅ Entered email: ${testEmail}`);

      // Find and fill password field
      const passwordField = await driver.$('input[id="registerPassword"]');
      await passwordField.setValue(testPassword);
      console.log(`✅ Entered password`);

      // Click Create Account button
      const createAccountButton = await driver.$('button*=Create Account');
      await createAccountButton.click();
      console.log('✅ Clicked Create Account button');

      // Wait for success message or dashboard
      await driver.pause(2000);

      // Check for either success message or dashboard
      try {
        // Look for success message first
        const successMessage = await driver.$('.success');
        if (await successMessage.isDisplayed()) {
          console.log('✅ Success message displayed');
          // Wait for auto-redirect to dashboard
          await driver.pause(2000);
        }
      } catch (e) {
        // Success message might not appear if auto-login is immediate
        console.log('ℹ️ No success message, checking for dashboard directly');
      }

      // Verify we're now on the dashboard
      const welcomeMessage = await driver.$('h2*=Welcome to Cipher');
      await welcomeMessage.waitForDisplayed({ timeout: 10000 });
      expect(await welcomeMessage.isDisplayed()).to.be.true;
      console.log('✅ Successfully registered and logged in to dashboard');
    });
  });

  describe('Dashboard Navigation', function() {
    it('should show dashboard with navigation buttons', async function() {
      // Check for navigation buttons
      const postsButton = await driver.$('button*=Posts');
      const messagesButton = await driver.$('button*=Messages');
      const friendsButton = await driver.$('button*=Friends');

      expect(await postsButton.isDisplayed()).to.be.true;
      expect(await messagesButton.isDisplayed()).to.be.true;
      expect(await friendsButton.isDisplayed()).to.be.true;
      console.log('✅ All navigation buttons are present');
    });

    it('should default to Posts tab', async function() {
      // Posts tab should be visible by default
      const postsTab = await driver.$('#postsTab');
      expect(await postsTab.isDisplayed()).to.be.true;

      const postsTitle = await driver.$('h3*=Latest Posts');
      expect(await postsTitle.isDisplayed()).to.be.true;
      console.log('✅ Posts tab is displayed by default');
    });
  });

  describe('Posting Flow', function() {
    it('should show "No posts yet" message initially', async function() {
      // Look for the "No posts yet" message
      const noPostsMessage = await driver.$('p*=No posts yet');

      // Wait a moment for posts to load
      await driver.pause(1000);

      expect(await noPostsMessage.isDisplayed()).to.be.true;
      console.log('✅ "No posts yet" message is displayed');
    });

    // Note: This test is commented out because the current UI doesn't have a post creation form
    // We would need to add a "Create Post" button and form to the UI first
    /*
    it('should allow creating a new post', async function() {
      const testPostContent = `Test post from automated test - ${Date.now()}`;

      // Find create post form (would need to be added to UI)
      const createPostButton = await driver.$('button*=Create Post');
      await createPostButton.click();

      const postContentField = await driver.$('textarea[placeholder*="What\'s on your mind"]');
      await postContentField.setValue(testPostContent);

      const publishButton = await driver.$('button*=Publish');
      await publishButton.click();

      // Verify post appears in feed
      const newPost = await driver.$(`div*=${testPostContent}`);
      await newPost.waitForDisplayed({ timeout: 5000 });
      expect(await newPost.isDisplayed()).to.be.true;
      console.log('✅ Successfully created and verified new post');
    });
    */
  });

  describe('Tab Navigation', function() {
    it('should navigate to Messages tab', async function() {
      const messagesButton = await driver.$('button*=Messages');
      await messagesButton.click();

      const messagesTab = await driver.$('#messagesTab');
      await messagesTab.waitForDisplayed({ timeout: 5000 });
      expect(await messagesTab.isDisplayed()).to.be.true;

      const messagesTitle = await driver.$('h3*=Messages');
      expect(await messagesTitle.isDisplayed()).to.be.true;
      console.log('✅ Successfully navigated to Messages tab');
    });

    it('should navigate to Friends tab', async function() {
      const friendsButton = await driver.$('button*=Friends');
      await friendsButton.click();

      const friendsTab = await driver.$('#friendsTab');
      await friendsTab.waitForDisplayed({ timeout: 5000 });
      expect(await friendsTab.isDisplayed()).to.be.true;

      const friendsTitle = await driver.$('h3*=Friends');
      expect(await friendsTitle.isDisplayed()).to.be.true;
      console.log('✅ Successfully navigated to Friends tab');
    });

    it('should navigate back to Posts tab', async function() {
      const postsButton = await driver.$('button*=Posts');
      await postsButton.click();

      const postsTab = await driver.$('#postsTab');
      await postsTab.waitForDisplayed({ timeout: 5000 });
      expect(await postsTab.isDisplayed()).to.be.true;

      const postsTitle = await driver.$('h3*=Latest Posts');
      expect(await postsTitle.isDisplayed()).to.be.true;
      console.log('✅ Successfully navigated back to Posts tab');
    });
  });

  describe('Logout Flow', function() {
    it('should allow user to sign out', async function() {
      const signOutButton = await driver.$('button*=Sign Out');
      await signOutButton.click();

      // Should return to signup form
      const signupButton = await driver.$('button*=Create Account');
      await signupButton.waitForDisplayed({ timeout: 5000 });
      expect(await signupButton.isDisplayed()).to.be.true;
      console.log('✅ Successfully signed out and returned to signup form');
    });
  });

  describe('Login Flow', function() {
    it('should allow switching to login form', async function() {
      // Click "Already have an account? Sign in" link
      const signInLink = await driver.$('span*=Already have an account');
      await signInLink.click();

      // Should now show login form
      const signInButton = await driver.$('button*=Sign In');
      await signInButton.waitForDisplayed({ timeout: 5000 });
      expect(await signInButton.isDisplayed()).to.be.true;
      console.log('✅ Successfully switched to login form');
    });

    it('should allow user to login with existing account', async function() {
      // Fill login form
      const loginUsernameField = await driver.$('input[id="loginUsername"]');
      await loginUsernameField.setValue(testUsername);

      const loginPasswordField = await driver.$('input[id="loginPassword"]');
      await loginPasswordField.setValue(testPassword);

      // Click Sign In button
      const signInButton = await driver.$('button*=Sign In');
      await signInButton.click();

      // Should return to dashboard
      const welcomeMessage = await driver.$('h2*=Welcome to Cipher');
      await welcomeMessage.waitForDisplayed({ timeout: 10000 });
      expect(await welcomeMessage.isDisplayed()).to.be.true;
      console.log('✅ Successfully logged in with existing account');
    });
  });
});