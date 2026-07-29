/**
 * Functional tests for the delegated event handlers that replaced the inline
 * onclick="fn('${peerControlledValue}')" attributes.
 *
 * Removing inline handlers is only a fix if the buttons still work, and the
 * whole point of the change is that hostile values reach the handler intact as
 * *data*. main.js is loaded once here because its top-level code installs the
 * document-level listeners.
 */

const { loadApp } = require('./helpers/load-app');

const HOSTILE_ID = `'); window.__pwned = 1; //`;
const HOSTILE_EMOJI = '<img src=x onerror=alert(1)>';

let app;

beforeAll(() => {
    app = loadApp();
});

beforeEach(() => {
    document.body.innerHTML = '';
    delete window.__pwned;
});

function click(el) {
    el.dispatchEvent(new window.MouseEvent('click', { bubbles: true, cancelable: true }));
}

test('reaction chip forwards the raw emoji to toggleReaction', () => {
    const spy = jest.spyOn(app.PostInteractions, 'toggleReaction').mockImplementation(() => {});
    document.body.innerHTML = app.PostInteractions.renderReactionSummary([[HOSTILE_EMOJI, 1]], null);

    click(document.querySelector('.reaction-chip'));

    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][1]).toBe(HOSTILE_EMOJI);
    expect(window.__pwned).toBeUndefined();
});

test('post menu button forwards a hostile post id to showPostMenu', () => {
    const spy = jest.spyOn(app.PostInteractions, 'showPostMenu').mockImplementation(() => {});
    document.body.innerHTML = `<button data-post-menu="${app.Utils.escapeHtml(HOSTILE_ID)}">x</button>`;

    click(document.querySelector('button'));

    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][1]).toBe(HOSTILE_ID);
    expect(window.__pwned).toBeUndefined();
});

test('comment submit button forwards the post id', () => {
    const spy = jest.spyOn(app.PostInteractions, 'submitComment').mockImplementation(() => {});
    document.body.innerHTML = `<button data-comment-submit="${app.Utils.escapeHtml(HOSTILE_ID)}">Post</button>`;

    click(document.querySelector('button'));

    expect(spy).toHaveBeenCalledWith(HOSTILE_ID);
});

test('Enter in a comment input submits the comment', () => {
    const spy = jest.spyOn(app.PostInteractions, 'submitComment').mockImplementation(() => {});
    document.body.innerHTML = `<input data-comment-input="${app.Utils.escapeHtml(HOSTILE_ID)}">`;

    document.querySelector('input').dispatchEvent(
        new window.KeyboardEvent('keypress', { key: 'Enter', bubbles: true, cancelable: true })
    );

    expect(spy).toHaveBeenCalledWith(HOSTILE_ID);
});

test('friend picker item forwards both id and display name', () => {
    document.body.innerHTML = '<div id="friendsList"></div><input id="friendSearch"><div id="selectedRecipient"></div>';
    app.__setAllFriends([{ id: HOSTILE_ID, displayName: `"><b>evil</b>` }]);
    app.__setSelectedRecipients([]);
    app.renderFriendsList('');

    click(document.querySelector('.friend-select-item'));

    // toggleFriendSelection pushes onto selectedRecipients, then re-renders.
    const chip = document.querySelector('.friend-select-item');
    expect(chip.classList.contains('selected')).toBe(true);
    expect(chip.dataset.friendSelect).toBe(HOSTILE_ID);
    expect(document.querySelector('b')).toBeNull();
});

test('unblock / unmute / device buttons reach their managers', () => {
    const unblock = jest.spyOn(app.SafetyManager, 'unblockUser').mockImplementation(() => {});
    const unmute = jest.spyOn(app.SafetyManager, 'unmuteUser').mockImplementation(() => {});
    const rename = jest.spyOn(app.DeviceManager, 'renameDevice').mockImplementation(() => {});

    document.body.innerHTML = `
        <button data-unblock-user="a">u</button>
        <button data-unmute-user="b">u</button>
        <button data-device-rename="c">r</button>`;

    document.querySelectorAll('button').forEach(click);

    expect(unblock).toHaveBeenCalledWith('a');
    expect(unmute).toHaveBeenCalledWith('b');
    expect(rename).toHaveBeenCalledWith('c');
});

test('image preview opens the viewer with the sanitized data URL', () => {
    const spy = jest.spyOn(app.UI, 'showImageViewer').mockImplementation(() => {});
    document.body.innerHTML = app.PostManager.createMediaPreview({
        id: 'm1', fileType: 'image/png', data: 'AAAA',
    });

    click(document.querySelector('img'));

    expect(spy).toHaveBeenCalledWith('data:image/png;base64,AAAA');
});

test('showImageViewer refuses a non-image src and never inlines it as JS', () => {
    app.UI.showImageViewer('javascript:window.__pwned=1');

    const viewer = document.getElementById('imageViewer');
    expect(viewer).not.toBeNull();
    expect(viewer.querySelector('img').getAttribute('src')).toBeNull();
    viewer.querySelectorAll('*').forEach(el => {
        Array.from(el.attributes).forEach(a => expect(a.name.startsWith('on')).toBe(false));
    });

    app.UI.closeImageViewer();
});

test('post edit cancel restores the original markup from memory, not an attribute', () => {
    document.body.innerHTML = `
        <div data-post-id="p1"><div class="post-content">hello &amp; goodbye</div></div>`;

    app.PostInteractions.editPost('p1');
    expect(document.querySelector('.edit-post-textarea')).not.toBeNull();

    click(document.querySelector('[data-post-edit-cancel]'));

    expect(document.querySelector('.post-content').textContent).toBe('hello & goodbye');
    expect(app.PostInteractions.editOriginals.has('p1')).toBe(false);
});
