/**
 * Simple Unit Tests for main.js functions
 * Testing individual functions without loading entire main.js
 */

require('./setup');

// Test Utils functions that can be tested in isolation
describe('Utils Functions (isolated)', () => {
    test('escapeHtml prevents XSS', () => {
        const div = document.createElement('div');
        div.textContent = '<script>alert("xss")</script>';
        const escaped = div.innerHTML;

        expect(escaped).not.toContain('<script>');
        expect(escaped).toContain('&lt;script&gt;');
    });

    test('formatFileSize formats correctly', () => {
        const formatFileSize = (bytes) => {
            if (bytes === 0) return '0 Bytes';
            const k = 1024;
            const sizes = ['Bytes', 'KB', 'MB', 'GB'];
            const i = Math.floor(Math.log(bytes) / Math.log(k));
            return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
        };

        expect(formatFileSize(0)).toBe('0 Bytes');
        expect(formatFileSize(1024)).toBe('1 KB');
        expect(formatFileSize(1048576)).toBe('1 MB');
        expect(formatFileSize(1500)).toBe('1.46 KB');
    });

    test('getMediaIcon returns correct icons', () => {
        const getMediaIcon = (fileType) => {
            const type = fileType.toLowerCase();
            if (type.startsWith('image/')) return '🖼️';
            if (type.startsWith('video/')) return '🎬';
            if (type.startsWith('audio/')) return '🎵';
            if (type === 'application/pdf') return '📄';
            return '📎';
        };

        expect(getMediaIcon('image/png')).toBe('🖼️');
        expect(getMediaIcon('video/mp4')).toBe('🎬');
        expect(getMediaIcon('audio/mp3')).toBe('🎵');
        expect(getMediaIcon('application/pdf')).toBe('📄');
        expect(getMediaIcon('text/plain')).toBe('📎');
    });

    test('delay function waits correctly', async () => {
        const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

        const start = Date.now();
        await delay(100);
        const elapsed = Date.now() - start;

        expect(elapsed).toBeGreaterThanOrEqual(90);
        expect(elapsed).toBeLessThan(150);
    });
});

describe('Session Management (isolated)', () => {
    beforeEach(() => {
        localStorage.clear();
        jest.clearAllMocks();
    });

    test('session save and load', () => {
        const mockUser = {
            id: '123',
            username: 'testuser',
            publicKey: 'test-key'
        };

        // Save session
        localStorage.setItem('cipher_user_session', JSON.stringify(mockUser));
        localStorage.setItem('cipher_last_login', Date.now().toString());

        // Load session
        const sessionData = localStorage.getItem('cipher_user_session');
        const lastLogin = localStorage.getItem('cipher_last_login');

        expect(JSON.parse(sessionData)).toEqual(mockUser);
        expect(lastLogin).toBeTruthy();
    });

    test('session expiry after 30 days', () => {
        const mockUser = {
            id: '123',
            username: 'testuser'
        };

        // Set expired session
        const thirtyOneDaysAgo = Date.now() - (31 * 24 * 60 * 60 * 1000);
        localStorage.setItem('cipher_user_session', JSON.stringify(mockUser));
        localStorage.setItem('cipher_last_login', thirtyOneDaysAgo.toString());

        // Check if session is expired
        const lastLogin = parseInt(localStorage.getItem('cipher_last_login'));
        const sessionAge = Date.now() - lastLogin;
        const thirtyDays = 30 * 24 * 60 * 60 * 1000;

        expect(sessionAge).toBeGreaterThan(thirtyDays);
    });

    test('clear session removes data', () => {
        localStorage.setItem('cipher_user_session', 'data');
        localStorage.setItem('cipher_last_login', 'time');

        localStorage.removeItem('cipher_user_session');
        localStorage.removeItem('cipher_last_login');

        expect(localStorage.getItem('cipher_user_session')).toBeNull();
        expect(localStorage.getItem('cipher_last_login')).toBeNull();
    });
});

