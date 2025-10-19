/**
 * Jest Test Setup
 * Configure Jest environment for testing Cipher frontend
 */

// Add jest-dom matchers for better assertions
require('@testing-library/jest-dom');

// Mock Tauri API globally
global.__TAURI__ = {
    invoke: jest.fn(),
    convertFileSrc: jest.fn(src => src),
    path: {
        appDataDir: jest.fn(() => Promise.resolve('/mock/app/data')),
        appConfigDir: jest.fn(() => Promise.resolve('/mock/app/config'))
    },
    fs: {
        readTextFile: jest.fn(),
        writeTextFile: jest.fn(),
        readBinaryFile: jest.fn(),
        writeBinaryFile: jest.fn()
    }
};

// Mock window.TauriAPI
global.TauriAPI = {
    invoke: jest.fn(),
    convertFileSrc: jest.fn(src => src)
};

// Mock localStorage
const localStorageMock = {
    getItem: jest.fn(),
    setItem: jest.fn(),
    removeItem: jest.fn(),
    clear: jest.fn()
};
global.localStorage = localStorageMock;

// Mock sessionStorage
const sessionStorageMock = {
    getItem: jest.fn(),
    setItem: jest.fn(),
    removeItem: jest.fn(),
    clear: jest.fn()
};
global.sessionStorage = sessionStorageMock;

// Mock console methods to reduce test output noise
global.console = {
    ...console,
    log: jest.fn(),
    error: jest.fn(),
    warn: jest.fn(),
    info: jest.fn(),
    debug: jest.fn()
};

// Reset mocks between tests
beforeEach(() => {
    jest.clearAllMocks();
    localStorageMock.getItem.mockReset();
    localStorageMock.setItem.mockReset();
    localStorageMock.removeItem.mockReset();
    localStorageMock.clear.mockReset();

    // Reset DOM
    document.body.innerHTML = '';
    document.head.innerHTML = '';
});

// Cleanup after each test
afterEach(() => {
    jest.clearAllTimers();
});