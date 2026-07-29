/**
 * XSS regression tests for the real renderers in src/js/main.js.
 *
 * Threat model: in this P2P app, display names, post/comment content, reaction
 * emoji and media fileType all arrive from peers. The WebView holds an
 * unrestricted window.__TAURI__, so DOM injection is equivalent to arbitrary
 * local-database access with no user interaction required.
 *
 * These tests feed hostile values through the actual functions (never a copy)
 * and assert that no element or event-handler attribute is created.
 */

const { loadApp } = require('./helpers/load-app');

// Payloads that broke the previous implementation:
//  - the <img onerror> is the classic no-interaction execution primitive
//  - the quote payloads defeat the old textContent->innerHTML "escape", which
//    left " and ' untouched and so escaped nothing in an attribute context
const HOSTILE_NAME = '<img src=x onerror=alert(1)>';
const HOSTILE_ATTR_NAME = '" onmouseover="alert(1)" x="';
const HOSTILE_JS_NAME = "'); window.__pwned = 1; //";
const HOSTILE_EMOJI = '<img src=x onerror=window.__pwned=1>';

let app;

function assertNoInjection(root) {
    expect(root.querySelector('img[onerror]')).toBeNull();
    expect(root.querySelector('script')).toBeNull();

    // No element anywhere may have gained an inline event handler, and no
    // element may exist that the renderer did not intend to create.
    root.querySelectorAll('*').forEach(el => {
        for (const attr of Array.from(el.attributes)) {
            expect(attr.name.startsWith('on')).toBe(false);
        }
    });

    expect(window.__pwned).toBeUndefined();
}

function mountFixture(html) {
    document.body.innerHTML = html;
}

beforeEach(() => {
    document.body.innerHTML = '';
    delete window.__pwned;
    app = loadApp();
});

describe('Utils.escapeHtml', () => {
    test('escapes the full set of HTML-significant characters', () => {
        expect(app.Utils.escapeHtml('&')).toBe('&amp;');
        expect(app.Utils.escapeHtml('<')).toBe('&lt;');
        expect(app.Utils.escapeHtml('>')).toBe('&gt;');
        expect(app.Utils.escapeHtml('"')).toBe('&quot;');
        expect(app.Utils.escapeHtml("'")).toBe('&#39;');
        expect(app.Utils.escapeHtml('/')).toBe('&#47;');
    });

    test('escapes quotes so attribute contexts are safe (regression)', () => {
        // The old textContent->innerHTML implementation returned this string
        // unchanged, so every `attr="${escapeHtml(x)}"` in the app was injectable.
        const escaped = app.Utils.escapeHtml(HOSTILE_ATTR_NAME);
        expect(escaped).not.toContain('"');
        expect(escaped).not.toContain("'");

        mountFixture(`<div id="t" title="${escaped}"></div>`);
        const el = document.getElementById('t');
        expect(el.getAttribute('onmouseover')).toBeNull();
        expect(el.title).toBe(HOSTILE_ATTR_NAME);
    });

    test('round-trips through the DOM without changing the visible text', () => {
        const name = `Alice <b>"Bob"</b> & 'Eve' /x`;
        mountFixture(`<div id="t">${app.Utils.escapeHtml(name)}</div>`);
        expect(document.getElementById('t').textContent).toBe(name);
        expect(document.getElementById('t').querySelector('b')).toBeNull();
    });

    test('handles null and undefined without throwing', () => {
        expect(app.Utils.escapeHtml(null)).toBe('');
        expect(app.Utils.escapeHtml(undefined)).toBe('');
    });
});

describe('Utils.safeMimeType', () => {
    test('rejects a fileType that would break out of a data: URL', () => {
        expect(app.Utils.safeMimeType('image/png"><img src=x onerror=alert(1)>'))
            .toBe('application/octet-stream');
        expect(app.Utils.safeMimeType('image/png')).toBe('image/png');
        expect(app.Utils.safeMimeType(null)).toBe('application/octet-stream');
    });
});

