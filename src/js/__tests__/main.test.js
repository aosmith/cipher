/**
 * Unit Tests for main.js
 * Testing core application functionality
 */

require('./setup');

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

// Import main.js functions after mocking
beforeEach(() => {
    // Reset DOM
    document.body.innerHTML = `
        <div id="loginForm" class="hidden"></div>
        <div id="dashboard" class="hidden"></div>
        <input id="loginUsername" />
        <input id="loginPassword" />
        <div id="loginError" class="error hidden"></div>
        <div id="loginSuccess" class="success hidden"></div>
        <div id="dashboardError" class="error hidden"></div>
        <div id="posts"></div>
        <div id="messages"></div>
        <div id="friends"></div>
        <div id="userGreeting"></div>
        <div id="userPublicKey"></div>
        <div id="postsStatusMessage"></div>
        <div id="postsTab" class="hidden"></div>
        <div id="messagesTab" class="hidden"></div>
        <div id="friendsTab" class="hidden"></div>
        <div id="profileTab" class="hidden"></div>
        <div id="createPostTab" class="hidden"></div>
        <div id="postsNavLink"></div>
        <div id="messagesNavLink"></div>
        <div id="friendsNavLink"></div>
        <div id="profileNavLink"></div>
        <div id="createPostNavLink"></div>
        <div id="friendSearch"></div>
        <div id="friendSearchResults" class="hidden"></div>
        <div id="selectedRecipient"></div>
        <textarea id="messageContent"></textarea>
        <input id="friendPublicKey" />
        <input id="addFriendPublicKey" />
        <input id="qrCodeFile" type="file" />
        <div id="myQrCode"></div>
        <div id="profileQrCode"></div>
    `;

    // Clear all mocks
    jest.clearAllMocks();

    // Reset global variables
    global.currentUser = null;
    global.allFriends = [];
    global.selectedRecipient = null;
    global.tauriInvoke = null;
});

describe('Utils', () => {
    test('escapeHtml should escape HTML characters', () => {
        const dangerous = '<script>alert("xss")</script>';
        const escaped = Utils.escapeHtml(dangerous);
        expect(escaped).not.toContain('<script>');
        expect(escaped).toContain('&lt;script&gt;');
    });

    test('formatFileSize should format bytes correctly', () => {
        expect(Utils.formatFileSize(0)).toBe('0 Bytes');
        expect(Utils.formatFileSize(1024)).toBe('1 KB');
        expect(Utils.formatFileSize(1048576)).toBe('1 MB');
        expect(Utils.formatFileSize(1073741824)).toBe('1 GB');
        expect(Utils.formatFileSize(1500)).toBe('1.46 KB');
    });

    test('getMediaIcon should return correct icon for file types', () => {
        expect(Utils.getMediaIcon('image/png')).toBe('🖼️');
        expect(Utils.getMediaIcon('video/mp4')).toBe('🎬');
        expect(Utils.getMediaIcon('audio/mp3')).toBe('🎵');
        expect(Utils.getMediaIcon('application/pdf')).toBe('📄');
        expect(Utils.getMediaIcon('text/plain')).toBe('📎');
    });

    test('getMediaIconClass should return correct class for file types', () => {
        expect(Utils.getMediaIconClass('image/jpeg')).toBe('media-icon-image');
        expect(Utils.getMediaIconClass('video/webm')).toBe('media-icon-video');
        expect(Utils.getMediaIconClass('audio/wav')).toBe('media-icon-audio');
        expect(Utils.getMediaIconClass('application/pdf')).toBe('media-icon-pdf');
        expect(Utils.getMediaIconClass('text/html')).toBe('media-icon-file');
    });

    test('fileToBase64 should convert file to base64', async () => {
        const file = new File(['hello world'], 'test.txt', { type: 'text/plain' });
        const base64 = await Utils.fileToBase64(file);
        expect(base64).toContain('data:text/plain;base64,');
        expect(base64).toContain('aGVsbG8gd29ybGQ='); // "hello world" in base64
    });

    test('delay should wait specified milliseconds', async () => {
        const start = Date.now();
        await Utils.delay(100);
        const elapsed = Date.now() - start;
        expect(elapsed).toBeGreaterThanOrEqual(90); // Allow some variance
        expect(elapsed).toBeLessThan(150);
    });
});