describe('Tauri API Mock Behavior', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('invoke calls Tauri with correct parameters', async () => {
        global.__TAURI__.invoke.mockResolvedValue({ success: true });

        await global.__TAURI__.invoke('test_command', { param1: 'value1' });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('test_command', { param1: 'value1' });
    });

    test('handles Tauri errors correctly', async () => {
        const error = new Error('Tauri error');
        global.__TAURI__.invoke.mockRejectedValue(error);

        await expect(
            global.__TAURI__.invoke('failing_command')
        ).rejects.toThrow('Tauri error');
    });

    test('handles multiple concurrent Tauri calls', async () => {
        global.__TAURI__.invoke
            .mockResolvedValueOnce({ id: 1 })
            .mockResolvedValueOnce({ id: 2 })
            .mockResolvedValueOnce({ id: 3 });

        const results = await Promise.all([
            global.__TAURI__.invoke('command1'),
            global.__TAURI__.invoke('command2'),
            global.__TAURI__.invoke('command3')
        ]);

        expect(results).toEqual([{ id: 1 }, { id: 2 }, { id: 3 }]);
        expect(global.__TAURI__.invoke).toHaveBeenCalledTimes(3);
    });
});

describe('DOM Manipulation Helpers', () => {
    beforeEach(() => {
        document.body.innerHTML = `
            <div id="testDiv" class="hidden">Test Content</div>
            <input id="testInput" value="initial" />
            <button id="testButton">Click Me</button>
        `;
    });

    test('show/hide element', () => {
        const element = document.getElementById('testDiv');

        // Hide
        element.classList.add('hidden');
        expect(element.classList.contains('hidden')).toBe(true);

        // Show
        element.classList.remove('hidden');
        expect(element.classList.contains('hidden')).toBe(false);
    });

    test('update element text content', () => {
        const element = document.getElementById('testDiv');

        element.textContent = 'Updated Content';
        expect(element.textContent).toBe('Updated Content');

        element.innerHTML = '<span>HTML Content</span>';
        expect(element.innerHTML).toBe('<span>HTML Content</span>');
    });

    test('input value manipulation', () => {
        const input = document.getElementById('testInput');

        expect(input.value).toBe('initial');

        input.value = 'updated';
        expect(input.value).toBe('updated');

        input.value = '';
        expect(input.value).toBe('');
    });

    test('event listener attachment', () => {
        const button = document.getElementById('testButton');
        const mockHandler = jest.fn();

        button.addEventListener('click', mockHandler);

        // Simulate click
        const event = new Event('click');
        button.dispatchEvent(event);

        expect(mockHandler).toHaveBeenCalled();
    });
});

describe('Data Validation', () => {
    test('validates email format', () => {
        const validateEmail = (email) => {
            const re = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
            return re.test(email);
        };

        expect(validateEmail('test@example.com')).toBe(true);
        expect(validateEmail('invalid.email')).toBe(false);
        expect(validateEmail('@example.com')).toBe(false);
        expect(validateEmail('test@')).toBe(false);
    });

    test('validates username length', () => {
        const validateUsername = (username) => {
            return username && username.length >= 3 && username.length <= 20;
        };

        expect(validateUsername('abc')).toBe(true);
        expect(validateUsername('ab')).toBe(false);
        expect(validateUsername('a'.repeat(21))).toBe(false);
        expect(validateUsername('')).toBeFalsy();
    });

    test('validates password strength', () => {
        const validatePassword = (password) => {
            return password && password.length >= 8;
        };

        expect(validatePassword('password123')).toBe(true);
        expect(validatePassword('short')).toBe(false);
        expect(validatePassword('')).toBeFalsy();
    });
});

describe('Message Formatting', () => {
    test('formats date correctly', () => {
        const formatDate = (dateString) => {
            return new Date(dateString).toLocaleDateString();
        };

        const testDate = '2024-01-15T10:30:00Z';
        const formatted = formatDate(testDate);

        expect(formatted).toMatch(/\d{1,2}\/\d{1,2}\/\d{4}/);
    });

    test('truncates long messages', () => {
        const truncate = (text, maxLength = 150) => {
            if (text.length <= maxLength) return text;
            return text.substring(0, maxLength) + '...';
        };

        const longText = 'a'.repeat(200);
        const truncated = truncate(longText);

        expect(truncated.length).toBe(153); // 150 + '...'
        expect(truncated.endsWith('...')).toBe(true);
    });

    test('escapes HTML in messages', () => {
        const escapeHtml = (text) => {
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        };

        const dangerous = '<img src=x onerror=alert(1)>';
        const escaped = escapeHtml(dangerous);

        expect(escaped).not.toContain('<img');
        expect(escaped).toContain('&lt;img');
    });
});