describe('feed renderer (loadPosts)', () => {
    async function renderFeed(post) {
        mountFixture('<div id="posts"></div><div id="postsStatusMessage"></div><div id="postsContent"></div>');
        app.__setCurrentUser({ id: 'me', displayName: 'Me' });

        window.__TAURI__.core.invoke = jest.fn((cmd) => {
            switch (cmd) {
                case 'get_all_posts': return Promise.resolve([post]);
                case 'get_media_attachments': return Promise.resolve(post.mediaAttachments || []);
                case 'get_post_reaction_summary': return Promise.resolve(post.reactionSummary || []);
                case 'get_post_comment_count': return Promise.resolve(0);
                case 'get_user_post_reaction': return Promise.resolve(null);
                default: return Promise.resolve(null);
            }
        });

        await app.loadPosts();
        return document.getElementById('posts');
    }

    test('a hostile peer display name is inert', async () => {
        const container = await renderFeed({
            id: 'p1', userId: 'peer', displayName: HOSTILE_NAME,
            content: 'hi', createdAt: new Date().toISOString(),
        });

        assertNoInjection(container);
        expect(container.querySelector('.post-meta').textContent).toContain(HOSTILE_NAME);
    });

    test('hostile post content is inert', async () => {
        const container = await renderFeed({
            id: 'p1', userId: 'peer', displayName: 'peer',
            content: '<script>window.__pwned=1</script><img src=x onerror=alert(1)>',
            createdAt: new Date().toISOString(),
        });

        assertNoInjection(container);
    });

    test('a hostile post id cannot inject an attribute or a handler', async () => {
        const container = await renderFeed({
            id: '"><img src=x onerror=alert(1)><span id="',
            userId: 'me', displayName: 'peer', content: 'hi',
            createdAt: new Date().toISOString(),
        });

        assertNoInjection(container);
        // The id survives intact as data, which is what the delegated handler reads.
        expect(container.querySelector('.post').dataset.postId)
            .toBe('"><img src=x onerror=alert(1)><span id="');
    });
});

describe('chat renderer (loadMessages)', () => {
    test('a hostile sender display name is inert in the bubble', async () => {
        mountFixture('<div id="messages"></div><div id="recentContacts"></div><div id="messagesContent"></div>');
        app.__setCurrentUser({ id: 'me', displayName: 'Me' });

        const friend = { id: 'peer', displayName: HOSTILE_NAME, publicKey: 'AAAABBBBCCCC' };
        app.__setAllFriends([friend]);

        window.__TAURI__.core.invoke = jest.fn((cmd) => {
            switch (cmd) {
                case 'get_friends': return Promise.resolve([friend]);
                case 'get_messages_for_user': return Promise.resolve([{
                    id: 'm1', senderId: 'peer', recipientId: 'me',
                    content: 'hello', encrypted: false, createdAt: new Date().toISOString(),
                }]);
                case 'get_message_reactions': return Promise.resolve([]);
                default: return Promise.resolve(null);
            }
        });

        await app.loadMessages();
        const container = document.getElementById('messages');

        assertNoInjection(container);
        expect(container.querySelector('.bubble-sender').textContent).toContain(HOSTILE_NAME);
    });

    test('a hostile message id does not become executable JS', async () => {
        mountFixture('<div id="messages"></div><div id="recentContacts"></div><div id="messagesContent"></div>');
        app.__setCurrentUser({ id: 'me', displayName: 'Me' });
        app.__setAllFriends([]);

        window.__TAURI__.core.invoke = jest.fn((cmd) => {
            switch (cmd) {
                case 'get_friends': return Promise.resolve([]);
                case 'get_messages_for_user': return Promise.resolve([{
                    id: HOSTILE_JS_NAME, senderId: 'me', recipientId: 'peer',
                    content: 'hello', encrypted: false, createdAt: new Date().toISOString(),
                }]);
                case 'get_message_reactions': return Promise.resolve([]);
                default: return Promise.resolve(null);
            }
        });

        await app.loadMessages();
        assertNoInjection(document.getElementById('messages'));
    });
});