describe('TauriAPI', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn().mockResolvedValue({ success: true })
        };
    });

    test('should initialize with Tauri 1.x API', async () => {
        const result = await TauriAPI.initialize();
        expect(result).toBe(true);
        expect(global.tauriInvoke).toBe(global.__TAURI__.invoke);
    });

    test('should initialize with Tauri 2.x API', async () => {
        delete global.__TAURI__.invoke;
        global.__TAURI__.core = {
            invoke: jest.fn().mockResolvedValue({ success: true })
        };

        const result = await TauriAPI.initialize();
        expect(result).toBe(true);
        expect(global.tauriInvoke).toBe(global.__TAURI__.core.invoke);
    });

    test('invoke should call Tauri API with correct arguments', async () => {
        await TauriAPI.initialize();
        const result = await TauriAPI.invoke('test_command', { arg1: 'value1' });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('test_command', { arg1: 'value1' });
        expect(result).toEqual({ success: true });
    });

    test('invoke should wait for API if not initialized', async () => {
        global.tauriInvoke = null;
        const result = await TauriAPI.invoke('test_command');

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('test_command', {});
        expect(result).toEqual({ success: true });
    });
});

describe('Session Management', () => {
    const mockUser = {
        id: '123',
        username: 'testuser',
        publicKey: 'test-public-key',
        deviceId: 'test-device-id'
    };

    beforeEach(() => {
        localStorage.clear();
    });

    test('save should store user data in localStorage', () => {
        Session.save(mockUser);

        expect(localStorage.setItem).toHaveBeenCalledWith(
            'cipher_user_session',
            JSON.stringify(mockUser)
        );
        expect(localStorage.setItem).toHaveBeenCalledWith(
            'cipher_last_login',
            expect.any(String)
        );
    });

    test('load should retrieve user data from localStorage', () => {
        localStorage.getItem.mockImplementation((key) => {
            if (key === 'cipher_user_session') return JSON.stringify(mockUser);
            if (key === 'cipher_last_login') return Date.now().toString();
            return null;
        });

        const user = Session.load();
        expect(user).toEqual(mockUser);
    });

    test('load should return null if session is expired', () => {
        const thirtyOneDaysAgo = Date.now() - (31 * 24 * 60 * 60 * 1000);
        localStorage.getItem.mockImplementation((key) => {
            if (key === 'cipher_user_session') return JSON.stringify(mockUser);
            if (key === 'cipher_last_login') return thirtyOneDaysAgo.toString();
            return null;
        });

        const user = Session.load();
        expect(user).toBeNull();
    });

    test('clear should remove session data from localStorage', () => {
        Session.clear();

        expect(localStorage.removeItem).toHaveBeenCalledWith('cipher_user_session');
        expect(localStorage.removeItem).toHaveBeenCalledWith('cipher_last_login');
    });

    test('attemptAutoLogin should restore user and initialize P2P', async () => {
        localStorage.getItem.mockImplementation((key) => {
            if (key === 'cipher_user_session') return JSON.stringify(mockUser);
            if (key === 'cipher_last_login') return Date.now().toString();
            return null;
        });

        const result = await Session.attemptAutoLogin();

        expect(result).toBe(true);
        expect(global.currentUser).toEqual(mockUser);
        expect(P2P.initialize).toHaveBeenCalledWith(
            mockUser.id,
            mockUser.publicKey,
            mockUser.deviceId
        );
    });
});

