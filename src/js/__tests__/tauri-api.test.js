/**
 * Tauri API Mock Tests
 * Testing all Tauri command invocations and API interactions
 */

require('./setup');

describe('Tauri API Commands - User Management', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
        global.currentUser = { id: '123', username: 'testuser' };
    });

    test('login_user command', async () => {
        const mockUser = {
            id: '123',
            username: 'testuser',
            publicKey: 'test-public-key',
            deviceId: 'device-123'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockUser);

        const result = await TauriAPI.invoke('login_user', {
            username: 'testuser',
            password: 'password123'
        });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('login_user', {
            username: 'testuser',
            password: 'password123'
        });
        expect(result).toEqual(mockUser);
    });

    test('register_user command', async () => {
        const mockUser = {
            id: '456',
            username: 'newuser',
            email: 'new@example.com',
            publicKey: 'new-public-key'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockUser);

        const result = await TauriAPI.invoke('register_user', {
            username: 'newuser',
            email: 'new@example.com',
            password: 'securepass'
        });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('register_user', {
            username: 'newuser',
            email: 'new@example.com',
            password: 'securepass'
        });
        expect(result).toEqual(mockUser);
    });

    test('update_user_profile command', async () => {
        const updatedUser = {
            id: '123',
            username: 'testuser',
            bio: 'Updated bio',
            profilePicture: 'data:image/jpeg;base64,xxx'
        };

        global.__TAURI__.invoke.mockResolvedValue(updatedUser);

        const result = await TauriAPI.invoke('update_user_profile', {
            userId: '123',
            bio: 'Updated bio',
            profilePicture: 'data:image/jpeg;base64,xxx'
        });

        expect(result).toEqual(updatedUser);
    });

    test('upload_profile_picture command', async () => {
        const mockResponse = {
            id: '123',
            profilePicture: 'data:image/jpeg;base64,updated'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('upload_profile_picture', {
            userId: '123',
            fileData: 'base64imagedata',
            filename: 'profile.jpg',
            fileType: 'image/jpeg'
        });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('upload_profile_picture', {
            userId: '123',
            fileData: 'base64imagedata',
            filename: 'profile.jpg',
            fileType: 'image/jpeg'
        });
        expect(result).toEqual(mockResponse);
    });

    test('get_user_by_public_key command', async () => {
        const mockUser = {
            id: '789',
            username: 'founduser',
            publicKey: 'search-key'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockUser);

        const result = await TauriAPI.invoke('get_user_by_public_key', {
            publicKey: 'search-key'
        });

        expect(result).toEqual(mockUser);
    });
});

describe('Tauri API Commands - Posts', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
        global.currentUser = { id: '123' };
    });

    test('create_post command', async () => {
        const mockPost = {
            id: '456',
            userId: '123',
            content: 'Test post',
            createdAt: new Date().toISOString()
        };

        global.__TAURI__.invoke.mockResolvedValue(mockPost);

        const result = await TauriAPI.invoke('create_post', {
            userId: '123',
            content: 'Test post',
            attachments: null
        });

        expect(result).toEqual(mockPost);
    });

    test('get_all_posts command', async () => {
        const mockPosts = [
            { id: '1', content: 'Post 1', userId: '123' },
            { id: '2', content: 'Post 2', userId: '456' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockPosts);

        const result = await TauriAPI.invoke('get_all_posts', {
            userId: '123'
        });

        expect(result).toEqual(mockPosts);
    });

    test('edit_post command', async () => {
        const mockResponse = { success: true, post: { id: '456', content: 'Updated' } };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('edit_post', {
            postId: '456',
            userId: '123',
            newContent: 'Updated content'
        });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('edit_post', {
            postId: '456',
            userId: '123',
            newContent: 'Updated content'
        });
        expect(result).toEqual(mockResponse);
    });

    test('delete_post command', async () => {
        const mockResponse = { success: true };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('delete_post', {
            postId: '456',
            userId: '123'
        });

        expect(result).toEqual(mockResponse);
    });
});

