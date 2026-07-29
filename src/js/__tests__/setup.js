/**
 * Jest Test Setup
 *
 * Runs in the jsdom environment configured in package.json. It deliberately
 * does NOT replace document/window/setTimeout with hand-rolled stubs - the old
 * version did, which fought jsdom and made every DOM assertion meaningless.
 * It only stubs what jsdom genuinely does not provide (the Tauri bridge) and
 * quiets the application's very chatty console logging.
 */

require('@testing-library/jest-dom');

// Tauri bridge. main.js reads window.__TAURI__.core.invoke and, at load time,
// registers window.__TAURI__.event.listen handlers.
function installTauriMock() {
    const invoke = jest.fn(() => Promise.resolve(null));
    window.__TAURI__ = {
        core: { invoke },
        event: { listen: jest.fn(() => Promise.resolve(() => {})), emit: jest.fn() },
        convertFileSrc: jest.fn(src => src),
    };
    return invoke;
}

global.installTauriMock = installTauriMock;
installTauriMock();

// Keep the real console reachable for debugging a failing test.
global.realConsole = console;

function silenceConsole() {
    jest.spyOn(console, 'log').mockImplementation(() => {});
    jest.spyOn(console, 'warn').mockImplementation(() => {});
    jest.spyOn(console, 'error').mockImplementation(() => {});
    jest.spyOn(console, 'info').mockImplementation(() => {});
    jest.spyOn(console, 'debug').mockImplementation(() => {});
}

// Also covers suites that load main.js in beforeAll.
beforeAll(silenceConsole);

beforeEach(() => {
    jest.spyOn(console, 'log').mockImplementation(() => {});
    jest.spyOn(console, 'warn').mockImplementation(() => {});
    jest.spyOn(console, 'error').mockImplementation(() => {});
    jest.spyOn(console, 'info').mockImplementation(() => {});
    jest.spyOn(console, 'debug').mockImplementation(() => {});
});

afterEach(() => {
    jest.restoreAllMocks();
});