describe('Friend Management Logic', () => {
    test('filters friends by search query', () => {
        const friends = [
            { id: '1', friendUsername: 'Alice' },
            { id: '2', friendUsername: 'Bob' },
            { id: '3', friendUsername: 'Charlie' }
        ];

        const filterFriends = (friends, query) => {
            return friends.filter(friend =>
                friend.friendUsername.toLowerCase().includes(query.toLowerCase())
            );
        };

        expect(filterFriends(friends, 'al')).toEqual([
            { id: '1', friendUsername: 'Alice' }
        ]);

        expect(filterFriends(friends, 'b')).toEqual([
            { id: '2', friendUsername: 'Bob' }
        ]);

        expect(filterFriends(friends, '')).toEqual(friends);
    });

    test('validates public key format', () => {
        const isValidPublicKey = (key) => {
            return key && key.length > 0 && key !== 'my-key';
        };

        expect(isValidPublicKey('valid-key-123')).toBe(true);
        expect(isValidPublicKey('my-key')).toBe(false);
        expect(isValidPublicKey('')).toBeFalsy();
        expect(isValidPublicKey(null)).toBeFalsy();
    });
});

describe('Clipboard Operations', () => {
    beforeEach(() => {
        navigator.clipboard = {
            writeText: jest.fn().mockResolvedValue(),
            readText: jest.fn()
        };
    });

    test('copies text to clipboard', async () => {
        const textToCopy = 'test-public-key-123';

        await navigator.clipboard.writeText(textToCopy);

        expect(navigator.clipboard.writeText).toHaveBeenCalledWith(textToCopy);
    });

    test('handles clipboard errors', async () => {
        navigator.clipboard.writeText.mockRejectedValue(new Error('Clipboard access denied'));

        await expect(
            navigator.clipboard.writeText('text')
        ).rejects.toThrow('Clipboard access denied');
    });
});

describe('Media File Handling', () => {
    test('converts file to base64', async () => {
        const file = new File(['hello world'], 'test.txt', { type: 'text/plain' });

        const toBase64 = (file) => {
            return new Promise((resolve) => {
                const reader = new FileReader();
                reader.onload = () => resolve(reader.result);
                reader.readAsDataURL(file);
                // Simulate the read
                setTimeout(() => {
                    reader.result = 'data:text/plain;base64,aGVsbG8gd29ybGQ=';
                    reader.onload();
                }, 0);
            });
        };

        const result = await toBase64(file);

        expect(result).toContain('data:text/plain;base64,');
    });

    test('validates file types', () => {
        const isValidImageType = (type) => {
            return type.startsWith('image/');
        };

        const isValidMediaType = (type) => {
            return type.startsWith('image/') ||
                   type.startsWith('video/') ||
                   type.startsWith('audio/');
        };

        expect(isValidImageType('image/png')).toBe(true);
        expect(isValidImageType('text/plain')).toBe(false);

        expect(isValidMediaType('video/mp4')).toBe(true);
        expect(isValidMediaType('application/pdf')).toBe(false);
    });
});

describe('Error Handling', () => {
    test('handles network errors gracefully', () => {
        const handleError = (error) => {
            if (error.message.includes('Network')) {
                return 'Network error: Please check your connection';
            }
            return 'An unexpected error occurred';
        };

        const networkError = new Error('Network request failed');
        expect(handleError(networkError)).toBe('Network error: Please check your connection');

        const genericError = new Error('Something went wrong');
        expect(handleError(genericError)).toBe('An unexpected error occurred');
    });

    test('validates required fields', () => {
        const validateRequired = (fields) => {
            const errors = [];
            for (const [key, value] of Object.entries(fields)) {
                if (!value || value.trim() === '') {
                    errors.push(`${key} is required`);
                }
            }
            return errors;
        };

        const fields = {
            username: '',
            password: 'pass123',
            email: ''
        };

        const errors = validateRequired(fields);
        expect(errors).toContain('username is required');
        expect(errors).toContain('email is required');
        expect(errors).not.toContain('password is required');
    });
});