describe('Tauri API Commands - Media', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('upload_media_file command', async () => {
        const mockResponse = {
            id: '789',
            postId: '456',
            filename: 'image.jpg',
            fileType: 'image/jpeg'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('upload_media_file', {
            fileData: 'base64data',
            filename: 'image.jpg',
            fileType: 'image/jpeg',
            fileSize: 1024,
            postId: '456'
        });

        expect(result).toEqual(mockResponse);
    });

    test('get_media_attachments command', async () => {
        const mockAttachments = [
            { id: '1', filename: 'file1.jpg', fileType: 'image/jpeg' },
            { id: '2', filename: 'file2.pdf', fileType: 'application/pdf' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockAttachments);

        const result = await TauriAPI.invoke('get_media_attachments', {
            postId: '456'
        });

        expect(result).toEqual(mockAttachments);
    });

    test('get_media_file_data command', async () => {
        const mockData = {
            id: '789',
            data: 'base64imagedata',
            fileType: 'image/jpeg'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockData);

        const result = await TauriAPI.invoke('get_media_file_data', {
            mediaId: '789'
        });

        expect(result).toEqual(mockData);
    });
});

describe('Tauri API Commands - Messages', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
        global.currentUser = { id: '123' };
    });

    test('send_encrypted_message command', async () => {
        const mockResponse = {
            id: '999',
            senderId: '123',
            recipientId: '456',
            content: 'Encrypted content',
            encrypted: true
        };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('send_encrypted_message', {
            senderId: '123',
            recipientId: '456',
            content: 'Hello friend'
        });

        expect(result).toEqual(mockResponse);
    });

    test('get_messages_for_user command', async () => {
        const mockMessages = [
            { id: '1', senderId: '123', recipientId: '456', content: 'Hi' },
            { id: '2', senderId: '456', recipientId: '123', content: 'Hello' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockMessages);

        const result = await TauriAPI.invoke('get_messages_for_user', {
            userId: '123'
        });

        expect(result).toEqual(mockMessages);
    });

    test('mark_conversation_as_read command', async () => {
        const mockResponse = { success: true, markedCount: 5 };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('mark_conversation_as_read', {
            userId: '123',
            otherUserId: '456'
        });

        expect(result).toEqual(mockResponse);
    });

    test('is_message_read command', async () => {
        global.__TAURI__.invoke.mockResolvedValue(true);

        const result = await TauriAPI.invoke('is_message_read', {
            messageId: '999',
            userId: '456'
        });

        expect(result).toBe(true);
    });

    test('edit_message command', async () => {
        const mockResponse = { success: true };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('edit_message', {
            messageId: '999',
            userId: '123',
            newContent: 'Edited message'
        });

        expect(result).toEqual(mockResponse);
    });

    test('delete_message command', async () => {
        const mockResponse = { success: true };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('delete_message', {
            messageId: '999',
            userId: '123'
        });

        expect(result).toEqual(mockResponse);
    });

    test('search_messages command', async () => {
        const mockResults = [
            { id: '1', content: 'Message containing search term' },
            { id: '2', content: 'Another search result' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockResults);

        const result = await TauriAPI.invoke('search_messages', {
            userId: '123',
            query: 'search'
        });

        expect(result).toEqual(mockResults);
    });
});

