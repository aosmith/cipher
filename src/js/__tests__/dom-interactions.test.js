/**
 * DOM Interaction Tests
 * Testing user interface interactions and DOM manipulations
 */

require('./setup');
const { screen, fireEvent, waitFor } = require('@testing-library/dom');

// Mock external modules
global.Navbar = {
    init: jest.fn(),
    updateLoginState: jest.fn(),
    updatePublicKey: jest.fn()
};

global.P2P = {
    initialize: jest.fn().mockResolvedValue(),
    shutdown: jest.fn().mockResolvedValue(),
    announcePresence: jest.fn().mockResolvedValue(),
    initialized: false
};

describe('DOM Form Interactions', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div id="loginForm">
                <input id="loginUsername" placeholder="Username" />
                <input id="loginPassword" type="password" placeholder="Password" />
                <button id="loginButton" onclick="handleLogin()">Sign In</button>
                <div id="loginError" class="error hidden"></div>
                <div id="loginSuccess" class="success hidden"></div>
            </div>
            <div id="dashboard" class="hidden">
                <div id="userGreeting"></div>
                <button id="logoutButton" onclick="handleLogout()">Logout</button>
            </div>
        `;

        global.__TAURI__ = {
            invoke: jest.fn()
        };

        jest.clearAllMocks();
    });

    test('should show error when login fields are empty', async () => {
        const loginButton = document.getElementById('loginButton');
        fireEvent.click(loginButton);

        await waitFor(() => {
            const error = document.getElementById('loginError');
            expect(error.textContent).toBe('Please fill in all fields');
            expect(error.classList.contains('hidden')).toBe(false);
        });
    });

    test('should disable login button during authentication', async () => {
        const usernameInput = document.getElementById('loginUsername');
        const passwordInput = document.getElementById('loginPassword');
        const loginButton = document.getElementById('loginButton');

        fireEvent.change(usernameInput, { target: { value: 'testuser' } });
        fireEvent.change(passwordInput, { target: { value: 'password123' } });

        // Mock a delayed response
        global.__TAURI__.invoke.mockImplementation(() =>
            new Promise(resolve => setTimeout(() => resolve({ id: '123', username: 'testuser' }), 100))
        );

        fireEvent.click(loginButton);

        // Check button is disabled during async operation
        expect(loginButton.disabled || loginButton.getAttribute('disabled')).toBeTruthy();

        await waitFor(() => {
            expect(global.__TAURI__.invoke).toHaveBeenCalled();
        });
    });

    test('should handle Enter key in login form', () => {
        const usernameInput = document.getElementById('loginUsername');
        const passwordInput = document.getElementById('loginPassword');

        fireEvent.change(usernameInput, { target: { value: 'testuser' } });
        fireEvent.change(passwordInput, { target: { value: 'password123' } });

        // Simulate Enter key press
        fireEvent.keyPress(passwordInput, { key: 'Enter', code: 'Enter', charCode: 13 });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('login_user', {
            username: 'testuser',
            password: 'password123'
        });
    });
});

describe('Tab Navigation', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <nav>
                <button id="postsNavLink" onclick="showPosts()">Posts</button>
                <button id="messagesNavLink" onclick="showMessages()">Messages</button>
                <button id="friendsNavLink" onclick="showFriends()">Friends</button>
                <button id="createPostNavLink" onclick="showCreatePostPage()">Create</button>
                <button id="profileNavLink" onclick="showEditProfile()">Profile</button>
            </nav>
            <div id="postsTab" class="tab-content">Posts Content</div>
            <div id="messagesTab" class="tab-content hidden">Messages Content</div>
            <div id="friendsTab" class="tab-content hidden">Friends Content</div>
            <div id="createPostTab" class="tab-content hidden">Create Post Content</div>
            <div id="profileTab" class="tab-content hidden">Profile Content</div>
        `;

        global.currentUser = { id: '123', username: 'testuser' };
        global.__TAURI__ = { invoke: jest.fn().mockResolvedValue([]) };
    });

    test('should show posts tab and hide others', () => {
        showPosts();

        expect(document.getElementById('postsTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('messagesTab').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('friendsTab').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('postsNavLink').classList.contains('active')).toBe(true);
    });

    test('should switch between tabs correctly', () => {
        // Start with posts tab
        showPosts();
        expect(document.getElementById('postsTab').classList.contains('hidden')).toBe(false);

        // Switch to messages
        showMessages();
        expect(document.getElementById('postsTab').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('messagesTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('messagesNavLink').classList.contains('active')).toBe(true);
        expect(document.getElementById('postsNavLink').classList.contains('active')).toBe(false);

        // Switch to friends
        showFriends();
        expect(document.getElementById('messagesTab').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('friendsTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('friendsNavLink').classList.contains('active')).toBe(true);
        expect(document.getElementById('messagesNavLink').classList.contains('active')).toBe(false);
    });
});

describe('Post Creation Form', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div id="createPostTab">
                <textarea id="createPostTextarea" placeholder="What's on your mind?"></textarea>
                <input type="file" id="createPostAttachments" multiple />
                <div id="fileCount"></div>
                <button onclick="createPostFromPage()">Share Post</button>
            </div>
            <div id="postsTab" class="hidden">
                <div id="posts"></div>
                <div id="postsStatusMessage"></div>
            </div>
            <div id="dashboardError" class="error hidden"></div>
        `;

        global.currentUser = { id: '123', username: 'testuser' };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('should validate post content before submission', async () => {
        const textarea = document.getElementById('createPostTextarea');
        const submitButton = document.querySelector('button[onclick="createPostFromPage()"]');

        // Try to submit empty post
        fireEvent.click(submitButton);

        await waitFor(() => {
            expect(global.alert).toHaveBeenCalledWith('Please enter some content for your post');
        });

        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('should display file count when files are selected', () => {
        const fileInput = document.getElementById('createPostAttachments');
        const fileCount = document.getElementById('fileCount');

        // Create mock files
        const file1 = new File(['content1'], 'file1.txt', { type: 'text/plain' });
        const file2 = new File(['content2'], 'file2.jpg', { type: 'image/jpeg' });

        // Simulate file selection
        Object.defineProperty(fileInput, 'files', {
            value: [file1, file2],
            writable: false
        });

        fireEvent.change(fileInput);

        expect(fileCount.textContent).toBe('2 files selected');
    });

    test('should clear form after successful post creation', async () => {
        const textarea = document.getElementById('createPostTextarea');
        const fileInput = document.getElementById('createPostAttachments');
        const fileCount = document.getElementById('fileCount');

        textarea.value = 'Test post content';
        fileCount.textContent = '1 file selected';

        global.__TAURI__.invoke.mockResolvedValue({
            id: '456',
            content: 'Test post content',
            userId: '123'
        });

        await createPostFromPage();

        expect(textarea.value).toBe('');
        expect(fileInput.value).toBe('');
        expect(fileCount.textContent).toBe('');
    });
});

describe('Friend Search Interface', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <input id="friendSearch" placeholder="Search friends..." />
            <div id="friendSearchResults" class="hidden"></div>
            <div id="selectedRecipient">
                <span class="no-selection">No friend selected</span>
            </div>
            <textarea id="messageContent"></textarea>
            <button onclick="sendMessage()">Send Message</button>
            <div id="dashboardError" class="error hidden"></div>
        `;

        global.currentUser = { id: '123', username: 'testuser' };
        global.allFriends = [
            { id: '456', friendUsername: 'Alice' },
            { id: '789', friendUsername: 'Bob' },
            { id: '101', friendUsername: 'Charlie' }
        ];
    });

    test('should filter friends based on search input', () => {
        const searchInput = document.getElementById('friendSearch');
        const resultsContainer = document.getElementById('friendSearchResults');

        setupFriendSearch();

        // Search for 'al' should match 'Alice'
        fireEvent.input(searchInput, { target: { value: 'al' } });

        expect(resultsContainer.classList.contains('hidden')).toBe(false);
        expect(resultsContainer.innerHTML).toContain('Alice');
        expect(resultsContainer.innerHTML).not.toContain('Bob');
        expect(resultsContainer.innerHTML).not.toContain('Charlie');
    });

    test('should hide results when search is cleared', () => {
        const searchInput = document.getElementById('friendSearch');
        const resultsContainer = document.getElementById('friendSearchResults');

        setupFriendSearch();

        fireEvent.input(searchInput, { target: { value: 'alice' } });
        expect(resultsContainer.classList.contains('hidden')).toBe(false);

        fireEvent.input(searchInput, { target: { value: '' } });
        expect(resultsContainer.classList.contains('hidden')).toBe(true);
    });

    test('should select friend when clicked', () => {
        const searchInput = document.getElementById('friendSearch');
        const selectedContainer = document.getElementById('selectedRecipient');

        setupFriendSearch();

        fireEvent.input(searchInput, { target: { value: 'alice' } });

        // Click on Alice in results
        const aliceResult = document.querySelector('.friend-search-item');
        fireEvent.click(aliceResult);

        expect(global.selectedRecipient).toEqual({
            id: '456',
            username: 'Alice'
        });

        expect(selectedContainer.innerHTML).toContain('Alice');
        expect(selectedContainer.innerHTML).toContain('A'); // Avatar initial
        expect(searchInput.value).toBe('');
    });

    test('should hide search results when clicking outside', () => {
        const searchInput = document.getElementById('friendSearch');
        const resultsContainer = document.getElementById('friendSearchResults');

        setupFriendSearch();

        fireEvent.input(searchInput, { target: { value: 'alice' } });
        expect(resultsContainer.classList.contains('hidden')).toBe(false);

        // Click outside
        fireEvent.click(document.body);

        expect(resultsContainer.classList.contains('hidden')).toBe(true);
    });
});

describe('Message Editing Interface', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div id="messages">
                <div data-message-id="789" class="post">
                    <div class="message-content">Original message</div>
                    <div class="message-actions">
                        <button onclick="editMessage(789)">Edit</button>
                        <button onclick="deleteMessage(789)">Delete</button>
                    </div>
                </div>
            </div>
        `;

        global.currentUser = { id: '123', username: 'testuser' };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('should show edit form when edit button clicked', () => {
        const editButton = document.querySelector('button[onclick="editMessage(789)"]');
        fireEvent.click(editButton);

        const messageElement = document.querySelector('[data-message-id="789"]');
        expect(messageElement.querySelector('.message-edit-form')).toBeTruthy();
        expect(messageElement.classList.contains('message-edit-mode')).toBe(true);

        const textarea = document.getElementById('edit-textarea-789');
        expect(textarea).toBeTruthy();
        expect(textarea.value).toBe('Original message');
    });

    test('should cancel edit and restore original view', () => {
        editMessage(789);

        const messageElement = document.querySelector('[data-message-id="789"]');
        expect(messageElement.querySelector('.message-edit-form')).toBeTruthy();

        cancelEditMessage(789);

        expect(messageElement.querySelector('.message-edit-form')).toBeFalsy();
        expect(messageElement.classList.contains('message-edit-mode')).toBe(false);
        expect(messageElement.querySelector('.message-content').style.display).toBe('block');
    });

    test('should save edited message', async () => {
        editMessage(789);

        const textarea = document.getElementById('edit-textarea-789');
        textarea.value = 'Updated message';

        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        await saveEditMessage(789);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('edit_message', {
            messageId: 789,
            userId: '123',
            newContent: 'Updated message'
        });
    });
});

describe('Voice Recording Interface', () => {
    let mockMediaRecorder;
    let mockStream;

    beforeEach(() => {
        document.body.innerHTML = `
            <button id="voiceRecordButton" onclick="startVoiceRecording()">
                🎤 Record Voice Message
            </button>
            <div id="dashboardError" class="error hidden"></div>
        `;

        global.currentUser = { id: '123' };
        global.selectedRecipient = { id: '456', username: 'friend' };

        mockStream = {
            getTracks: jest.fn().mockReturnValue([{ stop: jest.fn() }])
        };

        mockMediaRecorder = {
            start: jest.fn(),
            stop: jest.fn(),
            ondataavailable: null,
            onstop: null
        };

        global.MediaRecorder = jest.fn().mockImplementation(() => mockMediaRecorder);
        global.navigator.mediaDevices = {
            getUserMedia: jest.fn().mockResolvedValue(mockStream)
        };
    });

    test('should change button appearance when recording starts', async () => {
        const button = document.getElementById('voiceRecordButton');

        await startVoiceRecording();

        expect(button.textContent).toBe('🛑 Stop Recording');
        expect(button.classList.contains('recording')).toBe(true);
        expect(button.onclick).toBe(stopVoiceRecording);
    });

    test('should restore button when recording stops', () => {
        const button = document.getElementById('voiceRecordButton');
        button.textContent = '🛑 Stop Recording';
        button.classList.add('recording');
        button.onclick = stopVoiceRecording;

        global.mediaRecorder = mockMediaRecorder;
        global.isRecording = true;

        stopVoiceRecording();

        expect(button.textContent).toBe('🎤 Record Voice Message');
        expect(button.classList.contains('recording')).toBe(false);
        expect(button.onclick).toBe(startVoiceRecording);
    });

    test('should handle microphone permission error', async () => {
        const permissionError = new Error('Permission denied');
        global.navigator.mediaDevices.getUserMedia.mockRejectedValue(permissionError);

        await startVoiceRecording();

        const error = document.getElementById('dashboardError');
        expect(error.textContent).toContain('Failed to start voice recording');
        expect(error.classList.contains('hidden')).toBe(false);
    });
});

describe('QR Code Scanning', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <button onclick="scanQRCode()">Scan QR Code</button>
            <input type="file" id="qrCodeFile" style="display:none" onchange="handleQRCodeFile(event)" />
            <input id="friendPublicKey" />
            <div id="dashboardError" class="error hidden"></div>
        `;

        global.currentUser = { id: '123', publicKey: 'my-key' };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('should use file picker on desktop platform', async () => {
        global.__TAURI__.invoke.mockResolvedValueOnce('desktop');

        const fileInput = document.getElementById('qrCodeFile');
        const clickSpy = jest.spyOn(fileInput, 'click');

        await scanQRCode();

        expect(clickSpy).toHaveBeenCalled();
    });

    test('should process uploaded QR code file', async () => {
        const mockQRData = {
            username: 'friend',
            publicKey: 'friend-key'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockQRData);

        const file = new File(['qr-image'], 'qr.png', { type: 'image/png' });
        const event = {
            target: {
                files: [file],
                value: 'qr.png'
            }
        };

        await handleQRCodeFile(event);

        expect(document.getElementById('friendPublicKey').value).toBe('friend-key');
        expect(event.target.value).toBe('');
    });

    test('should show error for invalid QR code', async () => {
        global.__TAURI__.invoke.mockResolvedValue({});

        const file = new File(['invalid'], 'bad.png', { type: 'image/png' });
        const event = { target: { files: [file], value: 'bad.png' } };

        await handleQRCodeFile(event);

        const error = document.getElementById('dashboardError');
        expect(error.textContent).toContain('Invalid QR code');
        expect(error.classList.contains('hidden')).toBe(false);
    });
});

