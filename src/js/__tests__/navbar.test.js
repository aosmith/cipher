/**
 * Notification rendering in navbar.js.
 *
 * Notification title/message/data are derived from peer activity. The previous
 * implementation built an inline onclick and escaped only single quotes in the
 * `data` field, so a double quote or a backslash escaped the handler entirely.
 */

const path = require('path');

const Navbar = require(path.join(__dirname, '..', 'navbar.js'));
const { loadApp } = require('./helpers/load-app');

const HOSTILE = '<img src=x onerror=window.__pwned=1>';
const HOSTILE_QUOTES = `" onmouseover="window.__pwned=1" x="`;

let app;

beforeAll(() => {
    app = loadApp();
    // navbar.js calls the Utils helper defined by main.js.
    window.Utils = app.Utils;
    global.Utils = app.Utils;
});

beforeEach(() => {
    document.body.innerHTML = '<div id="notificationsList"></div>';
    delete window.__pwned;
});

test('hostile notification fields cannot inject elements or handlers', () => {
    Navbar.renderNotifications([{
        id: HOSTILE_QUOTES,
        notificationType: HOSTILE_QUOTES,
        data: `{"a":"\\"}`,
        title: HOSTILE,
        message: HOSTILE,
        read: false,
        createdAt: new Date().toISOString(),
    }]);

    const list = document.getElementById('notificationsList');
    expect(list.querySelector('img[onerror]')).toBeNull();
    list.querySelectorAll('*').forEach(el => {
        Array.from(el.attributes).forEach(a => expect(a.name.startsWith('on')).toBe(false));
    });
    expect(window.__pwned).toBeUndefined();

    // Text is preserved for the user, just inert.
    expect(list.querySelector('.notification-title').textContent).toBe(HOSTILE);
});

test('clicking a notification delivers the raw id/type/payload to the handler', () => {
    const spy = jest.spyOn(Navbar, 'handleNotificationClick').mockImplementation(() => {});

    Navbar.renderNotifications([{
        id: HOSTILE_QUOTES,
        notificationType: 'post_comment',
        data: '{"postId":"p1"}',
        title: 't', message: 'm', read: true,
        createdAt: new Date().toISOString(),
    }]);

    document.querySelector('.notification-item').dispatchEvent(
        new window.MouseEvent('click', { bubbles: true })
    );

    expect(spy).toHaveBeenCalledWith(HOSTILE_QUOTES, 'post_comment', '{"postId":"p1"}');
});

test('the dismiss button does not also trigger the item handler', () => {
    const open = jest.spyOn(Navbar, 'handleNotificationClick').mockImplementation(() => {});
    const dismiss = jest.spyOn(Navbar, 'dismissNotification').mockImplementation(() => {});

    Navbar.renderNotifications([{
        id: 'n1', notificationType: 'message', data: null,
        title: 't', message: 'm', read: true, createdAt: new Date().toISOString(),
    }]);

    document.querySelector('.notification-dismiss').dispatchEvent(
        new window.MouseEvent('click', { bubbles: true })
    );

    expect(dismiss).toHaveBeenCalledWith('n1');
    expect(open).not.toHaveBeenCalled();
});