describe('Tauri API Commands - Message Features', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('add_message_reaction command', async () => {
        const mockResponse = { success: true, reactionId: '111' };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('add_message_reaction', {
            messageId: '999',
            userId: '123',
            emoji: '❤️'
        });

        expect(result).toEqual(mockResponse);
    });

    test('get_message_reactions command', async () => {
        const mockReactions = [
            { id: '1', userId: '123', emoji: '❤️' },
            { id: '2', userId: '456', emoji: '👍' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockReactions);

        const result = await TauriAPI.invoke('get_message_reactions', {
            messageId: '999'
        });

        expect(result).toEqual(mockReactions);
    });

    test('reply_to_message command', async () => {
        const mockResponse = {
            id: '222',
            threadId: '999',
            content: 'Reply content'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('reply_to_message', {
            originalMessageId: '999',
            senderId: '123',
            recipientId: '456',
            content: 'Reply content'
        });

        expect(result).toEqual(mockResponse);
    });

    test('get_message_thread command', async () => {
        const mockThread = [
            { id: '999', content: 'Original message', threadId: null },
            { id: '111', content: 'First reply', threadId: '999' },
            { id: '222', content: 'Second reply', threadId: '999' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockThread);

        const result = await TauriAPI.invoke('get_message_thread', {
            threadId: '999'
        });

        expect(result).toEqual(mockThread);
    });
});

describe('Tauri API Commands - Voice Messages', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('send_voice_message command', async () => {
        const mockResponse = {
            id: '333',
            senderId: '123',
            recipientId: '456',
            durationSeconds: 5.2
        };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('send_voice_message', {
            senderId: '123',
            recipientId: '456',
            audioData: 'base64audio',
            durationSeconds: 5.2,
            waveform: '[0.1,0.5,0.8,0.3]',
            threadId: null
        });

        expect(result).toEqual(mockResponse);
    });

    test('get_voice_messages command', async () => {
        const mockVoiceMessages = [
            { id: '1', senderId: '123', audioData: 'base64_1' },
            { id: '2', senderId: '456', audioData: 'base64_2' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockVoiceMessages);

        const result = await TauriAPI.invoke('get_voice_messages', {
            userId: '123'
        });

        expect(result).toEqual(mockVoiceMessages);
    });

    test('delete_voice_message command', async () => {
        const mockResponse = { success: true };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('delete_voice_message', {
            voiceMessageId: '333',
            userId: '123'
        });

        expect(result).toEqual(mockResponse);
    });
});

describe('Tauri API Commands - Friends', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('get_friends command', async () => {
        const mockFriends = [
            { id: '1', friendUserId: '456', friendUsername: 'Alice' },
            { id: '2', friendUserId: '789', friendUsername: 'Bob' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockFriends);

        const result = await TauriAPI.invoke('get_friends', {
            userId: '123'
        });

        expect(result).toEqual(mockFriends);
    });

    test('add_friend command', async () => {
        const mockResponse = {
            id: '444',
            userId: '123',
            friendUserId: '456'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('add_friend', {
            userId: '123',
            friendUserId: '456'
        });

        expect(result).toEqual(mockResponse);
    });

    test('create_friend_invite command', async () => {
        const mockInvite = {
            inviteCode: 'ABC123',
            expiresAt: new Date().toISOString(),
            usesRemaining: 5
        };

        global.__TAURI__.invoke.mockResolvedValue(mockInvite);

        const result = await TauriAPI.invoke('create_friend_invite', {
            userId: '123',
            uses: 5,
            hoursValid: 24
        });

        expect(result).toEqual(mockInvite);
    });

    test('use_friend_invite command', async () => {
        const mockFriend = {
            id: '789',
            username: 'invitedfriend'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockFriend);

        const result = await TauriAPI.invoke('use_friend_invite', {
            userId: '123',
            inviteCode: 'ABC123'
        });

        expect(result).toEqual(mockFriend);
    });

    test('export_friends_list command', async () => {
        const mockFriendsList = [
            { username: 'friend1', publicKey: 'key1' },
            { username: 'friend2', publicKey: 'key2' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockFriendsList);

        const result = await TauriAPI.invoke('export_friends_list', {
            userId: '123'
        });

        expect(result).toEqual(mockFriendsList);
    });

    test('import_friends_list command', async () => {
        const mockResult = {
            added: ['friend1', 'friend2'],
            skipped: ['friend3'],
            errors: []
        };

        global.__TAURI__.invoke.mockResolvedValue(mockResult);

        const result = await TauriAPI.invoke('import_friends_list', {
            userId: '123',
            friendsJson: '[{"username":"friend1","publicKey":"key1"}]'
        });

        expect(result).toEqual(mockResult);
    });

    test('get_recent_contacts command', async () => {
        const mockContacts = [
            { userId: '456', username: 'recent1', lastContact: '2024-01-01' },
            { userId: '789', username: 'recent2', lastContact: '2024-01-02' }
        ];

        global.__TAURI__.invoke.mockResolvedValue(mockContacts);

        const result = await TauriAPI.invoke('get_recent_contacts', {
            userId: '123',
            limit: 10
        });

        expect(result).toEqual(mockContacts);
    });

    test('update_recent_contact command', async () => {
        const mockResponse = { success: true };

        global.__TAURI__.invoke.mockResolvedValue(mockResponse);

        const result = await TauriAPI.invoke('update_recent_contact', {
            userId: '123',
            contactUserId: '456'
        });

        expect(result).toEqual(mockResponse);
    });
});

describe('Tauri API Commands - QR Codes', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('generate_qr_code command', async () => {
        const mockQRDataUrl = 'data:image/png;base64,qrcode';

        global.__TAURI__.invoke.mockResolvedValue(mockQRDataUrl);

        const result = await TauriAPI.invoke('generate_qr_code', {
            data: 'cipher://add-friend?username=test&public_key=key'
        });

        expect(result).toBe(mockQRDataUrl);
    });

    test('scan_qr_code_from_image command', async () => {
        const mockQRData = {
            username: 'scanneduser',
            publicKey: 'scanned-key'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockQRData);

        const result = await TauriAPI.invoke('scan_qr_code_from_image', {
            base64Image: 'data:image/png;base64,imagedata'
        });

        expect(result).toEqual(mockQRData);
    });

    test('parse_qr_code_data command', async () => {
        const mockParsedData = {
            username: 'parseduser',
            publicKey: 'parsed-key',
            peer_id: 'peer123',
            peer_addr: '/ip4/127.0.0.1/tcp/4001'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockParsedData);

        const result = await TauriAPI.invoke('parse_qr_code_data', {
            qrData: 'cipher://add-friend?username=parseduser&public_key=parsed-key'
        });

        expect(result).toEqual(mockParsedData);
    });
});

describe('Tauri API Commands - Platform', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('get_platform command', async () => {
        global.__TAURI__.invoke.mockResolvedValue('desktop');

        const result = await TauriAPI.invoke('get_platform');

        expect(result).toBe('desktop');
    });

    test('get_platform returns mobile', async () => {
        global.__TAURI__.invoke.mockResolvedValue('android');

        const result = await TauriAPI.invoke('get_platform');

        expect(result).toBe('android');
    });

    test('debug_log command', async () => {
        global.__TAURI__.invoke.mockResolvedValue(undefined);

        await TauriAPI.invoke('debug_log', {
            message: 'Test debug message'
        });

        expect(global.__TAURI__.invoke).toHaveBeenCalledWith('debug_log', {
            message: 'Test debug message'
        });
    });
});

describe('Tauri API Error Handling', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('should handle network errors', async () => {
        const networkError = new Error('Network error');
        global.__TAURI__.invoke.mockRejectedValue(networkError);

        await expect(TauriAPI.invoke('login_user', {
            username: 'test',
            password: 'pass'
        })).rejects.toThrow('Network error');
    });

    test('should handle authentication errors', async () => {
        const authError = new Error('Invalid credentials');
        global.__TAURI__.invoke.mockRejectedValue(authError);

        await expect(TauriAPI.invoke('login_user', {
            username: 'wrong',
            password: 'wrong'
        })).rejects.toThrow('Invalid credentials');
    });

    test('should handle permission errors', async () => {
        const permissionError = new Error('Permission denied');
        global.__TAURI__.invoke.mockRejectedValue(permissionError);

        await expect(TauriAPI.invoke('delete_post', {
            postId: '999',
            userId: '456'
        })).rejects.toThrow('Permission denied');
    });

    test('should handle validation errors', async () => {
        const validationError = new Error('Validation failed: username too short');
        global.__TAURI__.invoke.mockRejectedValue(validationError);

        await expect(TauriAPI.invoke('register_user', {
            username: 'ab',
            password: 'pass'
        })).rejects.toThrow('Validation failed');
    });
});

describe('Tauri API Initialization', () => {
    test('should retry initialization on failure', async () => {
        let attempts = 0;
        global.__TAURI__ = undefined;

        // Simulate API becoming available after 3 attempts
        Object.defineProperty(window, '__TAURI__', {
            get: () => {
                attempts++;
                if (attempts >= 3) {
                    return { invoke: jest.fn() };
                }
                return undefined;
            },
            configurable: true
        });

        const result = await TauriAPI.waitForAPI();

        expect(result).toBe(true);
        expect(attempts).toBeGreaterThanOrEqual(3);
    });

    test('should throw error after max retries', async () => {
        global.__TAURI__ = undefined;
        window.__TAURI_INVOKE__ = undefined;

        // Mock delay to speed up test
        TauriAPI.delay = jest.fn().mockResolvedValue();

        await expect(TauriAPI.waitForAPI()).rejects.toThrow('Tauri API failed to initialize');
    });

    test('should detect Tauri 2.x core.invoke API', async () => {
        global.__TAURI__ = {
            core: {
                invoke: jest.fn()
            }
        };

        const initialized = await TauriAPI.initialize();

        expect(initialized).toBe(true);
        expect(global.tauriInvoke).toBe(global.__TAURI__.core.invoke);
    });

    test('should detect legacy __TAURI_INVOKE__ API', async () => {
        global.__TAURI__ = undefined;
        window.__TAURI_INVOKE__ = jest.fn();

        const initialized = await TauriAPI.initialize();

        expect(initialized).toBe(true);
        expect(global.tauriInvoke).toBe(window.__TAURI_INVOKE__);
    });
});

describe('Tauri Plugin Integration', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
        global.__TAURI_INTERNALS__ = {
            transformCallback: jest.fn()
        };
    });

    test('barcode scanner plugin integration', async () => {
        const mockScanResult = {
            content: 'cipher://add-friend?username=test&public_key=key',
            format: 'QR_CODE'
        };

        global.__TAURI__.invoke.mockResolvedValue(mockScanResult);

        const result = await global.__TAURI__.invoke('plugin:barcode-scanner|scan', {
            windowed: true,
            formats: ['QR_CODE']
        });

        expect(result).toEqual(mockScanResult);
        expect(global.__TAURI__.invoke).toHaveBeenCalledWith(
            'plugin:barcode-scanner|scan',
            {
                windowed: true,
                formats: ['QR_CODE']
            }
        );
    });
});

