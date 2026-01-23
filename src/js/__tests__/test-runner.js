/**
 * Test Runner - Loads main.js functions for testing
 */

// Load main.js content and make functions available globally
const fs = require('fs');
const path = require('path');

// Read main.js and execute in global context
const mainJsPath = path.join(__dirname, '..', 'main.js');
const mainJsContent = fs.readFileSync(mainJsPath, 'utf8');

// Create a function wrapper to execute main.js
const executeMainJs = new Function(mainJsContent);

// Mock window and document before executing
global.window = global;
global.document = {
    addEventListener: jest.fn(),
    getElementById: jest.fn(() => ({
        classList: {
            add: jest.fn(),
            remove: jest.fn(),
            contains: jest.fn(),
            toggle: jest.fn()
        },
        style: {},
        innerHTML: '',
        textContent: '',
        value: ''
    })),
    querySelector: jest.fn(),
    querySelectorAll: jest.fn(() => []),
    createElement: jest.fn((tag) => ({
        tagName: tag.toUpperCase(),
        classList: {
            add: jest.fn(),
            remove: jest.fn()
        },
        style: {},
        innerHTML: '',
        textContent: '',
        addEventListener: jest.fn(),
        appendChild: jest.fn(),
        removeChild: jest.fn()
    })),
    body: {
        innerHTML: '',
        appendChild: jest.fn(),
        removeChild: jest.fn(),
        classList: {
            add: jest.fn(),
            remove: jest.fn()
        }
    },
    head: {
        innerHTML: ''
    }
};

global.navigator = {
    mediaDevices: {
        getUserMedia: jest.fn()
    },
    clipboard: {
        writeText: jest.fn()
    }
};

global.localStorage = {
    getItem: jest.fn(),
    setItem: jest.fn(),
    removeItem: jest.fn(),
    clear: jest.fn()
};

global.sessionStorage = {
    getItem: jest.fn(),
    setItem: jest.fn(),
    removeItem: jest.fn(),
    clear: jest.fn()
};

global.console = {
    log: jest.fn(),
    error: jest.fn(),
    warn: jest.fn(),
    info: jest.fn(),
    debug: jest.fn()
};

global.setTimeout = jest.fn((cb, delay) => {
    if (typeof cb === 'function') cb();
    return 1;
});

global.setInterval = jest.fn();
global.clearTimeout = jest.fn();
global.clearInterval = jest.fn();

global.alert = jest.fn();
global.confirm = jest.fn();
global.prompt = jest.fn();

global.FileReader = jest.fn().mockImplementation(() => ({
    readAsDataURL: jest.fn(function() {
        setTimeout(() => {
            this.result = 'data:text/plain;base64,aGVsbG8gd29ybGQ=';
            if (this.onload) this.onload();
        }, 0);
    }),
    readAsText: jest.fn(),
    result: null,
    onload: null,
    onerror: null
}));

global.File = class File {
    constructor(bits, name, options = {}) {
        this.bits = bits;
        this.name = name;
        this.type = options.type || '';
        this.size = bits.reduce((acc, bit) => acc + bit.length, 0);
    }
};

global.Blob = class Blob {
    constructor(bits, options = {}) {
        this.bits = bits;
        this.type = options.type || '';
        this.size = bits.reduce((acc, bit) => acc + (bit.length || 0), 0);
    }
};

global.Audio = jest.fn().mockImplementation(() => ({
    play: jest.fn(),
    pause: jest.fn()
}));

global.MediaRecorder = jest.fn();

global.URL = {
    createObjectURL: jest.fn(() => 'blob:mock-url'),
    revokeObjectURL: jest.fn()
};

// Execute main.js to load functions
try {
    executeMainJs();
} catch (error) {
    // Ignore errors from missing dependencies during load
    console.log('Note: Some errors during main.js load are expected in test environment');
}

module.exports = {
    Utils: global.Utils,
    TauriAPI: global.TauriAPI,
    Session: global.Session,
    UI: global.UI,
    PostManager: global.PostManager,
    ProfileManager: global.ProfileManager,
    FriendManager: global.FriendManager,
    // Export all the global functions
    handleLogin: global.handleLogin,
    handleLogout: global.handleLogout,
    showLogin: global.showLogin,
    showDashboard: global.showDashboard,
    showFeed: global.showFeed,
    showPosts: global.showPosts,
    showMessages: global.showMessages,
    showFriends: global.showFriends,
    showCreatePostPage: global.showCreatePostPage,
    showEditProfile: global.showEditProfile,
    loadPosts: global.loadPosts,
    loadMessages: global.loadMessages,
    loadFriends: global.loadFriends,
    sendMessage: global.sendMessage,
    addFriendFromTab: global.addFriendFromTab,
    selectFriend: global.selectFriend,
    generateQRCode: global.generateQRCode,
    generateMyQRCode: global.generateMyQRCode,
    generateProfileQRCode: global.generateProfileQRCode,
    scanQRCode: global.scanQRCode,
    handleQRCodeFile: global.handleQRCodeFile,
    addFriendByQRCode: global.addFriendByQRCode,
    handleProfilePictureUpload: global.handleProfilePictureUpload,
    saveProfile: global.saveProfile,
    createFriendInvite: global.createFriendInvite,
    useFriendInvite: global.useFriendInvite,
    exportFriendsList: global.exportFriendsList,
    importFriendsList: global.importFriendsList,
    editMessage: global.editMessage,
    cancelEditMessage: global.cancelEditMessage,
    saveEditMessage: global.saveEditMessage,
    deleteMessage: global.deleteMessage,
    editPost: global.editPost,
    cancelEditPost: global.cancelEditPost,
    saveEditPost: global.saveEditPost,
    deletePost: global.deletePost,
    toggleHamburgerMenu: global.toggleHamburgerMenu,
    closeHamburgerMenu: global.closeHamburgerMenu,
    addReaction: global.addReaction,
    replyToMessage: global.replyToMessage,
    viewThread: global.viewThread,
    setupFriendSearch: global.setupFriendSearch,
    clearSelectedRecipient: global.clearSelectedRecipient,
    createPost: global.createPost,
    createPostFromPage: global.createPostFromPage,
    cancelCreatePost: global.cancelCreatePost,
    showCreatePost: global.showCreatePost,
    showAddFriend: global.showAddFriend,
    viewMediaAttachment: global.viewMediaAttachment,
    renderMessageReactions: global.renderMessageReactions
};