describe('Responsive Menu', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <button id="hamburgerBtn" onclick="toggleHamburgerMenu()">☰</button>
            <div id="navMenu" class="hidden">
                <a href="#" onclick="showPosts(); closeHamburgerMenu();">Posts</a>
                <a href="#" onclick="showMessages(); closeHamburgerMenu();">Messages</a>
                <a href="#" onclick="showFriends(); closeHamburgerMenu();">Friends</a>
            </div>
            <div id="navBackdrop" onclick="closeHamburgerMenu()"></div>
        `;
    });

    test('should toggle menu visibility', () => {
        const menu = document.getElementById('navMenu');
        const hamburger = document.getElementById('hamburgerBtn');
        const backdrop = document.getElementById('navBackdrop');

        toggleHamburgerMenu();

        expect(menu.classList.contains('hidden')).toBe(false);
        expect(hamburger.classList.contains('open')).toBe(true);
        expect(backdrop.classList.contains('visible')).toBe(true);

        toggleHamburgerMenu();

        expect(menu.classList.contains('hidden')).toBe(true);
        expect(hamburger.classList.contains('open')).toBe(false);
        expect(backdrop.classList.contains('visible')).toBe(false);
    });

    test('should close menu when backdrop is clicked', () => {
        const menu = document.getElementById('navMenu');
        const backdrop = document.getElementById('navBackdrop');

        // Open menu first
        toggleHamburgerMenu();
        expect(menu.classList.contains('hidden')).toBe(false);

        // Click backdrop
        fireEvent.click(backdrop);

        expect(menu.classList.contains('hidden')).toBe(true);
    });

    test('should close menu when menu item is clicked', () => {
        const menu = document.getElementById('navMenu');

        toggleHamburgerMenu();
        expect(menu.classList.contains('hidden')).toBe(false);

        const postsLink = menu.querySelector('a[onclick*="showPosts"]');
        fireEvent.click(postsLink);

        expect(menu.classList.contains('hidden')).toBe(true);
    });
});

describe('Drag and Drop', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div id="createPostTab">
                <div id="dropZone" class="drop-zone">
                    <p>Drag files here</p>
                    <input type="file" id="createPostAttachments" multiple />
                    <div id="fileCount"></div>
                </div>
            </div>
        `;
    });

    test('should handle drag over event', () => {
        const dropZone = document.getElementById('dropZone');

        const dragEvent = new Event('dragover');
        dragEvent.preventDefault = jest.fn();
        dragEvent.dataTransfer = { effectAllowed: 'copy' };

        fireEvent(dropZone, dragEvent);

        expect(dragEvent.preventDefault).toHaveBeenCalled();
        expect(dropZone.classList.contains('drag-over')).toBe(true);
    });

    test('should handle drag leave event', () => {
        const dropZone = document.getElementById('dropZone');
        dropZone.classList.add('drag-over');

        fireEvent.dragLeave(dropZone);

        expect(dropZone.classList.contains('drag-over')).toBe(false);
    });

    test('should handle file drop', () => {
        const dropZone = document.getElementById('dropZone');
        const fileInput = document.getElementById('createPostAttachments');
        const fileCount = document.getElementById('fileCount');

        const file1 = new File(['content'], 'test.txt', { type: 'text/plain' });
        const file2 = new File(['image'], 'test.jpg', { type: 'image/jpeg' });

        const dropEvent = new Event('drop');
        dropEvent.preventDefault = jest.fn();
        dropEvent.dataTransfer = { files: [file1, file2] };

        fireEvent(dropZone, dropEvent);

        expect(dropEvent.preventDefault).toHaveBeenCalled();
        expect(fileInput.files).toEqual([file1, file2]);
        expect(fileCount.textContent).toContain('2 files');
    });
});