describe('Tauri API Command Batching', () => {
    beforeEach(() => {
        global.__TAURI__ = {
            invoke: jest.fn()
        };
    });

    test('should batch multiple independent commands', async () => {
        const mockPosts = [{ id: '1', content: 'Post 1' }];
        const mockFriends = [{ id: '2', username: 'Friend 1' }];
        const mockMessages = [{ id: '3', content: 'Message 1' }];

        global.__TAURI__.invoke
            .mockResolvedValueOnce(mockPosts)
            .mockResolvedValueOnce(mockFriends)
            .mockResolvedValueOnce(mockMessages);

        const [posts, friends, messages] = await Promise.all([
            TauriAPI.invoke('get_all_posts', { userId: '123' }),
            TauriAPI.invoke('get_friends', { userId: '123' }),
            TauriAPI.invoke('get_messages_for_user', { userId: '123' })
        ]);

        expect(posts).toEqual(mockPosts);
        expect(friends).toEqual(mockFriends);
        expect(messages).toEqual(mockMessages);
        expect(global.__TAURI__.invoke).toHaveBeenCalledTimes(3);
    });

    test('should handle partial failures in batched commands', async () => {
        const mockPosts = [{ id: '1', content: 'Post 1' }];
        const error = new Error('Failed to load friends');

        global.__TAURI__.invoke
            .mockResolvedValueOnce(mockPosts)
            .mockRejectedValueOnce(error)
            .mockResolvedValueOnce([]);

        const results = await Promise.allSettled([
            TauriAPI.invoke('get_all_posts', { userId: '123' }),
            TauriAPI.invoke('get_friends', { userId: '123' }),
            TauriAPI.invoke('get_messages_for_user', { userId: '123' })
        ]);

        expect(results[0].status).toBe('fulfilled');
        expect(results[0].value).toEqual(mockPosts);
        expect(results[1].status).toBe('rejected');
        expect(results[1].reason).toEqual(error);
        expect(results[2].status).toBe('fulfilled');
    });
});