describe('reaction renderers', () => {
    test('renderReactionSummary neutralises a hostile emoji', () => {
        const html = app.PostInteractions.renderReactionSummary([[HOSTILE_EMOJI, 3]], null);
        mountFixture(`<div id="t">${html}</div>`);
        const container = document.getElementById('t');

        assertNoInjection(container);
        expect(container.querySelector('.reaction-emoji').textContent).toBe(HOSTILE_EMOJI);
        // The value is still recoverable for the delegated click handler.
        expect(container.querySelector('.reaction-chip').dataset.emoji).toBe(HOSTILE_EMOJI);
    });

    test('renderReactionSummary survives an attribute-breaking emoji', () => {
        const html = app.PostInteractions.renderReactionSummary([[HOSTILE_ATTR_NAME, 1]], null);
        mountFixture(`<div id="t">${html}</div>`);

        assertNoInjection(document.getElementById('t'));
        expect(document.querySelector('.reaction-chip').dataset.emoji).toBe(HOSTILE_ATTR_NAME);
    });

    test('renderMessageReactions escapes the emoji and the title attribute', () => {
        app.__setCurrentUser({ id: 'me' });
        const html = app.renderMessageReactions([
            { emoji: HOSTILE_EMOJI, userId: HOSTILE_ATTR_NAME },
        ]);
        mountFixture(`<div id="t">${html}</div>`);

        assertNoInjection(document.getElementById('t'));
    });
});

describe('comment renderer', () => {
    test('hostile comment content and author are inert', () => {
        app.__setCurrentUser({ id: 'me' });
        app.__setAllFriends([{ id: 'peer', displayName: HOSTILE_NAME, publicKey: 'AAAABBBBCCCC' }]);

        const html = app.PostInteractions.renderComment({
            id: '"><img src=x onerror=alert(1)>',
            userId: 'peer',
            content: '<img src=x onerror=window.__pwned=1>',
            createdAt: new Date().toISOString(),
            depth: 0,
        }, 'post-1');

        mountFixture(`<div id="t">${html}</div>`);
        assertNoInjection(document.getElementById('t'));
    });
});

describe('media preview renderer', () => {
    test('a peer-controlled fileType cannot break out of the src attribute', () => {
        const html = app.PostManager.createMediaPreview({
            id: 'm1',
            fileType: 'image/png"><img src=x onerror=window.__pwned=1><i class="',
            data: 'AAAA',
        });

        mountFixture(`<div id="t">${html}</div>`);
        const container = document.getElementById('t');

        assertNoInjection(container);
        expect(container.querySelectorAll('img').length).toBe(1);
        expect(container.querySelector('img').getAttribute('src'))
            .toBe('data:image/png;base64,AAAA');
    });

    test('the image opens the viewer through a data attribute, not inline JS', () => {
        const html = app.PostManager.createMediaPreview({
            id: 'm1', fileType: 'image/png', data: 'AAAA',
        });
        mountFixture(`<div id="t">${html}</div>`);

        const img = document.querySelector('img');
        expect(img.getAttribute('onclick')).toBeNull();
        expect(img.dataset.imageViewer).toBe('1');
    });
});

describe('friend picker and recipient chips', () => {
    test('a hostile friend name cannot escape the click handler', () => {
        mountFixture('<div id="friendsList"></div><input id="friendSearch">');
        app.__setAllFriends([{ id: HOSTILE_JS_NAME, displayName: HOSTILE_ATTR_NAME }]);
        app.__setSelectedRecipients([]);

        app.renderFriendsList('');
        const container = document.getElementById('friendsList');

        assertNoInjection(container);
        const item = container.querySelector('.friend-select-item');
        expect(item.dataset.friendSelect).toBe(HOSTILE_JS_NAME);
        expect(item.dataset.friendName).toBe(HOSTILE_ATTR_NAME);
    });

    test('recipient chips escape a hostile display name', () => {
        mountFixture('<div id="selectedRecipient"></div>');
        app.__setSelectedRecipients([{ id: HOSTILE_JS_NAME, displayName: HOSTILE_NAME }]);

        app.updateSelectedRecipientsUI();
        assertNoInjection(document.getElementById('selectedRecipient'));
    });
});
