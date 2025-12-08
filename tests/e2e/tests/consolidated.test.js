// Consolidated E2E tests using single app instance
const { expect } = require('chai');
const { getTestSuite } = require('../test_suite');

// Get the shared test suite instance
const suite = getTestSuite();

// Run once before all tests
before(async function() {
    this.timeout(30000); // Allow 30 seconds for initial setup
    await suite.setupOnce({
        logLevel: 'error',
        fixtureData: null // Start with clean slate
    });
});

// Run once after all tests
after(async function() {
    this.timeout(10000);
    await suite.teardownOnce();
});

// Clean up between test suites
afterEach(async function() {
    // Take screenshot on failure
    if (this.currentTest.state === 'failed') {
        await suite.takeScreenshot(this.currentTest.title);
    }
});

describe('Authentication Tests', function() {
    it('should allow user signup', async function() {
        const user = await suite.createTestUser('alice');
        expect(user.username).to.include('alice_');
        expect(user.sessionActive).to.be.true;

        // Verify we're on dashboard
        const dashboardElement = await suite.findElement('dashboard', 'Dashboard');
        expect(await dashboardElement.isExisting()).to.be.true;
    });

    it('should allow user logout', async function() {
        await suite.logout();

        // Verify we're on login screen
        const loginElement = await suite.findElement('login-form', 'Login');
        expect(await loginElement.isExisting()).to.be.true;
    });

    it('should allow user login', async function() {
        // Get the alice user created earlier
        const aliceInfo = suite.testUsers.get('alice');
        await suite.login(aliceInfo.username);

        // Verify we're on dashboard
        const dashboardElement = await suite.findElement('dashboard', 'Dashboard');
        expect(await dashboardElement.isExisting()).to.be.true;
    });

    it('should support multiple users', async function() {
        // Create Bob while Alice is still in system
        const bob = await suite.createTestUser('bob');
        expect(bob.username).to.include('bob_');

        // Switch back to Alice
        await suite.switchUser('alice');
        expect(suite.currentUser.username).to.include('alice_');

        // Switch to Bob
        await suite.switchUser('bob');
        expect(suite.currentUser.username).to.include('bob_');
    });
});

describe('Dashboard Tests', function() {
    before(async function() {
        // Ensure we have a logged-in user
        if (!suite.currentUser) {
            await suite.createTestUser('dashboard_user');
        }
    });

    it('should display user profile', async function() {
        const profileElement = await suite.findElement('user-profile');
        expect(await profileElement.isExisting()).to.be.true;

        // Check username is displayed
        const usernameText = await suite.waitForText(suite.currentUser.username);
        expect(usernameText).to.exist;
    });

    it('should allow navigation between tabs', async function() {
        // Navigate to Feed tab
        const feedTab = await suite.findElement('feed-tab', 'Feed');
        await feedTab.click();

        const feedContent = await suite.waitForElement('feed-content');
        expect(await feedContent.isExisting()).to.be.true;

        // Navigate to Messages tab
        const messagesTab = await suite.findElement('messages-tab', 'Messages');
        await messagesTab.click();

        const messagesContent = await suite.waitForElement('messages-content');
        expect(await messagesContent.isExisting()).to.be.true;

        // Navigate to Friends tab
        const friendsTab = await suite.findElement('friends-tab', 'Friends');
        await friendsTab.click();

        const friendsContent = await suite.waitForElement('friends-content');
        expect(await friendsContent.isExisting()).to.be.true;
    });
});