describe('Keyboard Shortcuts', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div id="dashboard">
                <input id="messageSearchInput" />
                <button onclick="searchMessages()">Search</button>
            </div>
        `;

        global.currentUser = { id: '123' };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('should trigger search on Enter key', () => {
        const searchInput = document.getElementById('messageSearchInput');
        searchInput.value = 'test query';

        // Add event listener
        searchInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                searchMessages();
            }
        });

        const enterEvent = new KeyboardEvent('keypress', { key: 'Enter' });
        searchInput.dispatchEvent(enterEvent);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('search_messages', {
            userId: '123',
            query: 'test query'
        });
    });

    test('should navigate tabs with keyboard', () => {
        // Setup tab navigation
        const tabs = ['postsTab', 'messagesTab', 'friendsTab'];
        let currentTab = 0;

        document.addEventListener('keydown', (e) => {
            if (e.ctrlKey && e.key === 'ArrowRight') {
                currentTab = (currentTab + 1) % tabs.length;
                const tabId = tabs[currentTab];
                if (tabId === 'postsTab') showPosts();
                else if (tabId === 'messagesTab') showMessages();
                else if (tabId === 'friendsTab') showFriends();
            }
        });

        // Simulate Ctrl+ArrowRight
        const keyEvent = new KeyboardEvent('keydown', {
            key: 'ArrowRight',
            ctrlKey: true
        });

        document.dispatchEvent(keyEvent);

        // Check navigation occurred
        expect(currentTab).toBe(1);
    });
});

describe('Copy to Clipboard', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div id="userPublicKey">test-public-key-123</div>
            <button class="btn-copy" onclick="copyPublicKey()">Copy Key</button>
            <div id="dashboardError" class="error hidden"></div>
        `;

        // Mock clipboard API
        global.navigator.clipboard = {
            writeText: jest.fn().mockResolvedValue()
        };
    });

    test('should copy public key to clipboard', async () => {
        const button = document.querySelector('.btn-copy');

        await copyPublicKey();

        expect(navigator.clipboard.writeText).toHaveBeenCalledWith('test-public-key-123');
        expect(button.textContent).toBe('Copied!');

        // Wait for button to reset
        await new Promise(resolve => setTimeout(resolve, 2100));
        expect(button.textContent).toContain('Copy');
    });

    test('should handle clipboard error gracefully', async () => {
        navigator.clipboard.writeText.mockRejectedValue(new Error('Clipboard access denied'));

        await copyPublicKey();

        const error = document.getElementById('dashboardError');
        expect(error.textContent).toContain('Failed to copy public key');
        expect(error.classList.contains('hidden')).toBe(false);
    });
});

describe('Visibility Change Handling', () => {
    beforeEach(() => {
        global.P2P.initialized = true;
        global.currentUser = { id: '123' };
    });

    test('should announce presence when app becomes visible', async () => {
        // Simulate app becoming visible
        Object.defineProperty(document, 'hidden', {
            configurable: true,
            get: () => false
        });

        const visibilityEvent = new Event('visibilitychange');
        document.dispatchEvent(visibilityEvent);

        await waitFor(() => {
            expect(P2P.announcePresence).toHaveBeenCalled();
        });
    });

    test('should not announce when app becomes hidden', async () => {
        Object.defineProperty(document, 'hidden', {
            configurable: true,
            get: () => true
        });

        P2P.announcePresence.mockClear();

        const visibilityEvent = new Event('visibilitychange');
        document.dispatchEvent(visibilityEvent);

        await new Promise(resolve => setTimeout(resolve, 100));
        expect(P2P.announcePresence).not.toHaveBeenCalled();
    });

    test('should announce presence on window focus', async () => {
        P2P.announcePresence.mockClear();

        const focusEvent = new Event('focus');
        window.dispatchEvent(focusEvent);

        await waitFor(() => {
            expect(P2P.announcePresence).toHaveBeenCalled();
        });
    });
});