describe('UI State Management', () => {
    test('clearErrors should hide all error and success elements', () => {
        document.getElementById('loginError').classList.remove('hidden');
        document.getElementById('loginSuccess').classList.remove('hidden');

        UI.clearErrors();

        expect(document.getElementById('loginError').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('loginSuccess').classList.contains('hidden')).toBe(true);
    });

    test('showError should display error message', () => {
        UI.showError('loginError', 'Test error message');

        const element = document.getElementById('loginError');
        expect(element.textContent).toBe('Test error message');
        expect(element.classList.contains('hidden')).toBe(false);
    });

    test('showSuccess should display success message', () => {
        UI.showSuccess('loginSuccess', 'Test success message');

        const element = document.getElementById('loginSuccess');
        expect(element.textContent).toBe('Test success message');
        expect(element.classList.contains('hidden')).toBe(false);
    });

    test('setActiveNavLink should set active class on correct nav item', () => {
        UI.setActiveNavLink('postsNavLink');

        expect(document.getElementById('postsNavLink').classList.contains('active')).toBe(true);
        expect(document.getElementById('messagesNavLink').classList.contains('active')).toBe(false);
    });

    test('hideAllTabs should hide all tab elements', () => {
        document.getElementById('postsTab').classList.remove('hidden');
        document.getElementById('messagesTab').classList.remove('hidden');

        UI.hideAllTabs();

        expect(document.getElementById('postsTab').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('messagesTab').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('friendsTab').classList.contains('hidden')).toBe(true);
    });

    test('showTab should show specified tab and hide others', () => {
        const loadFunction = jest.fn();
        UI.showTab('postsTab', 'postsContent', 'postsNavLink', loadFunction);

        expect(document.getElementById('postsTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('messagesTab').classList.contains('hidden')).toBe(true);
        expect(loadFunction).toHaveBeenCalled();
    });

    test('updateUserInterface should update user display elements', () => {
        global.currentUser = {
            username: 'testuser',
            publicKey: 'test-public-key'
        };

        UI.updateUserInterface();

        expect(document.getElementById('userGreeting').textContent).toBe('testuser');
        expect(document.getElementById('userPublicKey').textContent).toBe('test-public-key');
        expect(Navbar.updatePublicKey).toHaveBeenCalledWith('test-public-key');
    });
});

describe('Authentication', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('handleLogin should validate input fields', async () => {
        document.getElementById('loginUsername').value = '';
        document.getElementById('loginPassword').value = '';

        await handleLogin();

        expect(document.getElementById('loginError').textContent).toBe('Please fill in all fields');
        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('handleLogin should attempt login with credentials', async () => {
        const mockUser = {
            id: '123',
            username: 'testuser',
            publicKey: 'test-key',
            deviceId: 'device-123'
        };

        global.__TAURI__.invoke.mockResolvedValueOnce(mockUser);

        document.getElementById('loginUsername').value = 'testuser';
        document.getElementById('loginPassword').value = 'password123';

        await handleLogin();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('login_user', {
            username: 'testuser',
            password: 'password123'
        });

        expect(global.currentUser).toEqual(mockUser);
        expect(P2P.initialize).toHaveBeenCalledWith(
            mockUser.id,
            mockUser.publicKey,
            mockUser.deviceId
        );
    });

    test('handleLogin should auto-register new users', async () => {
        const mockUser = {
            id: '456',
            username: 'newuser',
            publicKey: 'new-key',
            deviceId: 'device-456'
        };

        // First login attempt returns null (user not found)
        global.__TAURI__.invoke.mockResolvedValueOnce(null);
        // Registration succeeds
        global.__TAURI__.invoke.mockResolvedValueOnce(mockUser);

        document.getElementById('loginUsername').value = 'newuser';
        document.getElementById('loginPassword').value = 'password456';

        await handleLogin();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('login_user', {
            username: 'newuser',
            password: 'password456'
        });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('register_user', {
            username: 'newuser',
            email: null,
            password: 'password456'
        });

        expect(global.currentUser).toEqual(mockUser);
    });

    test('handleLogout should clear session and shutdown P2P', async () => {
        global.currentUser = { id: '123', username: 'testuser' };

        await handleLogout();

        expect(P2P.shutdown).toHaveBeenCalled();
        expect(global.currentUser).toBeNull();
        expect(localStorage.removeItem).toHaveBeenCalledWith('cipher_user_session');
        expect(document.getElementById('loginForm').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('dashboard').classList.contains('hidden')).toBe(true);
    });
});

describe('Navigation Functions', () => {
    test('showLogin should display login form and hide dashboard', () => {
        document.getElementById('dashboard').classList.remove('hidden');

        showLogin();

        expect(document.getElementById('loginForm').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('dashboard').classList.contains('hidden')).toBe(true);
        expect(Navbar.updateLoginState).toHaveBeenCalledWith(false);
    });

    test('showDashboard should display dashboard and hide login', () => {
        global.currentUser = { username: 'testuser' };
        global.__TAURI__ = { invoke: jest.fn().mockResolvedValue([]) };

        showDashboard();

        expect(document.getElementById('loginForm').classList.contains('hidden')).toBe(true);
        expect(document.getElementById('dashboard').classList.contains('hidden')).toBe(false);
        expect(Navbar.updateLoginState).toHaveBeenCalledWith(true);
    });

    test('showFeed should display posts tab', () => {
        global.currentUser = { id: '123' };
        global.__TAURI__ = { invoke: jest.fn().mockResolvedValue([]) };

        showFeed();

        expect(document.getElementById('postsTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('postsNavLink').classList.contains('active')).toBe(true);
    });

    test('showMessages should display messages tab', () => {
        global.currentUser = { id: '123' };
        global.__TAURI__ = { invoke: jest.fn().mockResolvedValue([]) };

        showMessages();

        expect(document.getElementById('messagesTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('messagesNavLink').classList.contains('active')).toBe(true);
    });

    test('showFriends should display friends tab', () => {
        global.currentUser = { id: '123' };
        global.__TAURI__ = { invoke: jest.fn().mockResolvedValue([]) };

        showFriends();

        expect(document.getElementById('friendsTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('friendsNavLink').classList.contains('active')).toBe(true);
    });

    test('showCreatePostPage should display create post tab and clear form', () => {
        document.body.innerHTML += `
            <textarea id="createPostTextarea">Old content</textarea>
            <input id="createPostAttachments" type="file" />
            <div id="fileCount">2 files</div>
        `;

        showCreatePostPage();

        expect(document.getElementById('createPostTab').classList.contains('hidden')).toBe(false);
        expect(document.getElementById('createPostTextarea').value).toBe('');
        expect(document.getElementById('createPostAttachments').value).toBe('');
        expect(document.getElementById('fileCount').textContent).toBe('');
    });
});

describe('Post Management', () => {
    beforeEach(() => {
        global.currentUser = { id: '123', username: 'testuser' };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('PostManager.create should create post with content', async () => {
        const mockPost = { id: '456', content: 'Test post', userId: '123' };
        global.__TAURI__.invoke.mockResolvedValue(mockPost);

        const post = await PostManager.create('Test post');

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('create_post', {
            userId: '123',
            content: 'Test post',
            attachments: null
        });
        expect(post).toEqual(mockPost);
    });

    test('PostManager.create should upload attachments if provided', async () => {
        const mockPost = { id: '456', content: 'Test post', userId: '123' };
        global.__TAURI__.invoke.mockResolvedValue(mockPost);

        const files = [
            new File(['content1'], 'file1.txt', { type: 'text/plain' }),
            new File(['content2'], 'file2.txt', { type: 'text/plain' })
        ];

        await PostManager.create('Test post', files);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('create_post', expect.any(Object));
        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('upload_media_file', expect.objectContaining({
            filename: 'file1.txt',
            fileType: 'text/plain',
            postId: '456'
        }));
        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('upload_media_file', expect.objectContaining({
            filename: 'file2.txt',
            fileType: 'text/plain',
            postId: '456'
        }));
    });

    test('PostManager.getMediaAttachments should fetch attachments for post', async () => {
        const mockAttachments = [
            { id: '1', fileType: 'image/png', filename: 'test.png' }
        ];
        global.__TAURI__.invoke.mockResolvedValue(mockAttachments);

        const attachments = await PostManager.getMediaAttachments('456');

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('get_media_attachments', {
            postId: '456'
        });
        expect(attachments).toEqual(mockAttachments);
    });

    test('PostManager.createMediaPreview should create image preview', () => {
        const media = {
            id: '1',
            fileType: 'image/png',
            data: 'base64data'
        };

        const preview = PostManager.createMediaPreview(media);

        expect(preview).toContain('<img src="data:image/png;base64,base64data"');
        expect(preview).toContain('class="post-image"');
    });

    test('PostManager.createMediaPreview should create icon for non-images', () => {
        const media = {
            id: '2',
            fileType: 'application/pdf'
        };

        const preview = PostManager.createMediaPreview(media);

        expect(preview).toContain('class="media-icon media-icon-pdf"');
        expect(preview).toContain('📄');
    });
});

describe('Friend Management', () => {
    beforeEach(() => {
        global.currentUser = { id: '123', username: 'testuser', publicKey: 'my-key' };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('addFriendByPublicKey should validate input', async () => {
        document.getElementById('friendPublicKey').value = '';

        await addFriendByPublicKey();

        const error = document.getElementById('dashboardError');
        expect(error.textContent).toBe('Please enter a valid public key');
        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('addFriendByPublicKey should prevent adding self', async () => {
        document.getElementById('friendPublicKey').value = 'my-key';

        await addFriendByPublicKey();

        const error = document.getElementById('dashboardError');
        expect(error.textContent).toBe('You cannot add yourself as a friend');
        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('addFriendByPublicKey should add friend successfully', async () => {
        const mockFriend = { id: '456', username: 'friend1' };
        global.__TAURI__.invoke.mockResolvedValue(mockFriend);

        document.getElementById('friendPublicKey').value = 'friend-key';

        await addFriendByPublicKey();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('add_friend_from_qr_code', {
            currentUserId: '123',
            qrData: {
                username: 'User_friend-k',
                publicKey: 'friend-key'
            }
        });

        const success = document.getElementById('dashboardError');
        expect(success.textContent).toBe('Successfully added friend1 as a friend!');
        expect(document.getElementById('friendPublicKey').value).toBe('');
    });

    test('selectFriend should set selectedRecipient', () => {
        selectFriend('456', 'frienduser');

        expect(global.selectedRecipient).toEqual({
            id: '456',
            username: 'frienduser'
        });

        const selectedContainer = document.getElementById('selectedRecipient');
        expect(selectedContainer.innerHTML).toContain('frienduser');
        expect(selectedContainer.innerHTML).toContain('F'); // Avatar initial
    });
});

describe('Message Functions', () => {
    beforeEach(() => {
        global.currentUser = { id: '123', username: 'testuser' };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('sendMessage should validate recipient and content', async () => {
        document.getElementById('messageContent').value = '';
        global.selectedRecipient = null;

        await sendMessage();

        const error = document.getElementById('dashboardError');
        expect(error.textContent).toBe('Please select a recipient and enter a message');
        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('sendMessage should send encrypted message', async () => {
        global.selectedRecipient = { id: '456', username: 'friend' };
        document.getElementById('messageContent').value = 'Hello friend';
        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        await sendMessage();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('send_encrypted_message', {
            senderId: '123',
            recipientId: '456',
            content: 'Hello friend'
        });

        expect(document.getElementById('messageContent').value).toBe('');
        expect(global.selectedRecipient).toBeNull();
    });

    test('sendMessage should send reply when reply context exists', async () => {
        global.selectedRecipient = { id: '456', username: 'friend' };
        const messageInput = document.getElementById('messageContent');
        messageInput.value = 'Reply message';
        messageInput.setAttribute('data-reply-to', '789');

        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        await sendMessage();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('reply_to_message', {
            originalMessageId: 789,
            senderId: '123',
            recipientId: '456',
            content: 'Reply message'
        });

        expect(messageInput.hasAttribute('data-reply-to')).toBe(false);
    });

    test('addReaction should add emoji reaction to message', async () => {
        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        await addReaction(789, '❤️');

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('add_message_reaction', {
            messageId: 789,
            userId: '123',
            emoji: '❤️'
        });
    });

    test('deleteMessage should confirm before deletion', async () => {
        global.confirm = jest.fn().mockReturnValue(false);

        await deleteMessage(789);

        expect(global.confirm).toHaveBeenCalledWith(
            'Are you sure you want to delete this message? This action cannot be undone.'
        );
        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('deleteMessage should delete when confirmed', async () => {
        global.confirm = jest.fn().mockReturnValue(true);
        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        // Create a mock message element
        const messageEl = document.createElement('div');
        messageEl.setAttribute('data-message-id', '789');
        document.body.appendChild(messageEl);

        await deleteMessage(789);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('delete_message', {
            messageId: 789,
            userId: '123'
        });

        expect(document.querySelector('[data-message-id="789"]')).toBeNull();
    });
});

describe('Voice Message Functions', () => {
    let mockMediaRecorder;
    let mockStream;

    beforeEach(() => {
        global.currentUser = { id: '123' };
        global.selectedRecipient = { id: '456', username: 'friend' };
        global.__TAURI__ = { invoke: jest.fn() };

        // Mock MediaRecorder
        mockStream = {
            getTracks: jest.fn().mockReturnValue([
                { stop: jest.fn() }
            ])
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

    test('startVoiceRecording should initialize media recorder', async () => {
        document.body.innerHTML += `
            <button id="voiceRecordButton">🎤 Record Voice Message</button>
        `;

        await startVoiceRecording();

        expect(navigator.mediaDevices.getUserMedia).toHaveBeenCalledWith({ audio: true });
        expect(mockMediaRecorder.start).toHaveBeenCalled();
        expect(global.isRecording).toBe(true);

        const button = document.getElementById('voiceRecordButton');
        expect(button.textContent).toBe('🛑 Stop Recording');
        expect(button.classList.contains('recording')).toBe(true);
    });

    test('stopVoiceRecording should stop media recorder', () => {
        document.body.innerHTML += `
            <button id="voiceRecordButton" class="recording">🛑 Stop Recording</button>
        `;

        global.mediaRecorder = mockMediaRecorder;
        global.isRecording = true;

        stopVoiceRecording();

        expect(mockMediaRecorder.stop).toHaveBeenCalled();
        expect(global.isRecording).toBe(false);

        const button = document.getElementById('voiceRecordButton');
        expect(button.textContent).toBe('🎤 Record Voice Message');
        expect(button.classList.contains('recording')).toBe(false);
    });

    test('playVoiceMessage should create and play audio', () => {
        const mockPlay = jest.fn();
        global.Audio = jest.fn().mockImplementation(() => ({
            play: mockPlay
        }));

        playVoiceMessage('base64audiodata');

        expect(global.Audio).toHaveBeenCalledWith('data:audio/wav;base64,base64audiodata');
        expect(mockPlay).toHaveBeenCalled();
    });

    test('deleteVoiceMessage should delete voice message', async () => {
        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        await deleteVoiceMessage(999);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('delete_voice_message', {
            voiceMessageId: 999,
            userId: '123'
        });
    });
});

describe('QR Code Functions', () => {
    beforeEach(() => {
        global.currentUser = {
            id: '123',
            username: 'testuser',
            publicKey: 'test-public-key'
        };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('generateQRCode should create QR code image', async () => {
        const mockQRDataUrl = 'data:image/png;base64,qrcode';
        global.__TAURI__.invoke.mockResolvedValue(mockQRDataUrl);

        await generateQRCode('myQrCode');

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('generate_qr_code', {
            data: 'cipher://add-friend?username=testuser&public_key=test-public-key'
        });

        const container = document.getElementById('myQrCode');
        expect(container.innerHTML).toContain('<img src="data:image/png;base64,qrcode"');
    });

    test('scanQRCode should handle desktop file upload', async () => {
        global.__TAURI__.invoke
            .mockResolvedValueOnce('desktop') // get_platform
            .mockResolvedValueOnce({ // scan_qr_code_from_image
                username: 'friend',
                publicKey: 'friend-key'
            });

        const fileInput = document.getElementById('qrCodeFile');
        const clickSpy = jest.spyOn(fileInput, 'click');

        await scanQRCode();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('get_platform');
        expect(clickSpy).toHaveBeenCalled();
    });

    test('handleQRCodeFile should process QR code image', async () => {
        const mockQRData = {
            username: 'friend',
            publicKey: 'friend-key'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockQRData);

        const file = new File(['image'], 'qr.png', { type: 'image/png' });
        const event = { target: { files: [file], value: 'qr.png' } };

        await handleQRCodeFile(event);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('scan_qr_code_from_image', {
            base64Image: expect.stringContaining('data:image/png;base64,')
        });

        expect(document.getElementById('friendPublicKey').value).toBe('friend-key');
        expect(event.target.value).toBe('');
    });
});

describe('Profile Functions', () => {
    beforeEach(() => {
        global.currentUser = {
            id: '123',
            username: 'testuser',
            bio: 'Test bio'
        };
        global.__TAURI__ = { invoke: jest.fn() };
    });

    test('ProfileManager.save should update user profile', async () => {
        document.body.innerHTML += `
            <textarea id="profileBio">Updated bio</textarea>
        `;

        const updatedUser = {
            ...global.currentUser,
            bio: 'Updated bio'
        };

        global.__TAURI__.invoke.mockResolvedValue(updatedUser);

        await ProfileManager.save();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('update_user_profile', {
            userId: '123',
            bio: 'Updated bio',
            profilePicture: null
        });

        expect(global.currentUser.bio).toBe('Updated bio');
    });

    test('ProfileManager.uploadPicture should upload profile picture', async () => {
        const file = new File(['image'], 'profile.jpg', { type: 'image/jpeg' });
        const updatedUser = {
            ...global.currentUser,
            profilePicture: 'data:image/jpeg;base64,updated'
        };

        global.__TAURI__.invoke.mockResolvedValue(updatedUser);

        await ProfileManager.uploadPicture(file);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('upload_profile_picture', {
            userId: '123',
            fileData: expect.any(String),
            filename: 'profile.jpg',
            fileType: 'image/jpeg'
        });

        expect(global.currentUser.profilePicture).toBe('data:image/jpeg;base64,updated');
    });
});

describe('Post CRUD Operations', () => {
    beforeEach(() => {
        global.currentUser = { id: '123', username: 'testuser' };
        global.__TAURI__ = { invoke: jest.fn() };
        global.confirm = jest.fn();
    });

    test('editPost should show edit form for post', async () => {
        document.body.innerHTML = `
            <div data-post-id="456">
                <div class="post-content">Original content</div>
                <div class="post-actions"></div>
            </div>
        `;

        await editPost(456);

        const postElement = document.querySelector('[data-post-id="456"]');
        expect(postElement.querySelector('.post-edit-form')).toBeTruthy();
        expect(postElement.querySelector('.post-content').style.display).toBe('none');
        expect(postElement.classList.contains('post-edit-mode')).toBe(true);

        const textarea = document.getElementById('edit-post-textarea-456');
        expect(textarea.value).toBe('Original content');
    });

    test('saveEditPost should update post content', async () => {
        document.body.innerHTML = `
            <div data-post-id="456">
                <textarea id="edit-post-textarea-456">Updated content</textarea>
            </div>
        `;

        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        await saveEditPost(456);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('edit_post', {
            postId: 456,
            userId: '123',
            newContent: 'Updated content'
        });
    });

    test('deletePost should confirm before deletion', async () => {
        global.confirm.mockReturnValue(false);

        await deletePost(456);

        expect(global.confirm).toHaveBeenCalledWith(
            'Are you sure you want to delete this post? This action cannot be undone.'
        );
        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('deletePost should delete when confirmed', async () => {
        global.confirm.mockReturnValue(true);
        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        document.body.innerHTML = `
            <div data-post-id="456" class="post"></div>
        `;

        await deletePost(456);

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('delete_post', {
            postId: 456,
            userId: '123'
        });

        expect(document.querySelector('[data-post-id="456"]')).toBeNull();
    });
});

describe('Search Functions', () => {
    beforeEach(() => {
        global.currentUser = { id: '123' };
        global.__TAURI__ = { invoke: jest.fn() };

        document.body.innerHTML += `
            <input id="messageSearchInput" />
            <div id="searchResults" class="hidden"></div>
        `;
    });

    test('searchMessages should validate query input', async () => {
        document.getElementById('messageSearchInput').value = '';

        await searchMessages();

        const results = document.getElementById('searchResults');
        expect(results.textContent).toBe('Please enter a search query');
        expect(global.__TAURI__.invoke).not.toHaveBeenCalled();
    });

    test('searchMessages should perform search with query', async () => {
        const mockResults = [
            {
                id: 1,
                content: 'Test message',
                createdAt: new Date().toISOString(),
                encrypted: false
            }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockResults);
        document.getElementById('messageSearchInput').value = 'test';

        await searchMessages();

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('search_messages', {
            userId: '123',
            query: 'test'
        });

        const results = document.getElementById('searchResults');
        expect(results.classList.contains('hidden')).toBe(false);
        expect(results.innerHTML).toContain('Test message');
    });

    test('displaySearchResults should show no results message', () => {
        displaySearchResults([]);

        const results = document.getElementById('searchResults');
        expect(results.innerHTML).toContain('No messages found matching your search');
        expect(results.classList.contains('hidden')).toBe(false);
    });

    test('clearMessageSearch should clear search input and hide results', () => {
        document.getElementById('messageSearchInput').value = 'test query';
        document.getElementById('searchResults').classList.remove('hidden');

        clearMessageSearch();

        expect(document.getElementById('messageSearchInput').value).toBe('');
        expect(document.getElementById('searchResults').classList.contains('hidden')).toBe(true);
    });
});

describe('Friend Invites', () => {
    beforeEach(() => {
        global.currentUser = { id: '123' };
        global.tauriInvoke = jest.fn();
        global.prompt = jest.fn();
        global.navigator.clipboard = { writeText: jest.fn().mockResolvedValue() };
    });

    test('FriendManager.createInvite should create and copy invite code', async () => {
        const mockInvite = {
            inviteCode: 'ABC123',
            expiresAt: new Date().toISOString(),
            usesRemaining: 5
        };

        global.tauriInvoke.mockResolvedValue(mockInvite);

        const invite = await FriendManager.createInvite(5, 24);

        expect(global.tauriInvoke).toHaveBeenCalledWith('create_friend_invite', {
            userId: '123',
            uses: 5,
            hoursValid: 24
        });

        expect(navigator.clipboard.writeText).toHaveBeenCalledWith('ABC123');
        expect(invite).toEqual(mockInvite);
    });

    test('FriendManager.useInvite should use invite code', async () => {
        const mockFriend = {
            id: '456',
            username: 'newfriend'
        };

        global.tauriInvoke.mockResolvedValue(mockFriend);

        const friend = await FriendManager.useInvite('abc123');

        expect(global.tauriInvoke).toHaveBeenCalledWith('use_friend_invite', {
            userId: '123',
            inviteCode: 'ABC123'
        });

        expect(friend).toEqual(mockFriend);
    });
});

describe('DOM Content Loaded', () => {
    test('should initialize app on DOMContentLoaded', async () => {
        const initSpy = jest.spyOn(Navbar, 'init');
        const waitForAPISpy = jest.spyOn(TauriAPI, 'waitForAPI').mockResolvedValue();
        const autoLoginSpy = jest.spyOn(Session, 'attemptAutoLogin').mockResolvedValue(false);

        // Trigger DOMContentLoaded event
        const event = new Event('DOMContentLoaded');
        document.dispatchEvent(event);

        // Wait for async operations
        await new Promise(resolve => setTimeout(resolve, 0));

        expect(initSpy).toHaveBeenCalledWith('navbarContainer');
        expect(waitForAPISpy).toHaveBeenCalled();
        expect(autoLoginSpy).toHaveBeenCalled();
    });
});

describe('Window Export', () => {
    test('should export functions to window object', () => {
        expect(typeof window.handleLogin).toBe('function');
        expect(typeof window.handleLogout).toBe('function');
        expect(typeof window.showLogin).toBe('function');
        expect(typeof window.showFeed).toBe('function');
        expect(typeof window.sendMessage).toBe('function');
        expect(typeof window.addFriendByPublicKey).toBe('function');
        expect(typeof window.toggleHamburgerMenu).toBe('function');
    });
});