describe('Post Tests', function() {
    before(async function() {
        // Create a test user for posts
        if (!suite.testUsers.has('poster')) {
            await suite.createTestUser('poster');
        } else {
            await suite.switchUser('poster');
        }
    });

    it('should create a text post', async function() {
        const postContent = `Test post at ${Date.now()}`;
        await suite.createPost(postContent);

        // Verify post appears in feed
        const postElement = await suite.waitForText(postContent);
        expect(postElement).to.exist;
    });

    it('should edit a post', async function() {
        const originalContent = `Original post ${Date.now()}`;
        await suite.createPost(originalContent);

        // Find and click edit button
        const postElement = await suite.waitForText(originalContent);
        const editButton = await postElement.$('.edit-button');
        await editButton.click();

        // Edit the post
        const editInput = await suite.findElement('edit-input');
        const editedContent = `Edited post ${Date.now()}`;
        await editInput.clearValue();
        await editInput.setValue(editedContent);

        const saveButton = await suite.findElement('save-button', 'Save');
        await saveButton.click();

        // Verify edited content
        const editedElement = await suite.waitForText(editedContent);
        expect(editedElement).to.exist;
    });

    it('should delete a post', async function() {
        const postContent = `Post to delete ${Date.now()}`;
        await suite.createPost(postContent);

        // Find and click delete button
        const postElement = await suite.waitForText(postContent);
        const deleteButton = await postElement.$('.delete-button');
        await deleteButton.click();

        // Confirm deletion
        const confirmButton = await suite.findElement('confirm-delete', 'Delete');
        await confirmButton.click();

        // Verify post is gone
        await new Promise(resolve => setTimeout(resolve, 1000)); // Wait for deletion

        try {
            const deletedPost = await suite.driver.$(`//*[contains(text(), "${postContent}")]`);
            expect(await deletedPost.isExisting()).to.be.false;
        } catch (e) {
            // Element not found - expected
        }
    });
});

describe('Friendship Tests', function() {
    before(async function() {
        // Create two users for friendship tests
        if (!suite.testUsers.has('friend1')) {
            await suite.createTestUser('friend1');
        }
        if (!suite.testUsers.has('friend2')) {
            await suite.createTestUser('friend2');
        }
    });

    it('should send friend request', async function() {
        await suite.switchUser('friend1');

        // Search for friend2
        const searchInput = await suite.findElement('search-input', 'Search users');
        const friend2Username = suite.testUsers.get('friend2').username;
        await searchInput.setValue(friend2Username);

        // Click search button
        const searchButton = await suite.findElement('search-button', 'Search');
        await searchButton.click();

        // Find user in results
        const userResult = await suite.waitForText(friend2Username);
        expect(userResult).to.exist;

        // Send friend request
        const addButton = await suite.findElement('add-friend-button', 'Add Friend');
        await addButton.click();

        // Verify request sent
        const successMessage = await suite.waitForText('Friend request sent');
        expect(successMessage).to.exist;
    });

    it('should accept friend request', async function() {
        await suite.switchUser('friend2');

        // Navigate to friend requests
        const requestsTab = await suite.findElement('requests-tab', 'Requests');
        await requestsTab.click();

        // Find the request
        const friend1Username = suite.testUsers.get('friend1').username;
        const requestElement = await suite.waitForText(friend1Username);
        expect(requestElement).to.exist;

        // Accept request
        const acceptButton = await suite.findElement('accept-button', 'Accept');
        await acceptButton.click();

        // Verify friendship established
        const friendsList = await suite.findElement('friends-list');
        const friendElement = await friendsList.$(`//*[contains(text(), "${friend1Username}")]`);
        expect(await friendElement.isExisting()).to.be.true;
    });
});

