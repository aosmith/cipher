/**
 * Loads the real src/js/main.js into the jsdom environment and returns its
 * bindings.
 *
 * The app has no bundler: index.html loads plain <script> tags and main.js
 * declares everything with top-level `const`/`function`. Wrapping the source in
 * a Function and appending an explicit `return` gives the tests the *actual*
 * objects from main.js, so a test can never silently end up exercising a copy.
 *
 * Load errors are thrown, not swallowed. The previous test-runner.js caught and
 * logged them, which turned "main.js failed to parse" into "every export is
 * undefined" and produced 130+ meaningless failures.
 */

const fs = require('fs');
const path = require('path');

const MAIN_JS = path.join(__dirname, '..', '..', 'main.js');

// Names main.js declares at top level that the tests need. Anything not listed
// here is still loaded (and still attached to window by main.js's own
// Object.assign at the bottom of the file), it is just not returned directly.
const EXPORTS = [
    'Utils',
    'UI',
    'TauriAPI',
    'Session',
    'PostManager',
    'PostInteractions',
    'ProfileManager',
    'SafetyManager',
    'DeviceManager',
    'RecentContacts',
    'getDisplayName',
    'formatNameWithFingerprint',
    'renderMessageReactions',
    'loadPosts',
    'loadMessages',
    'loadFriends',
    'renderFriendsList',
    'updateSelectedRecipientsUI',
];

function loadApp() {
    const source = fs.readFileSync(MAIN_JS, 'utf8');

    // Dependencies main.js expects other <script> tags to have defined.
    window.Navbar = {
        init: jest.fn(),
        updateLoginState: jest.fn(),
        setActiveLink: jest.fn(),
        copyInviteLink: jest.fn(),
        loadNotifications: jest.fn(),
    };
    window.P2P = {
        initialized: false,
        initialize: jest.fn(() => Promise.resolve()),
        shutdown: jest.fn(() => Promise.resolve()),
        announcePresence: jest.fn(() => Promise.resolve()),
        generateInvite: jest.fn(() => Promise.resolve('cipher://add-friend?key=test')),
        publishReaction: jest.fn(() => Promise.resolve()),
        publishComment: jest.fn(() => Promise.resolve()),
        queuePublish: jest.fn(),
        healthCheckAndRecover: jest.fn(() => Promise.resolve(true)),
    };

    const factory = new Function(
        `${source}\n;return { ${EXPORTS.join(', ')},
            __setCurrentUser(u) { currentUser = u; },
            __setAllFriends(f) { allFriends = f; },
            __setSelectedRecipients(r) { selectedRecipients = r; }
        };`
    );

    // main.js's top-level code runs here against the real jsdom document.
    return factory.call(window);
}

module.exports = { loadApp };