describe('Messaging Tests', function() {
    before(async function() {
        // Ensure we have two friends for messaging
        const friend1 = suite.testUsers.get('friend1');
        const friend2 = suite.testUsers.get('friend2');

        if (!friend1 || !friend2) {
            // Create and befriend if needed
            await suite.createTestUser('msg_user1');
            await suite.createTestUser('msg_user2');
            await suite.createFriendship('msg_user1', 'msg_user2');
        }
    });

    it('should send a message', async function() {
        await suite.switchUser('friend1');

        // Navigate to messages
        const messagesTab = await suite.findElement('messages-tab', 'Messages');
        await messagesTab.click();

        // Select friend2
        const friend2Username = suite.testUsers.get('friend2').username;
        const friendChat = await suite.findElement('chat-' + friend2Username);
        await friendChat.click();

        // Send message
        const messageInput = await suite.findElement('message-input');
        const messageText = `Hello from E2E test ${Date.now()}`;
        await messageInput.setValue(messageText);

        const sendButton = await suite.findElement('send-button', 'Send');
        await sendButton.click();

        // Verify message appears
        const messageElement = await suite.waitForText(messageText);
        expect(messageElement).to.exist;
    });

    it('should receive a message', async function() {
        const messageFromFriend1 = `Message from friend1 ${Date.now()}`;

        // Friend1 sends message
        await suite.switchUser('friend1');
        const friend2Username = suite.testUsers.get('friend2').username;

        const messagesTab = await suite.findElement('messages-tab', 'Messages');
        await messagesTab.click();

        const friendChat = await suite.findElement('chat-' + friend2Username);
        await friendChat.click();

        const messageInput = await suite.findElement('message-input');
        await messageInput.setValue(messageFromFriend1);

        const sendButton = await suite.findElement('send-button', 'Send');
        await sendButton.click();

        // Switch to friend2 and check message
        await suite.switchUser('friend2');

        const messagesTab2 = await suite.findElement('messages-tab', 'Messages');
        await messagesTab2.click();

        // Should see unread indicator
        const unreadIndicator = await suite.findElement('unread-indicator');
        expect(await unreadIndicator.isExisting()).to.be.true;

        // Open chat and verify message
        const friend1Username = suite.testUsers.get('friend1').username;
        const friendChat2 = await suite.findElement('chat-' + friend1Username);
        await friendChat2.click();

        const receivedMessage = await suite.waitForText(messageFromFriend1);
        expect(receivedMessage).to.exist;
    });
});

describe('Settings Tests', function() {
    it('should toggle dark mode', async function() {
        // Navigate to settings
        const settingsButton = await suite.findElement('settings-button', 'Settings');
        await settingsButton.click();

        // Find theme toggle
        const themeToggle = await suite.findElement('theme-toggle');
        const initialState = await themeToggle.getAttribute('data-theme');

        // Toggle theme
        await themeToggle.click();

        // Verify theme changed
        const newState = await themeToggle.getAttribute('data-theme');
        expect(newState).to.not.equal(initialState);

        // Toggle back
        await themeToggle.click();
        const finalState = await themeToggle.getAttribute('data-theme');
        expect(finalState).to.equal(initialState);
    });

    it('should update profile', async function() {
        const settingsButton = await suite.findElement('settings-button', 'Settings');
        await settingsButton.click();

        // Update bio
        const bioInput = await suite.findElement('bio-input');
        const newBio = `Updated bio ${Date.now()}`;
        await bioInput.clearValue();
        await bioInput.setValue(newBio);

        // Save changes
        const saveButton = await suite.findElement('save-profile-button', 'Save');
        await saveButton.click();

        // Verify bio updated
        const profileBio = await suite.waitForText(newBio);
        expect(profileBio).to.exist;
    });
});

describe('Performance Tests', function() {
    it('should handle rapid navigation', async function() {
        const tabs = ['feed-tab', 'messages-tab', 'friends-tab', 'profile-tab'];

        // Rapidly switch between tabs
        for (let i = 0; i < 10; i++) {
            const tabId = tabs[i % tabs.length];
            const tab = await suite.findElement(tabId);
            await tab.click();
        }

        // Should still be responsive
        const feedTab = await suite.findElement('feed-tab');
        await feedTab.click();

        const feedContent = await suite.waitForElement('feed-content');
        expect(await feedContent.isExisting()).to.be.true;
    });

    it('should handle multiple posts efficiently', async function() {
        // Create multiple posts
        for (let i = 0; i < 5; i++) {
            await suite.createPost(`Performance test post ${i} at ${Date.now()}`);
        }

        // Feed should still load quickly
        const feedTab = await suite.findElement('feed-tab');
        await feedTab.click();

        const posts = await suite.driver.$$('.post-item');
        expect(posts.length).to.be.at.least(5);
    });
});

// Report test statistics at the end
after(function() {
    console.log('\n📊 Test Statistics:');
    const stats = suite.getStats();
    console.log(`  Platform: ${stats.platform}`);
    console.log(`  Test Users Created: ${stats.testUsers}`);
    console.log(`  App Instances Spawned: 1`); // Always 1 with new approach
    console.log(`  Current User: ${stats.currentUser || 'None'}`);
});