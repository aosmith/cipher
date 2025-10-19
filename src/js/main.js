console.log('[MAIN.JS] ===== FILE LOADING STARTED =====');
// Main application JavaScript - consolidated and optimized
console.log('[MAIN.JS] JavaScript is loading...');

// Global variables
let currentUser = null;
let tauriInvoke = null;
let allFriends = [];
let selectedRecipient = null;

// Removed legacy WebSocket notification system - now using WebRTC P2P

// Utility functions
const Utils = {
    async delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    },

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    },

    async fileToBase64(file) {
        return new Promise((resolve, reject) => {
            console.log('fileToBase64 - Starting file read for:', file.name, 'Size:', file.size);
            const reader = new FileReader();

            reader.onload = () => {
                const dataUrl = reader.result;
                console.log('fileToBase64 - File read completed, data URL length:', dataUrl.length);

                // Validate base64 structure
                const parts = dataUrl.split(',');
                if (parts.length !== 2) {
                    console.error('fileToBase64 - Invalid data URL format, parts:', parts.length);
                    reject(new Error('Invalid data URL format'));
                    return;
                }

                const base64Data = parts[1];
                console.log('fileToBase64 - Base64 preview (first 100 chars):', base64Data.substring(0, 100));
                console.log('fileToBase64 - Base64 preview (last 100 chars):', base64Data.substring(base64Data.length - 100));

                // Validate base64 characters
                const base64Regex = /^[A-Za-z0-9+/]*={0,2}$/;
                if (!base64Regex.test(base64Data)) {
                    console.error('fileToBase64 - Invalid base64 characters detected');
                    reject(new Error('Invalid base64 data'));
                    return;
                }

                console.log('fileToBase64 - Validation passed ✓');
                resolve(dataUrl);
            };

            reader.onerror = error => {
                console.error('fileToBase64 - File read error:', error);
                reject(error);
            };

            reader.onprogress = (event) => {
                if (event.lengthComputable) {
                    const percentComplete = Math.round((event.loaded / event.total) * 100);
                    console.log(`fileToBase64 - Progress: ${percentComplete}%`);
                }
            };

            console.log('fileToBase64 - Starting readAsDataURL');
            reader.readAsDataURL(file);
        });
    },

    formatFileSize(bytes) {
        if (bytes === 0) return '0 Bytes';
        const k = 1024;
        const sizes = ['Bytes', 'KB', 'MB', 'GB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    },

    getMediaIcon(fileType) {
        const type = fileType.toLowerCase();
        if (type.startsWith('image/')) return '🖼️';
        if (type.startsWith('video/')) return '🎬';
        if (type.startsWith('audio/')) return '🎵';
        if (type === 'application/pdf') return '📄';
        return '📎';
    },

    getMediaIconClass(fileType) {
        const type = fileType.toLowerCase();
        if (type.startsWith('image/')) return 'media-icon-image';
        if (type.startsWith('video/')) return 'media-icon-video';
        if (type.startsWith('audio/')) return 'media-icon-audio';
        if (type === 'application/pdf') return 'media-icon-pdf';
        return 'media-icon-file';
    }
};

// Tauri API management
const TauriAPI = {
    async initialize() {
        if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
            tauriInvoke = window.__TAURI__.core.invoke;
            await this.debugLog('Tauri 2.x API found (core.invoke)');
            return true;
        }
        if (window.__TAURI__ && window.__TAURI__.invoke) {
            tauriInvoke = window.__TAURI__.invoke;
            await this.debugLog('Tauri 1.x API found (legacy invoke)');
            return true;
        }
        if (window.__TAURI_INVOKE__) {
            tauriInvoke = window.__TAURI_INVOKE__;
            await this.debugLog('Tauri API found (direct invoke)');
            return true;
        }
        console.log('Tauri API not yet available, retrying...');
        return false;
    },

    async waitForAPI() {
        if (tauriInvoke) return true;

        for (let i = 0; i < 50; i++) {
            if (await this.initialize()) {
                await this.debugLog('Tauri API initialized successfully after ' + (i + 1) + ' attempts');
                return true;
            }
            await Utils.delay(100);
        }
        throw new Error('Tauri API failed to initialize');
    },

    async invoke(command, args = {}) {
        await this.waitForAPI();
        return await tauriInvoke(command, args);
    },

    async debugLog(message) {
        if (tauriInvoke) {
            try {
                await tauriInvoke('debug_log', { message: message });
            } catch (e) {
                console.log('Debug: ' + message);
            }
        } else {
            console.log('Debug (no Tauri): ' + message);
        }
    }
};

// Session management
const Session = {
    save(user) {
        localStorage.setItem('cipher_user_session', JSON.stringify(user));
        localStorage.setItem('cipher_last_login', Date.now().toString());
    },

    load() {
        try {
            const sessionData = localStorage.getItem('cipher_user_session');
            const lastLogin = localStorage.getItem('cipher_last_login');

            if (sessionData && lastLogin) {
                const sessionAge = Date.now() - parseInt(lastLogin);
                const thirtyDays = 30 * 24 * 60 * 60 * 1000;

                if (sessionAge < thirtyDays) {
                    return JSON.parse(sessionData);
                }
            }
        } catch (error) {
            console.warn('Failed to load user session:', error);
        }
        return null;
    },

    clear() {
        localStorage.removeItem('cipher_user_session');
        localStorage.removeItem('cipher_last_login');
    },

    async attemptAutoLogin() {
        const savedUser = this.load();
        if (savedUser) {
            currentUser = savedUser;

            // Initialize P2P system for auto-logged in user (non-blocking)
            const publicKey = savedUser.publicKey || savedUser.public_key; // Support both formats
            const deviceId = savedUser.deviceId || savedUser.device_id; // Support both formats
            P2P.initialize(savedUser.id, publicKey, deviceId).then(() => {
                console.log('P2P system initialized during auto-login');
            }).catch((error) => {
                console.error('Failed to initialize P2P during auto-login:', error);
                // P2P is not critical for basic functionality
            });

            return true;
        }
        return false;
    }
};

// UI State management
const UI = {
    clearErrors() {
        document.querySelectorAll('.error, .success').forEach(el => {
            el.classList.add('hidden');
            el.textContent = '';
        });
    },

    showError(elementId, message) {
        const element = document.getElementById(elementId);
        if (element) {
            element.textContent = message;
            element.classList.remove('hidden');
        }
    },

    showSuccess(elementId, message) {
        const element = document.getElementById(elementId);
        if (element) {
            element.textContent = message;
            element.classList.remove('hidden');
        }
    },

    updateModalLayout(modalContentElement) {
        if (!modalContentElement) return;

        const viewportHeight = window.innerHeight;
        const contentHeight = modalContentElement.scrollHeight;

        if (contentHeight < viewportHeight * 0.6) {
            modalContentElement.classList.remove('filled');
            modalContentElement.classList.add('centered');
        } else {
            modalContentElement.classList.remove('centered');
            modalContentElement.classList.add('filled');
        }
    },

    setActiveNavLink(activeId) {
        const navLinks = ['postsNavLink', 'createPostNavLink', 'messagesNavLink', 'friendsNavLink', 'profileNavLink'];
        navLinks.forEach(id => {
            const element = document.getElementById(id);
            if (element) {
                element.classList.toggle('active', id === activeId);
            }
        });
    },

    hideAllTabs() {
        const tabs = ['postsTab', 'createPostTab', 'messagesTab', 'friendsTab', 'profileTab'];
        tabs.forEach(tabId => {
            const tab = document.getElementById(tabId);
            if (tab) tab.classList.add('hidden');
        });
    },

    showTab(tabId, contentId, navId, loadFunction) {
        this.hideAllTabs();
        document.getElementById(tabId).classList.remove('hidden');
        this.setActiveNavLink(navId);

        if (loadFunction) loadFunction();

        setTimeout(() => {
            const content = document.getElementById(contentId);
            if (content) this.updateModalLayout(content);
        }, 100);
    },

    updateUserInterface() {
        if (!currentUser) return;

        const userGreeting = document.getElementById('userGreeting');
        if (userGreeting) {
            userGreeting.textContent = currentUser.username;
        }

        const userPublicKey = document.getElementById('userPublicKey');
        if (userPublicKey && currentUser.publicKey) {
            userPublicKey.textContent = currentUser.publicKey;
        }

        // Update navbar public key display using Navbar module
        console.log('[UI] currentUser object:', currentUser);
        console.log('[UI] currentUser.publicKey:', currentUser.publicKey);
        console.log('[UI] currentUser.id:', currentUser.id);
        if (typeof Navbar !== 'undefined' && currentUser.publicKey) {
            Navbar.updatePublicKey(currentUser.publicKey);
        }
    }
};

// Navigation functions
function showLogin() {
    document.getElementById('loginForm').classList.remove('hidden');
    document.getElementById('dashboard').classList.add('hidden');
    document.body.classList.remove('dashboard-view');
    document.body.classList.remove('app-loading');
    // Hide logged-in navbar elements using Navbar module
    if (typeof Navbar !== 'undefined') {
        Navbar.updateLoginState(false);
    }
    UI.clearErrors();

    setTimeout(() => {
        const loginContent = document.querySelector('#loginForm .modal-content');
        if (loginContent) UI.updateModalLayout(loginContent);
    }, 100);
}

function showDashboard() {
    document.getElementById('loginForm').classList.add('hidden');
    document.getElementById('dashboard').classList.remove('hidden');
    document.body.classList.add('dashboard-view');
    document.body.classList.remove('app-loading');
    // Show logged-in navbar elements using Navbar module
    if (typeof Navbar !== 'undefined') {
        Navbar.updateLoginState(true);
    }
    UI.clearErrors();
    UI.updateUserInterface();
    loadPosts();
    showFeed();
}

function showFeed() {
    UI.showTab('postsTab', 'postsContent', 'postsNavLink', loadPosts);
}

function showPosts() {
    showFeed();
}

function showMessages() {
    UI.showTab('messagesTab', 'messagesContent', 'messagesNavLink', loadMessages);
    // Clear message notification indicator when viewing messages
    if (notificationManager) {
        notificationManager.clearMessageIndicator();
    }
}

function showFriends() {
    UI.showTab('friendsTab', 'friendsContent', 'friendsNavLink', loadFriends);
}

function showCreatePostPage() {
    UI.showTab('createPostTab', null, 'createPostNavLink', () => {
        console.log('[CREATE_POST] Initializing create post page');
        document.getElementById('createPostTextarea').value = '';

        // Clear and setup file input
        const fileInput = document.getElementById('createPostAttachments');
        console.log('[CREATE_POST] File input found:', fileInput);

        if (!fileInput) {
            console.error('[CREATE_POST] File input element not found!');
            return;
        }

        // Clear file input value
        fileInput.value = '';
        document.getElementById('fileCount').textContent = '';

        // Remove any existing event listeners by cloning
        const newFileInput = fileInput.cloneNode(true);
        fileInput.parentNode.replaceChild(newFileInput, fileInput);
        console.log('[CREATE_POST] File input cloned and replaced');

        // Get the new element from DOM (it's been replaced)
        const actualFileInput = document.getElementById('createPostAttachments');
        console.log('[CREATE_POST] Got actual file input from DOM:', actualFileInput);

        // Add event listener to the actual element in the DOM
        actualFileInput.addEventListener('change', function(e) {
            console.log('[CREATE_POST] File input change event triggered!');
            console.log('[CREATE_POST] Event target:', e.target);
            console.log('[CREATE_POST] Files selected:', this.files ? this.files.length : 0);

            const fileCount = this.files ? this.files.length : 0;
            const countDisplay = document.getElementById('fileCount');

            if (fileCount > 0) {
                let totalSize = 0;
                let fileInfo = [];

                for (let i = 0; i < this.files.length; i++) {
                    const file = this.files[i];
                    totalSize += file.size;
                    fileInfo.push(`${file.name} (${Utils.formatFileSize(file.size)})`);
                    console.log(`[CREATE_POST] File ${i+1}: ${file.name}, ${file.size} bytes`);
                }

                // Check size limits
                const maxFileSize = 10 * 1024 * 1024; // 10MB
                const maxTotalSize = 50 * 1024 * 1024; // 50MB
                let hasOversizedFile = false;

                for (let i = 0; i < this.files.length; i++) {
                    if (this.files[i].size > maxFileSize) {
                        hasOversizedFile = true;
                        break;
                    }
                }

                if (hasOversizedFile) {
                    countDisplay.innerHTML = `<span style="color: var(--color-error);">⚠️ Some files exceed 10MB limit</span>`;
                } else if (totalSize > maxTotalSize) {
                    countDisplay.innerHTML = `<span style="color: var(--color-error);">⚠️ Total size exceeds 50MB limit</span>`;
                } else {
                    countDisplay.textContent = `${fileCount} file${fileCount !== 1 ? 's' : ''} selected (${Utils.formatFileSize(totalSize)} total)`;
                }
                console.log('[CREATE_POST] File count display updated');
            } else {
                countDisplay.textContent = '';
                console.log('[CREATE_POST] No files selected, cleared display');
            }
        });

        // Also add a click listener to the label to debug
        const label = document.querySelector('label[for="createPostAttachments"]');
        if (label) {
            console.log('[CREATE_POST] Found label for file input');
            label.addEventListener('click', function(e) {
                console.log('[CREATE_POST] Label clicked!');
            });
        }

        setTimeout(() => document.getElementById('createPostTextarea').focus(), 100);
    });
}

function showEditProfile() {
    UI.showTab('profileTab', 'profileContent', 'profileNavLink', () => {
        ProfileManager.createProfileTab();
        ProfileManager.loadCurrentProfile();
    });
}

function showAddFriend() {
    UI.showTab('addFriendTab', 'addFriendContent', 'addFriendNavLink', () => {
        // Clear the input field when showing the tab
        const input = document.getElementById('addFriendPublicKey');
        if (input) {
            input.value = '';
            setTimeout(() => input.focus(), 100);
        }
    });
}

// Authentication
// Toggle between create and restore sections
function showRestoreSection() {
    document.getElementById('createAccountSection').classList.add('hidden');
    document.getElementById('restoreAccountSection').classList.remove('hidden');
    document.getElementById('loginError').classList.add('hidden');
    document.getElementById('loginSuccess').classList.add('hidden');
}

function showCreateSection() {
    document.getElementById('restoreAccountSection').classList.add('hidden');
    document.getElementById('createAccountSection').classList.remove('hidden');
    document.getElementById('loginError').classList.add('hidden');
    document.getElementById('loginSuccess').classList.add('hidden');
}

// Global variable to store user pending authentication
let pendingAuthUser = null;

// Create new account
async function handleCreateAccount() {
    const displayName = document.getElementById('newDisplayName').value.trim();

    if (!displayName) {
        UI.showError('loginError', 'Please enter a display name');
        return;
    }

    try {
        UI.showSuccess('loginSuccess', 'Creating new account...');
        await TauriAPI.debugLog('Creating new user: ' + displayName);

        const result = await TauriAPI.invoke('create_new_user', {
            displayName: displayName
        });

        console.log('[CREATE_ACCOUNT] Raw result:', result);
        console.log('[CREATE_ACCOUNT] Result type:', typeof result);
        console.log('[CREATE_ACCOUNT] Result keys:', Object.keys(result || {}));

        const user = result.user;
        const recoveryPhrase = result.recoveryPhrase;

        console.log('[CREATE_ACCOUNT] User:', user);
        console.log('[CREATE_ACCOUNT] RecoveryPhrase:', recoveryPhrase);

        if (user && recoveryPhrase) {
            UI.showSuccess('loginSuccess', 'Account created successfully!');
            console.log('IMPORTANT: Save your recovery phrase:', recoveryPhrase);

            // Store user for later authentication
            pendingAuthUser = user;

            // Show recovery phrase modal
            showRecoveryPhraseModal(recoveryPhrase);
        } else {
            console.error('[CREATE_ACCOUNT] Missing user or recoveryPhrase!', {user, recoveryPhrase});
            UI.showError('loginError', 'Failed to create account - invalid response from server');
        }
    } catch (error) {
        console.error('[CREATE_ACCOUNT] Exception:', error);
        await TauriAPI.debugLog('Account creation error: ' + error.toString());
        UI.showError('loginError', 'Account creation failed: ' + error);
    }
}

// Show recovery phrase modal
function showRecoveryPhraseModal(recoveryPhrase) {
    document.getElementById('recoveryPhraseText').textContent = recoveryPhrase;
    document.getElementById('recoveryPhraseModal').classList.remove('hidden');

    // Store phrase for copying
    window.currentRecoveryPhrase = recoveryPhrase;
}

// Copy recovery phrase to clipboard
async function copyRecoveryPhrase() {
    const phrase = window.currentRecoveryPhrase;
    if (!phrase) return;

    try {
        // Try modern clipboard API first
        if (navigator.clipboard && navigator.clipboard.writeText) {
            await navigator.clipboard.writeText(phrase);
            alert('Recovery phrase copied to clipboard!');
        } else {
            // Fallback: create temporary textarea
            const textarea = document.createElement('textarea');
            textarea.value = phrase;
            textarea.style.position = 'fixed';
            textarea.style.opacity = '0';
            document.body.appendChild(textarea);
            textarea.select();
            document.execCommand('copy');
            document.body.removeChild(textarea);
            alert('Recovery phrase copied to clipboard!');
        }
    } catch (error) {
        console.error('Failed to copy:', error);
        alert('Could not copy automatically. Please manually copy the text above.');
    }
}

// Confirm recovery phrase saved and proceed to dashboard
async function confirmRecoveryPhraseSaved() {
    document.getElementById('recoveryPhraseModal').classList.add('hidden');
    window.currentRecoveryPhrase = null;

    if (pendingAuthUser) {
        await completeAuthentication(pendingAuthUser);
        pendingAuthUser = null;
    }
}

// Restore existing account
async function handleRestoreAccount() {
    const displayName = document.getElementById('restoreDisplayName').value.trim();
    const recoveryPhrase = document.getElementById('restoreRecoveryPhrase').value.trim();

    if (!displayName) {
        UI.showError('loginError', 'Please enter your display name');
        return;
    }

    if (!recoveryPhrase) {
        UI.showError('loginError', 'Please enter your recovery phrase');
        return;
    }

    // Validate recovery phrase has 24 words
    const words = recoveryPhrase.split(/\s+/).filter(w => w.length > 0);
    if (words.length !== 24) {
        UI.showError('loginError', 'Recovery phrase must be exactly 24 words');
        return;
    }

    try {
        UI.showSuccess('loginSuccess', 'Restoring account...');
        await TauriAPI.debugLog('Restoring user: ' + displayName);

        const user = await TauriAPI.invoke('restore_from_recovery_phrase', {
            displayName: displayName,
            recoveryPhrase: recoveryPhrase
        });

        if (user) {
            UI.showSuccess('loginSuccess', 'Account restored successfully!');
            await completeAuthentication(user);
        } else {
            UI.showError('loginError', 'Failed to restore account');
        }
    } catch (error) {
        await TauriAPI.debugLog('Account restoration error: ' + error.toString());
        UI.showError('loginError', 'Account restoration failed: ' + error);
    }
}

// Complete authentication and transition to dashboard
async function completeAuthentication(user) {
    console.log('[AUTH] Full user object received from backend:', user);
    console.log('[AUTH] user.id:', user.id);
    console.log('[AUTH] user.publicKey:', user.publicKey);
    console.log('[AUTH] user.deviceId:', user.deviceId);
    console.log('[AUTH] All keys in user object:', Object.keys(user));

    currentUser = user;
    Session.save(user);

    // Show dashboard immediately to avoid UI flash
    showDashboard();

    // Initialize Iroh P2P system in background
    try {
        await P2P.initialize(user.id, user.publicKey, user.deviceId);
        console.log('Iroh P2P system initialized successfully');
    } catch (error) {
        console.error('Failed to initialize P2P - Error name:', error.name);
        console.error('Failed to initialize P2P - Error message:', error.message);
        console.error('Failed to initialize P2P - Error stack:', error.stack);
        console.error('Failed to initialize P2P - Full error:', JSON.stringify(error));
        // Continue anyway - P2P is not critical for basic functionality
    }
}

async function handleLogout() {
    // Shutdown P2P system
    try {
        await P2P.shutdown();
        console.log('P2P system shut down');
    } catch (error) {
        console.error('Failed to stop P2P server:', error);
    }

    currentUser = null;
    Session.clear();
    showLogin();
}

// Posts Management
const PostManager = {
    async create(content, attachments = null) {
        console.log('📝 Creating post:', { userId: currentUser.id, contentLength: content.length, hasAttachments: !!attachments });

        try {
            // Validate file sizes before processing
            if (attachments && attachments.length > 0) {
                const maxFileSize = 10 * 1024 * 1024; // 10MB limit per file
                const maxTotalSize = 50 * 1024 * 1024; // 50MB total limit
                let totalSize = 0;

                for (let i = 0; i < attachments.length; i++) {
                    const file = attachments[i];
                    if (file.size > maxFileSize) {
                        throw new Error(`File "${file.name}" is too large. Maximum file size is 10MB.`);
                    }
                    totalSize += file.size;
                    if (totalSize > maxTotalSize) {
                        throw new Error('Total attachment size exceeds 50MB limit.');
                    }
                }
            }

            const post = await TauriAPI.invoke('create_post', {
                userId: currentUser.id,
                content: content,
                attachments: null
            });

            console.log('Post created successfully:', post);

            if (attachments && attachments.length > 0) {
                console.log('Uploading', attachments.length, 'attachments...');
                await this.uploadAttachments(post.id, attachments);
                console.log('Attachments uploaded');
            }

            // Publish post to P2P network
            if (P2P.initialized) {
                try {
                    await P2P.publishPost(content, post.id);
                    console.log('Post published to P2P network');
                } catch (error) {
                    console.error('Failed to publish post to P2P:', error);
                    // Don't throw - post is still created locally
                }
            }

            return post;
        } catch (error) {
            console.error('Failed to create post:', error);
            throw error;
        }
    },

    async uploadAttachments(postId, files) {
        if (!files || files.length === 0) {
            console.log('No files to upload');
            return;
        }

        console.log(`Starting upload of ${files.length} files for post ${postId}`);
        for (let i = 0; i < files.length; i++) {
            const file = files[i];
            console.log(`Uploading file ${i + 1}/${files.length}:`, file.name, file.type, file.size, 'bytes');

            try {
                console.log('Starting file reader for file:', file.name);
                const base64Data = await Utils.fileToBase64(file);
                console.log('File read complete, data URL length:', base64Data.length);

                const base64Only = base64Data.split(',')[1];
                console.log('Base64 extracted, length:', base64Only ? base64Only.length : 0);

                if (!base64Only) {
                    console.error('Failed to extract base64 data from data URL');
                    throw new Error('Failed to process file data');
                }

                console.log('Calling upload_media_file Tauri command...');
                const result = await TauriAPI.invoke('upload_media_file', {
                    fileData: base64Only,
                    filename: file.name,
                    fileType: file.type,
                    fileSize: file.size,
                    postId: postId
                });
                console.log(`File ${i + 1} uploaded successfully:`, result);
            } catch (error) {
                console.error(`Error uploading file ${i + 1}:`, error);
                throw error;
            }
        }
        console.log('All files uploaded');
    },

    async getMediaAttachments(postId) {
        try {
            console.log(`Getting media attachments for post ${postId}`);
            const attachments = await TauriAPI.invoke('get_media_attachments', { postId: postId });
            console.log(`Retrieved ${attachments.length} attachments:`, attachments);
            return attachments;
        } catch (error) {
            console.error('Failed to get media attachments:', error);
            return [];
        }
    },

    createMediaPreview(media) {
        console.log('Creating media preview:', {
            id: media.id,
            type: media.fileType,
            hasData: !!media.data,
            dataLength: media.data ? media.data.length : 0
        });

        // For images, show the actual image using embedded data
        if (media.fileType && media.fileType.startsWith('image/')) {
            if (media.data && media.data.length > 0) {
                // Validate base64 from database
                console.log('Retrieved base64 preview (first 100 chars):', media.data.substring(0, 100));
                console.log('Retrieved base64 preview (last 100 chars):', media.data.substring(media.data.length - 100));

                // Check if base64 is valid
                const base64Regex = /^[A-Za-z0-9+/]*={0,2}$/;
                if (!base64Regex.test(media.data)) {
                    console.error('createMediaPreview - Invalid base64 characters in retrieved data!');
                    return `<div class="media-placeholder">
                        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                            <circle cx="8.5" cy="8.5" r="1.5"></circle>
                            <polyline points="21 15 16 10 5 21"></polyline>
                        </svg>
                        <p>Corrupted image data</p>
                    </div>`;
                }

                const dataUrl = `data:${media.fileType};base64,${media.data}`;
                console.log('createMediaPreview - Base64 validation passed ✓');
                console.log('createMediaPreview - Creating image with data URL (length:', dataUrl.length, ')');
                return `<img src="${dataUrl}" alt="Image" class="post-image">`;
            } else {
                console.warn('Image has no data - showing placeholder');
                return `<div class="media-placeholder">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                        <circle cx="8.5" cy="8.5" r="1.5"></circle>
                        <polyline points="21 15 16 10 5 21"></polyline>
                    </svg>
                    <p>Image unavailable</p>
                </div>`;
            }
        }
        // For other file types, show icon
        console.log('Creating file icon for non-image media');
        return `<div class="media-icon ${Utils.getMediaIconClass(media.fileType)}">${Utils.getMediaIcon(media.fileType)}</div>`;
    },

    async viewMedia(mediaId) {
        try {
            const mediaData = await TauriAPI.invoke('get_media_file_data', { mediaId: mediaId });

            if (mediaData && mediaData.data) {
                const mimeType = mediaData.fileType || 'application/octet-stream';
                const dataUrl = `data:${mimeType};base64,${mediaData.data}`;
                window.open(dataUrl, '_blank');
            }
        } catch (error) {
            UI.showError('dashboardError', 'Failed to view media: ' + error);
        }
    }
};

// Profile Management
const ProfileManager = {
    createProfileTab() {
        let profileTab = document.getElementById('profileTab');

        if (!profileTab) {
            const dashboard = document.getElementById('dashboard');
            profileTab = document.createElement('div');
            profileTab.id = 'profileTab';
            profileTab.className = 'hidden';
            profileTab.innerHTML = `
                <div class="modal-content" id="profileContent">
                    <div class="modal-header">
                        <h3 class="tab-header">Edit Profile</h3>
                    </div>
                    <div class="modal-scrollable">
                        <div class="create-post-form" style="max-width: 600px; margin: 0 auto;">
                            <div class="form-group" style="margin: var(--spacing-lg) 0;">
                                <label for="profilePictureUpload">Profile Picture</label>
                                <div class="file-upload-wrapper" style="margin-top: var(--spacing-md);">
                                    <input type="file" id="profilePictureUpload" accept="image/*" class="file-input" onchange="handleProfilePictureUpload(event)">
                                    <label for="profilePictureUpload" class="file-upload-button">
                                        📷 Choose Profile Picture
                                    </label>
                                </div>
                                <div id="currentProfilePicture" style="margin-top: var(--spacing-md);"></div>
                            </div>
                            <div class="form-group" style="margin: var(--spacing-lg) 0;">
                                <label for="profileBio">Bio</label>
                                <textarea id="profileBio" class="textarea" placeholder="Tell people about yourself..." style="min-height: 120px; margin-top: var(--spacing-md);"></textarea>
                            </div>
                            <div class="form-group" style="margin: var(--spacing-lg) 0;">
                                <label>Share Your Profile</label>
                                <p style="color: rgba(255, 255, 255, 0.7); font-size: var(--font-size-sm); margin-top: var(--spacing-sm);">
                                    Share this QR code with others to let them add you as a friend
                                </p>
                                <div id="profileQrCode" style="margin-top: var(--spacing-md); text-align: center; padding: var(--spacing-lg); background: rgba(255, 255, 255, 0.05); border-radius: var(--border-radius-lg);"></div>
                                <div style="display: flex; gap: var(--spacing-md); justify-content: center; margin-top: var(--spacing-md);">
                                </div>
                            </div>
                            <div style="display: flex; gap: var(--spacing-lg); justify-content: center; margin-top: var(--spacing-xl);">
                                <button class="btn btn-primary" onclick="saveProfile()">Save Profile</button>
                                <button class="btn btn-secondary" onclick="showFeed()">Cancel</button>
                            </div>
                        </div>
                    </div>
                </div>
            `;
            dashboard.appendChild(profileTab);
        }
    },

    async loadCurrentProfile() {
        if (!currentUser) return;

        const bioTextarea = document.getElementById('profileBio');
        if (bioTextarea && currentUser.bio) {
            bioTextarea.value = currentUser.bio;
        }

        // Generate QR code automatically
        await generateProfileQRCode();

        const currentPictureDiv = document.getElementById('currentProfilePicture');
        if (currentPictureDiv && currentUser.profilePicture) {
            currentPictureDiv.innerHTML = `
                <div style="text-align: center;">
                    <p style="margin-bottom: var(--spacing-sm);">Current Profile Picture:</p>
                    <img src="${currentUser.profilePicture}" alt="Profile Picture" style="width: 100px; height: 100px; border-radius: 50%; object-fit: cover; border: 2px solid var(--color-border);">
                </div>
            `;
        }
    },

    async uploadPicture(file) {
        if (!file || !currentUser) return;

        const base64Data = await Utils.fileToBase64(file);
        const base64Only = base64Data.split(',')[1];

        const updatedUser = await TauriAPI.invoke('upload_profile_picture', {
            userId: currentUser.id,
            fileData: base64Only,
            filename: file.name,
            fileType: file.type
        });

        currentUser = updatedUser;
        this.updatePictureDisplay(updatedUser.profilePicture);
        UI.showSuccess('dashboardError', 'Profile picture updated successfully!');
    },

    updatePictureDisplay(profilePicture) {
        const currentPictureDiv = document.getElementById('currentProfilePicture');
        if (currentPictureDiv && profilePicture) {
            currentPictureDiv.innerHTML = `
                <div style="text-align: center;">
                    <p style="margin-bottom: var(--spacing-sm);">Current Profile Picture:</p>
                    <img src="${profilePicture}" alt="Profile Picture" style="width: 100px; height: 100px; border-radius: 50%; object-fit: cover; border: 2px solid var(--color-border);">
                </div>
            `;
        }
    },

    async save() {
        if (!currentUser) {
            UI.showError('dashboardError', 'Please log in to update your profile');
            return;
        }

        const bioTextarea = document.getElementById('profileBio');
        const bio = bioTextarea ? bioTextarea.value.trim() : '';

        const updatedUser = await TauriAPI.invoke('update_user_profile', {
            userId: currentUser.id,
            bio: bio || null,
            profilePicture: null
        });

        currentUser = updatedUser;
        UI.showSuccess('dashboardError', 'Profile updated successfully!');
        showFeed();
    }
};

// Load functions
async function loadPosts() {
    try {
        if (!currentUser) return;
        const posts = await TauriAPI.invoke('get_all_posts', { userId: currentUser.id });
        const postsContainer = document.getElementById('posts');
        const postsStatusMessage = document.getElementById('postsStatusMessage');

        if (posts.length === 0) {
            postsContainer.innerHTML = '';
            postsStatusMessage.innerHTML = `
                <div style="text-align: center; padding: var(--spacing-3xl) var(--spacing-lg);">
                    <h2 style="color: var(--color-text-primary); margin-bottom: var(--spacing-lg); font-size: var(--font-size-2xl);">No Content Yet</h2>
                    <p style="color: var(--color-text-secondary); margin-bottom: var(--spacing-xl); font-size: var(--font-size-lg);">Start sharing your thoughts with the community</p>
                    <button class="btn btn-primary" onclick="showCreatePostPage()" style="max-width: 200px;">Create Post</button>
                </div>
            `;
        } else {
            postsStatusMessage.innerHTML = '';
            const postsWithMedia = await Promise.all(posts.map(async post => {
                const mediaAttachments = await PostManager.getMediaAttachments(post.id);
                return { ...post, mediaAttachments };
            }));

            postsContainer.innerHTML = postsWithMedia.map(post => `
                <div class="post glass-card hover-lift-md" data-post-id="${post.id}">
                    <div class="post-meta">
                        ${post.userId === currentUser.id ? currentUser.username : `User ${post.userId}`} • ${new Date(post.createdAt).toLocaleDateString()}
                    </div>
                    ${post.mediaAttachments && post.mediaAttachments.length > 0 ? `
                        <div class="post-media">
                            ${post.mediaAttachments.map(media => PostManager.createMediaPreview(media)).join('')}
                        </div>
                    ` : ''}
                    <div class="post-content">${Utils.escapeHtml(post.content)}</div>
                </div>
            `).join('');
        }

        setTimeout(() => UI.updateModalLayout(document.getElementById('postsContent')), 100);
    } catch (error) {
        UI.showError('dashboardError', 'Failed to load posts: ' + error);
    }
}

async function loadMessages() {
    if (!currentUser) return;

    try {
        const friends = await TauriAPI.invoke('get_friends', { userId: currentUser.id });
        allFriends = friends;
        setupFriendSearch();

        const messages = await TauriAPI.invoke('get_messages_for_user', { userId: currentUser.id });
        const voiceMessages = await TauriAPI.invoke('get_voice_messages', { userId: currentUser.id });

        // Auto-mark all messages as read when viewing messages tab
        // This marks all messages from all friends as read
        for (const friend of friends) {
            try {
                await TauriAPI.invoke('mark_conversation_as_read', {
                    userId: currentUser.id,
                    otherUserId: friend.friendUserId
                });
            } catch (error) {
                console.warn('Failed to mark conversation as read for friend', friend.friendUserId, error);
            }
        }
        const messagesContainer = document.getElementById('messages');

        if (messages.length === 0 && voiceMessages.length === 0) {
            messagesContainer.innerHTML = '<p class="text-center">No messages yet.</p>';
        } else {
            // Process messages to load reactions and read receipts for each
            const messagesWithReactions = await Promise.all(messages.map(async (message) => {
                try {
                    const reactions = await TauriAPI.invoke('get_message_reactions', { messageId: message.id });

                    // Check if message has been read (for messages sent by current user to recipient)
                    let isRead = false;
                    if (message.senderId === currentUser.id && message.recipientId !== currentUser.id) {
                        try {
                            isRead = await TauriAPI.invoke('is_message_read', {
                                messageId: message.id,
                                userId: message.recipientId
                            });
                        } catch (error) {
                            console.warn('Failed to check read status for message', message.id, error);
                        }
                    }

                    return { ...message, reactions: reactions || [], isRead };
                } catch (error) {
                    console.warn('Failed to load reactions for message', message.id, error);
                    return { ...message, reactions: [], isRead: false };
                }
            }));

            // Combine and sort messages and voice messages by timestamp
            const allMessages = [
                ...messagesWithReactions.map(msg => ({ ...msg, type: 'text' })),
                ...voiceMessages.map(msg => ({ ...msg, type: 'voice' }))
            ].sort((a, b) => new Date(b.createdAt) - new Date(a.createdAt));

            messagesContainer.innerHTML = allMessages.map(message => {
                if (message.type === 'voice') {
                    return `
                        <div class="post glass-card hover-lift-md voice-message-post" data-voice-id="${message.id}">
                            <div class="post-meta">
                                ${message.senderId === currentUser.id ? 'You' : `User ${message.senderId}`}
                                → ${message.recipientId === currentUser.id ? 'You' : `User ${message.recipientId}`}
                                • ${new Date(message.createdAt).toLocaleDateString()}
                                • 🔒 Encrypted Voice Message
                                ${message.threadId ? ` • 💬 Reply to Message ${message.threadId}` : ''}
                            </div>
                            <div class="post-content">
                                ${renderVoiceMessage(message)}
                            </div>
                        </div>
                    `;
                } else {
                    return `
                        <div class="post glass-card hover-lift-md" data-message-id="${message.id}">
                            <div class="post-meta">
                                ${message.senderId === currentUser.id ? 'You' : `User ${message.senderId}`}
                                → ${message.recipientId === currentUser.id ? 'You' : `User ${message.recipientId}`}
                                • ${new Date(message.createdAt).toLocaleDateString()}
                                ${message.encrypted ? ' • 🔒 Encrypted' : ''}
                                ${message.threadId ? ` • 💬 Reply to Message ${message.threadId}` : ''}
                                ${message.senderId === currentUser.id && message.isRead ? ' • <span style="color: var(--color-success)">✓✓ Read</span>' : ''}
                            </div>
                            <div class="post-content message-content">
                                ${message.encrypted ? '[Encrypted Message - Click to decrypt]' : Utils.escapeHtml(message.content)}
                            </div>
                            <div class="message-actions">
                                <div class="message-reactions">
                                    ${renderMessageReactions(message.reactions)}
                                    <button class="btn-reaction" onclick="addReaction(${message.id})">😊</button>
                                    <button class="btn-reaction" onclick="addReaction(${message.id}, '❤️')">❤️</button>
                                    <button class="btn-reaction" onclick="addReaction(${message.id}, '👍')">👍</button>
                                    <button class="btn-reaction" onclick="addReaction(${message.id}, '👎')">👎</button>
                                    <button class="btn-reaction" onclick="addReaction(${message.id}, '😂')">😂</button>
                                </div>
                                <div class="message-thread-actions">
                                    <button class="btn-secondary btn-small" onclick="replyToMessage(${message.id})">💬 Reply</button>
                                    ${message.threadId ? `<button class="btn-secondary btn-small" onclick="viewThread(${message.threadId || message.id})">🧵 View Thread</button>` : ''}
                                    ${message.senderId === currentUser.id ? `
                                        <button class="message-action-btn edit" onclick="editMessage(${message.id})">✏️ Edit</button>
                                        <button class="message-action-btn delete" onclick="deleteMessage(${message.id})">🗑️ Delete</button>
                                    ` : ''}
                                </div>
                            </div>
                        </div>
                    `;
                }
            }).join('');
        }

        setTimeout(() => UI.updateModalLayout(document.getElementById('messagesContent')), 100);
    } catch (error) {
        UI.showError('dashboardError', 'Failed to load messages: ' + error);
    }
}

// Message reactions and threading functions
function renderMessageReactions(reactions) {
    if (!reactions || reactions.length === 0) return '';

    // Group reactions by emoji
    const reactionGroups = reactions.reduce((groups, reaction) => {
        if (!groups[reaction.emoji]) {
            groups[reaction.emoji] = [];
        }
        groups[reaction.emoji].push(reaction);
        return groups;
    }, {});

    return Object.entries(reactionGroups).map(([emoji, reactionList]) => {
        const count = reactionList.length;
        const userIds = reactionList.map(r => r.userId);
        const hasCurrentUser = userIds.includes(currentUser?.id);
        return `<span class="reaction-badge ${hasCurrentUser ? 'user-reacted' : ''}" title="Reacted by ${userIds.join(', ')}">${emoji} ${count}</span>`;
    }).join(' ');
}

async function addReaction(messageId, emoji = '😊') {
    if (!currentUser) return;

    try {
        await TauriAPI.invoke('add_message_reaction', {
            messageId,
            userId: currentUser.id,
            emoji
        });

        // Reload messages to show updated reactions
        await loadMessages();
    } catch (error) {
        console.error('Failed to add reaction:', error);
        UI.showError('dashboardError', 'Failed to add reaction: ' + error);
    }
}

async function replyToMessage(messageId) {
    if (!currentUser) return;

    // Show reply form with thread context
    const selectedRecipientDiv = document.getElementById('selectedRecipient');
    const messageContent = document.getElementById('messageContent');

    // Find the original message to get recipient info
    const messageDivs = document.querySelectorAll('[data-message-id]');
    let originalMessage = null;

    for (const div of messageDivs) {
        if (div.getAttribute('data-message-id') == messageId) {
            originalMessage = div;
            break;
        }
    }

    if (originalMessage) {
        // Pre-fill reply context
        messageContent.placeholder = `Replying to message ${messageId}...`;
        messageContent.focus();

        // Store the thread context
        messageContent.setAttribute('data-reply-to', messageId);
    }
}

async function viewThread(threadId) {
    if (!currentUser) return;

    try {
        const threadMessages = await TauriAPI.invoke('get_message_thread', { threadId });

        // Display thread in a modal or expanded view
        const threadHtml = threadMessages.map(message => `
            <div class="thread-message">
                <div class="post-meta">
                    ${message.senderId === currentUser.id ? 'You' : `User ${message.senderId}`}
                    • ${new Date(message.createdAt).toLocaleDateString()}
                    ${message.encrypted ? ' • 🔒 Encrypted' : ''}
                </div>
                <div class="post-content">
                    ${message.encrypted ? '[Encrypted Message - Click to decrypt]' : Utils.escapeHtml(message.content)}
                </div>
            </div>
        `).join('');

        // Show thread in messages area temporarily
        const messagesContainer = document.getElementById('messages');
        messagesContainer.innerHTML = `
            <div class="thread-view">
                <div class="thread-header">
                    <h3>🧵 Message Thread</h3>
                    <button class="btn-secondary" onclick="loadMessages()">← Back to Messages</button>
                </div>
                ${threadHtml}
            </div>
        `;
    } catch (error) {
        console.error('Failed to load thread:', error);
        UI.showError('dashboardError', 'Failed to load thread: ' + error);
    }
}

// Voice message functions
let mediaRecorder = null;
let audioChunks = [];
let isRecording = false;

async function startVoiceRecording() {
    try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        mediaRecorder = new MediaRecorder(stream);
        audioChunks = [];

        mediaRecorder.ondataavailable = (event) => {
            audioChunks.push(event.data);
        };

        mediaRecorder.onstop = async () => {
            const audioBlob = new Blob(audioChunks, { type: 'audio/wav' });
            await processVoiceMessage(audioBlob);

            // Stop all tracks to release microphone
            stream.getTracks().forEach(track => track.stop());
        };

        mediaRecorder.start();
        isRecording = true;

        // Update UI
        const recordButton = document.getElementById('voiceRecordButton');
        if (recordButton) {
            recordButton.textContent = '🛑 Stop Recording';
            recordButton.onclick = stopVoiceRecording;
            recordButton.classList.add('recording');
        }

        console.log('Voice recording started...');
    } catch (error) {
        console.error('Error starting voice recording:', error);
        UI.showError('dashboardError', 'Failed to start voice recording: ' + error.message);
    }
}

function stopVoiceRecording() {
    if (mediaRecorder && isRecording) {
        mediaRecorder.stop();
        isRecording = false;

        // Update UI
        const recordButton = document.getElementById('voiceRecordButton');
        if (recordButton) {
            recordButton.textContent = '🎤 Record Voice Message';
            recordButton.onclick = startVoiceRecording;
            recordButton.classList.remove('recording');
        }

        console.log('Voice recording stopped...');
    }
}

async function processVoiceMessage(audioBlob) {
    if (!selectedRecipient) {
        UI.showError('dashboardError', 'Please select a recipient for the voice message');
        return;
    }

    try {
        // Convert blob to base64
        const reader = new FileReader();
        reader.onload = async () => {
            const base64Audio = reader.result.split(',')[1]; // Remove data URL prefix

            // Calculate duration (approximate)
            const duration = audioBlob.size / (16000 * 2); // Rough estimate

            // Generate simple waveform data (placeholder)
            const waveform = generateWaveformData(audioBlob);

            // Check for thread context
            const messageContentInput = document.getElementById('messageContent');
            const replyToId = messageContentInput?.getAttribute('data-reply-to');

            await TauriAPI.invoke('send_voice_message', {
                senderId: currentUser.id,
                recipientId: selectedRecipient.id,
                audioData: base64Audio,
                durationSeconds: duration,
                waveform: waveform,
                threadId: replyToId ? parseInt(replyToId) : null
            });

            UI.showSuccess('dashboardError', 'Voice message sent successfully!');
            await loadMessages();
            await loadVoiceMessages();

            // Clear reply context if it exists
            if (messageContentInput) {
                messageContentInput.removeAttribute('data-reply-to');
                messageContentInput.placeholder = 'Enter your message';
            }
        };

        reader.readAsDataURL(audioBlob);
    } catch (error) {
        console.error('Error processing voice message:', error);
        UI.showError('dashboardError', 'Failed to send voice message: ' + error);
    }
}

function generateWaveformData(audioBlob) {
    // Simple placeholder waveform generation
    // In a real implementation, you'd analyze the audio to generate actual waveform data
    const length = Math.min(100, Math.max(20, Math.floor(audioBlob.size / 1000)));
    const waveform = [];
    for (let i = 0; i < length; i++) {
        waveform.push(Math.random() * 0.8 + 0.1); // Random values between 0.1 and 0.9
    }
    return JSON.stringify(waveform);
}

async function loadVoiceMessages() {
    if (!currentUser) return;

    try {
        const voiceMessages = await TauriAPI.invoke('get_voice_messages', { userId: currentUser.id });

        // Voice messages will be integrated into the main messages display
        // This function can be used to specifically load voice messages if needed
        console.log('Voice messages loaded:', voiceMessages.length);

        return voiceMessages;
    } catch (error) {
        console.error('Failed to load voice messages:', error);
        return [];
    }
}

function playVoiceMessage(audioData) {
    try {
        const audio = new Audio('data:audio/wav;base64,' + audioData);
        audio.play();
    } catch (error) {
        console.error('Error playing voice message:', error);
        UI.showError('dashboardError', 'Failed to play voice message');
    }
}

async function deleteVoiceMessage(voiceMessageId) {
    if (!currentUser) return;

    try {
        await TauriAPI.invoke('delete_voice_message', {
            voiceMessageId,
            userId: currentUser.id
        });

        UI.showSuccess('dashboardError', 'Voice message deleted successfully!');
        await loadMessages();
        await loadVoiceMessages();
    } catch (error) {
        console.error('Error deleting voice message:', error);
        UI.showError('dashboardError', 'Failed to delete voice message: ' + error);
    }
}

function renderVoiceMessage(voiceMessage) {
    const duration = Math.floor(voiceMessage.durationSeconds);
    const minutes = Math.floor(duration / 60);
    const seconds = duration % 60;
    const durationText = `${minutes}:${seconds.toString().padStart(2, '0')}`;

    return `
        <div class="voice-message" data-voice-id="${voiceMessage.id}">
            <div class="voice-message-header">
                <span class="voice-duration">🎵 Voice Message (${durationText})</span>
                ${voiceMessage.senderId === currentUser?.id ?
                    `<button class="btn-small btn-secondary" onclick="deleteVoiceMessage(${voiceMessage.id})">🗑️ Delete</button>` :
                    ''
                }
            </div>
            <div class="voice-message-controls">
                <button class="btn-play" onclick="playVoiceMessage('${voiceMessage.audioData}')">▶️ Play</button>
                ${voiceMessage.waveform ? `<div class="waveform">${renderWaveform(voiceMessage.waveform)}</div>` : ''}
            </div>
        </div>
    `;
}

function renderWaveform(waveformData) {
    try {
        const waveform = JSON.parse(waveformData);
        return waveform.map(amplitude =>
            `<div class="waveform-bar" style="height: ${amplitude * 100}%"></div>`
        ).join('');
    } catch (error) {
        return '<div class="waveform-error">Waveform unavailable</div>';
    }
}

async function loadFriends() {
    if (!currentUser) return;

    try {
        const friends = await TauriAPI.invoke('get_friends', { userId: currentUser.id });
        const friendsContainer = document.getElementById('friends');

        if (friends.length === 0) {
            friendsContainer.innerHTML = '<p class="text-center">No friends yet. Add some friends to start connecting!</p>';
        } else {
            friendsContainer.innerHTML = friends.map(friend => `
                <div class="post glass-card hover-lift-md">
                    <div class="post-meta">
                        ${Utils.escapeHtml(friend.friendUsername)} • Added ${new Date(friend.createdAt).toLocaleDateString()}
                    </div>
                    <div class="post-content">
                        <strong>${Utils.escapeHtml(friend.friendUsername)}</strong>
                        <br><small class="public-key-display" style="margin-top: var(--spacing-xs); padding: var(--spacing-xs); font-size: 0.75rem;">Public Key: ${friend.publicKey ? friend.publicKey.substring(0, 32) + '...' : 'None'}</small>
                    </div>
                </div>
            `).join('');
        }

        await generateMyQRCode();
        setTimeout(() => UI.updateModalLayout(document.getElementById('friendsContent')), 100);
    } catch (error) {
        UI.showError('dashboardError', 'Failed to load friends: ' + error);
    }
}


// Create post functions
function showCreatePost() {
    const postsStatusMessage = document.getElementById('postsStatusMessage');
    postsStatusMessage.innerHTML = `
        <div class="create-post-form">
            <h3>Create Post</h3>
            <textarea id="postContent" class="textarea" placeholder="What's on your mind?" required style="margin: var(--spacing-md) 0;"></textarea>
            <div class="form-group" style="margin: var(--spacing-md) 0;">
                <label for="postAttachments">Attachments (optional)</label>
                <input type="file" id="postAttachments" multiple accept="image/*,video/*,audio/*,.pdf,.txt,.doc,.docx" style="margin-top: var(--spacing-xs);">
                <small style="color: rgba(255, 255, 255, 0.7); display: block; margin-top: var(--spacing-xs);">Select images, videos, documents, or other files to attach</small>
            </div>
            <div style="display: flex; gap: var(--spacing-md); justify-content: center;">
                <button class="btn btn-primary" onclick="createPost()">Share Post</button>
                <button class="btn btn-secondary" onclick="cancelCreatePost()">Cancel</button>
            </div>
        </div>
    `;
    document.getElementById('postContent').focus();
}

async function createPost() {
    const content = document.getElementById('postContent').value.trim();
    if (!content) {
        alert('Please enter some content for your post');
        return;
    }

    try {
        const fileInput = document.getElementById('postAttachments');
        await PostManager.create(content, fileInput ? fileInput.files : null);
        await loadPosts();
        document.getElementById('postsStatusMessage').innerHTML = '';
    } catch (error) {
        alert('Failed to create post: ' + error);
    }
}

function cancelCreatePost() {
    loadPosts();
}

async function createPostFromPage() {
    const content = document.getElementById('createPostTextarea').value.trim();
    const fileInput = document.getElementById('createPostAttachments');
    const hasFiles = fileInput && fileInput.files && fileInput.files.length > 0;

    // Require either text content OR attachments (or both)
    if (!content && !hasFiles) {
        alert('Please enter some text or attach an image');
        return;
    }

    // Disable the share button to prevent double-clicks
    const shareButton = document.querySelector('button[onclick="createPostFromPage()"]');
    if (shareButton) {
        shareButton.disabled = true;
        shareButton.textContent = 'Creating post...';
    }

    try {
        console.log('Creating post from page...');

        // Add timeout for the entire post creation process
        // Use empty string if no content (image-only post)
        const createPostPromise = PostManager.create(content || '', fileInput ? fileInput.files : null);
        const timeoutPromise = new Promise((_, reject) =>
            setTimeout(() => reject(new Error('Post creation timed out. File may be too large.')), 30000)
        );

        await Promise.race([createPostPromise, timeoutPromise]);

        // Clear the form
        document.getElementById('createPostTextarea').value = '';
        document.getElementById('createPostAttachments').value = '';
        document.getElementById('fileCount').textContent = '';

        console.log('Post created, navigating to feed...');
        // Reload posts to show the new post
        await loadPosts();
        // Navigate to feed
        showPosts();
        console.log('Navigation complete');
    } catch (error) {
        console.error('Failed to create post:', error);
        alert('Failed to create post: ' + (error.message || error));
    } finally {
        // Re-enable the share button
        if (shareButton) {
            shareButton.disabled = false;
            shareButton.textContent = 'Share Post';
        }
    }
}

// Friend search and messaging
function setupFriendSearch() {
    const searchInput = document.getElementById('friendSearch');
    const resultsContainer = document.getElementById('friendSearchResults');

    if (!searchInput || !resultsContainer) return;

    searchInput.addEventListener('input', function() {
        const query = this.value.toLowerCase().trim();

        if (query === '') {
            resultsContainer.classList.add('hidden');
            return;
        }

        const filteredFriends = allFriends.filter(friend =>
            friend.friendUsername.toLowerCase().includes(query)
        );

        if (filteredFriends.length === 0) {
            resultsContainer.innerHTML = '<div class="friend-search-item">No friends found</div>';
        } else {
            resultsContainer.innerHTML = filteredFriends.map(friend => `
                <div class="friend-search-item" onclick="selectFriend(${friend.id}, '${Utils.escapeHtml(friend.friendUsername)}')">
                    <div class="friend-info">
                        <div class="friend-avatar">${friend.friendUsername.charAt(0).toUpperCase()}</div>
                        <span class="friend-name">${Utils.escapeHtml(friend.friendUsername)}</span>
                    </div>
                </div>
            `).join('');
        }

        resultsContainer.classList.remove('hidden');
    });

    document.addEventListener('click', function(event) {
        if (!event.target.closest('#friendSearch') && !event.target.closest('#friendSearchResults')) {
            resultsContainer.classList.add('hidden');
        }
    });
}

function selectFriend(friendId, friendUsername) {
    selectedRecipient = { id: friendId, username: friendUsername };

    const selectedContainer = document.getElementById('selectedRecipient');
    selectedContainer.innerHTML = `
        <div class="friend-info">
            <div class="friend-avatar">${friendUsername.charAt(0).toUpperCase()}</div>
            <span class="friend-name">${Utils.escapeHtml(friendUsername)}</span>
        </div>
    `;

    document.getElementById('friendSearch').value = '';
    document.getElementById('friendSearchResults').classList.add('hidden');
}

async function sendMessage() {
    if (!currentUser) return;

    const messageContentInput = document.getElementById('messageContent');
    const content = messageContentInput.value;
    const replyToId = messageContentInput.getAttribute('data-reply-to');

    if (!selectedRecipient || !content) {
        UI.showError('dashboardError', 'Please select a recipient and enter a message');
        return;
    }

    try {
        if (replyToId) {
            // Send as a reply using the reply_to_message command
            await TauriAPI.invoke('reply_to_message', {
                originalMessageId: parseInt(replyToId),
                senderId: currentUser.id,
                recipientId: selectedRecipient.id,
                content: content
            });
        } else {
            // Send as a regular message
            await TauriAPI.invoke('send_encrypted_message', {
                senderId: currentUser.id,
                recipientId: selectedRecipient.id,
                content: content
            });
        }

        messageContentInput.value = '';
        messageContentInput.placeholder = 'Enter your message';
        messageContentInput.removeAttribute('data-reply-to');
        clearSelectedRecipient();
        UI.showSuccess('dashboardError', replyToId ? 'Reply sent successfully!' : 'Encrypted message sent successfully!');
        loadMessages();
    } catch (error) {
        UI.showError('dashboardError', 'Failed to send message: ' + error);
    }
}

function clearSelectedRecipient() {
    selectedRecipient = null;
    const selectedContainer = document.getElementById('selectedRecipient');
    selectedContainer.innerHTML = '<span class="no-selection">No friend selected</span>';
}

// Friend management
async function addFriendByPublicKey() {
    console.log('[ADD_FRIEND] Function called');
    if (!currentUser) {
        console.log('[ADD_FRIEND] No current user');
        return;
    }

    const friendPublicKey = document.getElementById('friendPublicKey').value.trim();
    console.log('[ADD_FRIEND] Public key input:', friendPublicKey);

    if (!friendPublicKey) {
        console.log('[ADD_FRIEND] No public key entered');
        UI.showError('dashboardError', 'Please enter a valid public key');
        return;
    }

    if (friendPublicKey === currentUser.publicKey) {
        console.log('[ADD_FRIEND] Cannot add self');
        UI.showError('dashboardError', 'You cannot add yourself as a friend');
        return;
    }

    try {
        console.log('[ADD_FRIEND] Calling add_friend_from_qr_code command');
        // Use add_friend_from_qr_code which creates peer users if they don't exist
        // We use a default username that can be overridden when the users sync
        const friend = await TauriAPI.invoke('add_friend_from_qr_code', {
            currentUserId: currentUser.id,
            qrData: {
                username: `User_${friendPublicKey.substring(0, 8)}`,  // Temporary username from key prefix
                publicKey: friendPublicKey
            }
        });

        console.log('[ADD_FRIEND] Friend added successfully:', friend);
        document.getElementById('friendPublicKey').value = '';
        UI.showSuccess('dashboardError', `Successfully added ${friend.username} as a friend!`);
        loadFriends();
    } catch (error) {
        console.log('[ADD_FRIEND] Error:', error);
        UI.showError('dashboardError', 'Failed to add friend: ' + error);
    }
}

async function addFriendFromTab() {
    console.log('[ADD_FRIEND_TAB] Function called');
    if (!currentUser) {
        console.log('[ADD_FRIEND_TAB] No current user');
        return;
    }

    const friendPublicKey = document.getElementById('addFriendPublicKey').value.trim();
    console.log('[ADD_FRIEND_TAB] Public key input:', friendPublicKey);

    if (!friendPublicKey) {
        console.log('[ADD_FRIEND_TAB] No public key entered');
        UI.showError('dashboardError', 'Please enter a valid public key');
        return;
    }

    if (friendPublicKey === currentUser.publicKey) {
        console.log('[ADD_FRIEND_TAB] Cannot add self');
        UI.showError('dashboardError', 'You cannot add yourself as a friend');
        return;
    }

    try {
        console.log('[ADD_FRIEND_TAB] Calling add_friend_from_qr_code command');
        const friend = await TauriAPI.invoke('add_friend_from_qr_code', {
            currentUserId: currentUser.id,
            qrData: {
                username: `User_${friendPublicKey.substring(0, 8)}`,
                publicKey: friendPublicKey
            }
        });

        console.log('[ADD_FRIEND_TAB] Friend added successfully:', friend);
        document.getElementById('addFriendPublicKey').value = '';
        UI.showSuccess('dashboardError', `Successfully added ${friend.username} as a friend!`);
        showFriends();
    } catch (error) {
        console.log('[ADD_FRIEND_TAB] Error:', error);
        UI.showError('dashboardError', 'Failed to add friend: ' + error);
    }
}

async function copyPublicKey() {
    const publicKey = document.getElementById('userPublicKey').textContent;
    try {
        await navigator.clipboard.writeText(publicKey);

        const copyBtn = document.querySelector('.btn-copy');
        const originalText = copyBtn.textContent;
        copyBtn.textContent = 'Copied!';
        copyBtn.style.background = 'var(--color-success-light)';

        setTimeout(() => {
            copyBtn.textContent = originalText;
            copyBtn.style.background = 'var(--glass-regular)';
        }, 2000);
    } catch (error) {
        UI.showError('dashboardError', 'Failed to copy public key: ' + error);
    }
}

// QR Code functions
async function generateQRCode(containerId, options = {}) {
    if (!currentUser) return;

    const { maxWidth = '200px', showSuccess = false } = options;

    try {
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('🔵 FRONTEND: QR CODE GENERATION STARTED');
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('[QR-GEN] Calling iroh_generate_invite (includes NodeId + addresses)...');
        // Use Iroh invite system (includes NodeId + relay URLs + direct addresses)
        const inviteCode = await TauriAPI.invoke('iroh_generate_invite');
        console.log('[QR-GEN] ✓ Invite code received from backend');
        console.log('[QR-GEN]   Length:', inviteCode.length, 'chars');
        console.log('[QR-GEN]   First 30 chars:', inviteCode.substring(0, 30) + '...');

        // Generate QR code from the invite
        console.log('[QR-GEN] Generating QR code image...');
        const qrCodeDataUrl = await TauriAPI.invoke('generate_qr_code', { data: inviteCode });
        console.log('[QR-GEN] ✓ QR code image generated');
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('✅ FRONTEND: QR CODE GENERATION COMPLETE');
        console.log('═══════════════════════════════════════════════════════════════');

        const qrContainer = document.getElementById(containerId);
        if (qrContainer) {
            qrContainer.innerHTML = `<img src="${qrCodeDataUrl}" alt="Your QR Code" style="max-width: ${maxWidth}; max-height: ${maxWidth}; border-radius: var(--border-radius-md);">`;
        }

        if (showSuccess) {
            UI.showSuccess('dashboardError', 'QR code generated successfully!');
        }
    } catch (error) {
        console.error('[QR] Failed to generate QR code:', error);
        if (showSuccess) {
            UI.showError('dashboardError', 'Failed to generate QR code: ' + error);
        } else {
            console.error('Failed to generate QR code:', error);
        }
    }
}

// Convenience wrappers for backward compatibility
async function generateMyQRCode() {
    console.log('[QR-GEN] generateMyQRCode() called');

    if (!currentUser) {
        console.log('[QR-GEN] No current user, returning early');
        return;
    }

    console.log('[QR-GEN] Current user:', currentUser.username);
    console.log('[QR-GEN] Checking P2P object existence...');
    console.log('[QR-GEN] P2P exists:', typeof P2P !== 'undefined');
    console.log('[QR-GEN] P2P.initialized:', P2P?.initialized);
    console.log('[QR-GEN] P2P.generateInvite exists:', typeof P2P?.generateInvite === 'function');

    try {
        console.log('[QR-GEN] Calling P2P.generateInvite()...');
        // Generate Iroh invite (already formatted as cipher://add-friend?key=...)
        // NO FALLBACK - we require Iroh for proper peer connectivity
        const inviteCode = await P2P.generateInvite();
        console.log('[QR-GEN] Invite code received, length:', inviteCode?.length);

        // inviteCode is already in cipher:// format, no need to wrap it
        const qrData = inviteCode;
        console.log('[QR-GEN] QR data prepared, calling generate_qr_code...');

        const qrCodeDataUrl = await TauriAPI.invoke('generate_qr_code', { data: qrData });
        console.log('[QR-GEN] QR code image generated successfully');

        const qrContainer = document.getElementById('myQrCode');
        if (qrContainer) {
            qrContainer.innerHTML = `<img src="${qrCodeDataUrl}" alt="Your QR Code" style="width: 90%; height: auto; max-width: 500px; border-radius: var(--border-radius-md);">`;
            console.log('[QR-GEN] QR code displayed successfully');
        }
    } catch (error) {
        console.error('[QR-GEN] ERROR generating QR code:', error);
        console.error('[QR-GEN] Error name:', error?.name);
        console.error('[QR-GEN] Error message:', error?.message);
        console.error('[QR-GEN] Error stack:', error?.stack);

        const qrContainer = document.getElementById('myQrCode');
        if (qrContainer) {
            qrContainer.innerHTML = '<p style="color: var(--color-error);">P2P system not ready. Please wait a moment and try again.</p>';
            console.log('[QR-GEN] Error message displayed to user');
        }
    }
}

async function generateProfileQRCode() {
    await generateQRCode('profileQrCode', { maxWidth: '250px' });
}

async function scanQRCode() {
    console.log('[QR] scanQRCode called');
    const platform = await TauriAPI.invoke('get_platform');
    console.log('[QR] Platform:', platform);

    if (platform === 'android' || platform === 'ios') {
        // Use camera scanner on mobile via Tauri barcode-scanner plugin
        try {
            console.log('[QR] Starting camera scan via barcode-scanner plugin...');

            // Use the correct Tauri plugin API
            const result = await window.__TAURI__.invoke('plugin:barcode-scanner|scan', {
                windowed: true,
                formats: ['QR_CODE']
            });
            console.log('[QR] Scan result:', result);

            if (result && result.content) {
                console.log('[QR] QR code scanned, content:', result.content);

                // Extract public key and optional node info from cipher:// URI
                let publicKey = result.content;
                let nodeId = null;
                let relayUrl = null;
                if (result.content.startsWith('cipher://add-friend?key=')) {
                    const url = new URL(result.content);
                    publicKey = url.searchParams.get('key');
                    nodeId = url.searchParams.get('node');
                    const encodedRelay = url.searchParams.get('relay');
                    if (encodedRelay) {
                        relayUrl = decodeURIComponent(encodedRelay);
                    }
                    if (nodeId && relayUrl) {
                        console.log('[QR] Extracted public key and node info from URI');
                        console.log('[QR]   Public key:', publicKey);
                        console.log('[QR]   NodeId:', nodeId);
                        console.log('[QR]   Relay:', relayUrl);
                    } else {
                        console.log('[QR] Extracted public key from URI (no node info):', publicKey);
                    }
                } else {
                    console.log('[QR] Using raw public key (no node info)');
                }

                // Add friend by public key with optional node info - single function call
                console.log('[QR] Adding friend by public key...');
                try {
                    const addedPublicKey = await TauriAPI.invoke('iroh_add_friend_by_public_key', {
                        friendPublicKey: publicKey,
                        nodeId: nodeId,
                        relayUrl: relayUrl
                    });
                    console.log('[QR] ✓ Friend added successfully:', addedPublicKey);

                    UI.showSuccess('dashboardError', 'Friend added successfully!');

                    // Reload friends list to show the new friend
                    console.log('[QR] Reloading friends list...');
                    await new Promise(resolve => setTimeout(resolve, 100));
                    if (typeof loadFriends === 'function') {
                        await loadFriends();
                        console.log('[QR] Friends list reloaded');
                    }
                } catch (error) {
                    console.error('[QR] Failed to add friend:', error);
                    UI.showError('dashboardError', 'Failed to add friend: ' + error);
                }
            } else {
                console.log('[QR] Scan cancelled or no result');
            }
        } catch (error) {
            console.error('[QR] Camera scanner error:', error);
            console.error('[QR] Error details:', JSON.stringify(error));

            if (error && (error.toString().includes('permission') || error.toString().includes('Permission'))) {
                UI.showError('dashboardError', 'Camera permission denied. Please enable camera access in your device settings.');
            } else {
                UI.showError('dashboardError', 'Failed to scan QR code: ' + error);
            }
        }
    } else {
        // Fall back to file picker on desktop
        const fileInput = document.getElementById('qrCodeFile');
        fileInput.click();
    }
}

async function handleQRCodeFile(event) {
    const file = event.target.files[0];
    if (!file) return;

    try {
        const base64Data = await Utils.fileToBase64(file);
        const qrCodeData = await TauriAPI.invoke('scan_qr_code_from_image', { base64Image: base64Data });

        if (qrCodeData && qrCodeData.username && qrCodeData.publicKey) {
            document.getElementById('friendPublicKey').value = qrCodeData.publicKey;
            await addFriendByQRCode(qrCodeData.username, qrCodeData.publicKey);
        } else {
            UI.showError('dashboardError', 'Invalid QR code or QR code does not contain friend data');
        }
    } catch (error) {
        UI.showError('dashboardError', 'Failed to scan QR code: ' + error);
    }

    event.target.value = '';
}

async function addFriendByQRCode(username, publicKey, peerId, peerAddr) {
    if (!currentUser) return;

    if (publicKey === currentUser.publicKey) {
        UI.showError('dashboardError', 'You cannot add yourself as a friend');
        return;
    }

    try {
        const friend = await TauriAPI.invoke('get_user_by_public_key', { publicKey: publicKey });

        if (!friend) {
            UI.showError('dashboardError', `No user found with username ${username}`);
            return;
        }

        await TauriAPI.invoke('add_friend', {
            userId: currentUser.id,
            friendUserId: friend.id
        });

        UI.showSuccess('dashboardError', `Successfully added ${username} as a friend!`);
        loadFriends();
    } catch (error) {
        UI.showError('dashboardError', 'Failed to add friend: ' + error);
    }
}

// Profile functions
async function handleProfilePictureUpload(event) {
    try {
        const file = event.target.files[0];
        await ProfileManager.uploadPicture(file);
    } catch (error) {
        console.error('Error uploading profile picture:', error);
        UI.showError('dashboardError', 'Failed to upload profile picture: ' + error);
    }
}

async function saveProfile() {
    try {
        await ProfileManager.save();
    } catch (error) {
        console.error('Error updating profile:', error);
        UI.showError('dashboardError', 'Failed to update profile: ' + error);
    }
}

// Media functions
async function viewMediaAttachment(mediaId) {
    await PostManager.viewMedia(mediaId);
}

// Enhanced Friend Management Functions
const FriendManager = {
    async createInvite(uses = 5, hours = 24) {
        try {
            const invite = await tauriInvoke('create_friend_invite', {
                userId: currentUser.id,
                uses: uses,
                hoursValid: hours
            });

            // Show invite code to user
            const inviteText = `Friend Invite Code: ${invite.inviteCode}\n\nExpires: ${new Date(invite.expiresAt).toLocaleString()}\nRemaining uses: ${invite.usesRemaining}`;

            // Copy to clipboard
            if (navigator.clipboard) {
                await navigator.clipboard.writeText(invite.inviteCode);
                UI.showSuccess('dashboardError', `Invite code created and copied to clipboard!\n\nCode: ${invite.inviteCode}\nExpires: ${new Date(invite.expiresAt).toLocaleString()}`);
            } else {
                UI.showSuccess('dashboardError', inviteText);
            }

            return invite;
        } catch (error) {
            console.error('Error creating invite:', error);
            UI.showError('dashboardError', 'Failed to create invite: ' + error);
            throw error;
        }
    },

    async useInvite(inviteCode) {
        try {
            const friend = await tauriInvoke('use_friend_invite', {
                userId: currentUser.id,
                inviteCode: inviteCode.trim().toUpperCase()
            });

            UI.showSuccess('dashboardError', `Successfully added ${friend.username} as a friend!`);
            await loadFriends(); // Refresh friends list
            return friend;
        } catch (error) {
            console.error('Error using invite:', error);
            UI.showError('dashboardError', 'Failed to use invite: ' + error);
            throw error;
        }
    },

    async exportFriends() {
        try {
            const friendsList = await tauriInvoke('export_friends_list', {
                userId: currentUser.id
            });

            const exportData = JSON.stringify(friendsList, null, 2);

            // Create download
            const blob = new Blob([exportData], { type: 'application/json' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `cipher-friends-${new Date().toISOString().split('T')[0]}.json`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);

            UI.showSuccess('dashboardError', `Exported ${friendsList.length} friends to file`);
            return friendsList;
        } catch (error) {
            console.error('Error exporting friends:', error);
            UI.showError('dashboardError', 'Failed to export friends: ' + error);
            throw error;
        }
    },

    async importFriends(jsonString) {
        try {
            const result = await tauriInvoke('import_friends_list', {
                userId: currentUser.id,
                friendsJson: jsonString
            });

            let message = `Import Results:\n`;
            if (result.added.length > 0) {
                message += `Added: ${result.added.join(', ')}\n`;
            }
            if (result.skipped.length > 0) {
                message += `Skipped: ${result.skipped.join(', ')}\n`;
            }
            if (result.errors.length > 0) {
                message += `Errors: ${result.errors.join(', ')}\n`;
            }

            UI.showSuccess('dashboardError', message);
            await loadFriends(); // Refresh friends list
            return result;
        } catch (error) {
            console.error('Error importing friends:', error);
            UI.showError('dashboardError', 'Failed to import friends: ' + error);
            throw error;
        }
    },

    async getRecentContacts(limit = 10) {
        try {
            const contacts = await tauriInvoke('get_recent_contacts', {
                userId: currentUser.id,
                limit: limit
            });
            return contacts;
        } catch (error) {
            console.error('Error getting recent contacts:', error);
            return [];
        }
    },

    async updateRecentContact(contactUserId) {
        try {
            await tauriInvoke('update_recent_contact', {
                userId: currentUser.id,
                contactUserId: contactUserId
            });
        } catch (error) {
            console.error('Error updating recent contact:', error);
        }
    }
};

// Friend management UI functions
async function createFriendInvite() {
    try {
        const hours = prompt('How many hours should this invite be valid? (default: 24)', '24');
        const uses = prompt('How many times can this invite be used? (default: 5)', '5');

        if (hours && uses) {
            await FriendManager.createInvite(parseInt(uses), parseInt(hours));
        }
    } catch (error) {
        console.error('Error in createFriendInvite:', error);
    }
}

async function useFriendInvite() {
    try {
        const inviteCode = prompt('Enter the friend invite code:');
        if (inviteCode) {
            await FriendManager.useInvite(inviteCode);
        }
    } catch (error) {
        console.error('Error in useFriendInvite:', error);
    }
}

async function exportFriendsList() {
    try {
        await FriendManager.exportFriends();
    } catch (error) {
        console.error('Error in exportFriendsList:', error);
    }
}

async function importFriendsList() {
    try {
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = '.json';
        input.onchange = async (e) => {
            const file = e.target.files[0];
            if (file) {
                const text = await file.text();
                await FriendManager.importFriends(text);
            }
        };
        input.click();
    } catch (error) {
        console.error('Error in importFriendsList:', error);
    }
}

// Initialize app
document.addEventListener('DOMContentLoaded', async () => {
    try {
        console.log('[INIT] DOMContentLoaded - Setting up event listeners');

        // Initialize navbar first
        if (typeof Navbar !== 'undefined') {
            console.log('[INIT] Initializing navbar component');
            Navbar.init('navbarContainer');
        }

        console.log('[INIT] Login form element:', document.getElementById('loginForm'));
        console.log('[INIT] Dashboard element:', document.getElementById('dashboard'));

        console.log('[INIT] Waiting for Tauri API...');
        await TauriAPI.waitForAPI();
        console.log('[INIT] Tauri API ready');

        console.log('[INIT] Attempting auto-login...');
        const autoLoggedIn = await Session.attemptAutoLogin();
        console.log('[INIT] Auto-login result:', autoLoggedIn);

        if (autoLoggedIn) {
            console.log('[INIT] Auto-login successful, showing dashboard');
            showDashboard();
        } else {
            console.log('[INIT] Auto-login failed, showing login form');
            showLogin();
        }

        setTimeout(() => {
            const loginContent = document.querySelector('#loginForm .modal-content');
            if (loginContent && !document.getElementById('loginForm').classList.contains('hidden')) {
                UI.updateModalLayout(loginContent);
            }
        }, 200);

        window.addEventListener('resize', () => {
            const contents = ['postsContent', 'messagesContent', 'friendsContent', 'profileContent'];
            const tabs = ['postsTab', 'messagesTab', 'friendsTab', 'profileTab'];

            contents.forEach((contentId, index) => {
                const content = document.getElementById(contentId);
                const tab = document.getElementById(tabs[index]);
                if (content && tab && !tab.classList.contains('hidden')) {
                    UI.updateModalLayout(content);
                }
            });
        });

        // Handle app coming back from background - announce presence
        document.addEventListener('visibilitychange', async () => {
            if (!document.hidden && P2P.initialized) {
                console.log('App became visible, announcing presence...');
                try {
                    await P2P.announcePresence();
                } catch (error) {
                    console.error('Failed to announce presence:', error);
                }
            }
        });

        // Handle page focus (additional safeguard for mobile)
        window.addEventListener('focus', async () => {
            if (P2P.initialized && currentUser) {
                console.log('Window focused, announcing presence...');
                try {
                    await P2P.announcePresence();
                } catch (error) {
                    console.error('Failed to announce presence:', error);
                }
            }
        });
    } catch (error) {
        console.error('Failed to initialize app:', error);
        UI.showError('loginError', 'Failed to initialize app: ' + error.message);
        showLogin();
    }
});

// Add keyboard shortcut for message search
document.addEventListener('DOMContentLoaded', () => {
    const searchInput = document.getElementById('messageSearchInput');
    if (searchInput) {
        searchInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                searchMessages();
            }
        });
    }
});

// Message Search and Editing Functions
async function searchMessages() {
    if (!currentUser) return;

    const searchInput = document.getElementById('messageSearchInput');
    const query = searchInput.value.trim();

    if (!query) {
        UI.showError('searchResults', 'Please enter a search query');
        return;
    }

    try {
        const results = await TauriAPI.invoke('search_messages', {
            userId: currentUser.id,
            query: query
        });

        displaySearchResults(results);
    } catch (error) {
        console.error('Search failed:', error);
        UI.showError('searchResults', 'Search failed: ' + error);
    }
}

function displaySearchResults(results) {
    const searchResultsContainer = document.getElementById('searchResults');

    if (results.length === 0) {
        searchResultsContainer.innerHTML = '<div class="no-search-results">No messages found matching your search.</div>';
        searchResultsContainer.classList.remove('hidden');
        return;
    }

    let resultsHTML = '';
    results.forEach(message => {
        const date = new Date(message.createdAt).toLocaleDateString();
        const time = new Date(message.createdAt).toLocaleTimeString();

        // For search results, we need to decrypt if encrypted
        let contentPreview = message.content;
        if (message.encrypted) {
            contentPreview = '[Encrypted Message - Click to view]';
        }

        // Truncate long messages
        if (contentPreview.length > 150) {
            contentPreview = contentPreview.substring(0, 150) + '...';
        }

        resultsHTML += `
            <div class="search-result-item" onclick="scrollToMessage(${message.id})">
                <div class="search-result-content">${Utils.escapeHtml(contentPreview)}</div>
                <div class="search-result-meta">
                    <span class="search-result-sender">Message ID: ${message.id}</span>
                    <span class="search-result-date">${date} ${time}</span>
                </div>
            </div>
        `;
    });

    searchResultsContainer.innerHTML = resultsHTML;
    searchResultsContainer.classList.remove('hidden');
}

function clearMessageSearch() {
    document.getElementById('messageSearchInput').value = '';
    document.getElementById('searchResults').classList.add('hidden');
}

function scrollToMessage(messageId) {
    // Switch to regular messages view if in search mode
    clearMessageSearch();

    // Find the message element and scroll to it
    const messageElement = document.querySelector(`[data-message-id="${messageId}"]`);
    if (messageElement) {
        messageElement.scrollIntoView({ behavior: 'smooth', block: 'center' });
        // Highlight the message briefly
        messageElement.classList.add('message-highlight');
        setTimeout(() => {
            messageElement.classList.remove('message-highlight');
        }, 2000);
    } else {
        // If message not visible, reload messages and try again
        loadMessages().then(() => {
            const messageElement = document.querySelector(`[data-message-id="${messageId}"]`);
            if (messageElement) {
                messageElement.scrollIntoView({ behavior: 'smooth', block: 'center' });
                messageElement.classList.add('message-highlight');
                setTimeout(() => {
                    messageElement.classList.remove('message-highlight');
                }, 2000);
            }
        });
    }
}

function editMessage(messageId) {
    const messageElement = document.querySelector(`[data-message-id="${messageId}"]`);
    if (!messageElement) return;

    const contentElement = messageElement.querySelector('.message-content');
    const currentContent = contentElement.textContent;

    // Create edit form
    const editForm = document.createElement('div');
    editForm.className = 'message-edit-form';
    editForm.innerHTML = `
        <textarea class="message-edit-textarea" id="edit-textarea-${messageId}">${Utils.escapeHtml(currentContent)}</textarea>
        <div class="message-edit-actions">
            <button class="btn btn-secondary" onclick="cancelEditMessage(${messageId})">Cancel</button>
            <button class="btn btn-primary" onclick="saveEditMessage(${messageId})">Save</button>
        </div>
    `;

    // Replace content with edit form
    contentElement.style.display = 'none';
    messageElement.appendChild(editForm);
    messageElement.classList.add('message-edit-mode');

    // Focus on textarea
    document.getElementById(`edit-textarea-${messageId}`).focus();
}

function cancelEditMessage(messageId) {
    const messageElement = document.querySelector(`[data-message-id="${messageId}"]`);
    if (!messageElement) return;

    const editForm = messageElement.querySelector('.message-edit-form');
    const contentElement = messageElement.querySelector('.message-content');

    if (editForm) editForm.remove();
    if (contentElement) contentElement.style.display = 'block';
    messageElement.classList.remove('message-edit-mode');
}

async function saveEditMessage(messageId) {
    if (!currentUser) return;

    const newContent = document.getElementById(`edit-textarea-${messageId}`).value.trim();

    if (!newContent) {
        alert('Message cannot be empty');
        return;
    }

    try {
        await TauriAPI.invoke('edit_message', {
            messageId: messageId,
            userId: currentUser.id,
            newContent: newContent
        });

        // Reload messages to show the updated content
        loadMessages();

        UI.showSuccess('messages', 'Message updated successfully');
    } catch (error) {
        console.error('Failed to edit message:', error);
        UI.showError('messages', 'Failed to edit message: ' + error);
    }
}

async function deleteMessage(messageId) {
    if (!currentUser) return;

    if (!confirm('Are you sure you want to delete this message? This action cannot be undone.')) {
        return;
    }

    try {
        await TauriAPI.invoke('delete_message', {
            messageId: messageId,
            userId: currentUser.id
        });

        // Remove the message element from DOM
        const messageElement = document.querySelector(`[data-message-id="${messageId}"]`);
        if (messageElement) {
            messageElement.remove();
        }

        UI.showSuccess('messages', 'Message deleted successfully');
    } catch (error) {
        console.error('Failed to delete message:', error);
        UI.showError('messages', 'Failed to delete message: ' + error);
    }
}

// Hamburger Menu Functions
function toggleHamburgerMenu() {
    const navMenu = document.getElementById('navMenu');
    const hamburgerBtn = document.getElementById('hamburgerBtn');
    const backdrop = document.getElementById('navBackdrop');

    navMenu.classList.toggle('hidden');
    hamburgerBtn.classList.toggle('open');
    backdrop.classList.toggle('visible');
}

function closeHamburgerMenu() {
    const navMenu = document.getElementById('navMenu');
    const hamburgerBtn = document.getElementById('hamburgerBtn');
    const backdrop = document.getElementById('navBackdrop');

    navMenu.classList.add('hidden');
    hamburgerBtn.classList.remove('open');
    backdrop.classList.remove('visible');
}

// Post editing and deletion functions
async function editPost(postId) {
    if (!currentUser) return;

    const postElement = document.querySelector(`[data-post-id="${postId}"]`);
    if (!postElement) return;

    const contentElement = postElement.querySelector('.post-content');
    const currentContent = contentElement.textContent;

    // Create edit form
    const editForm = document.createElement('div');
    editForm.className = 'post-edit-form';
    editForm.innerHTML = `
        <textarea class="message-edit-textarea" id="edit-post-textarea-${postId}" style="min-height: 100px;">${Utils.escapeHtml(currentContent)}</textarea>
        <div class="message-edit-actions" style="margin-top: var(--spacing-md); display: flex; gap: var(--spacing-sm); justify-content: flex-end;">
            <button class="btn-secondary btn-small" onclick="cancelEditPost(${postId})">Cancel</button>
            <button class="btn btn-small" onclick="saveEditPost(${postId})">Save</button>
        </div>
    `;

    // Replace content with edit form
    contentElement.style.display = 'none';
    const actionsDiv = postElement.querySelector('.post-actions');
    if (actionsDiv) actionsDiv.style.display = 'none';
    postElement.appendChild(editForm);
    postElement.classList.add('post-edit-mode');

    // Focus on textarea
    document.getElementById(`edit-post-textarea-${postId}`).focus();
}

function cancelEditPost(postId) {
    const postElement = document.querySelector(`[data-post-id="${postId}"]`);
    if (!postElement) return;

    const editForm = postElement.querySelector('.post-edit-form');
    const contentElement = postElement.querySelector('.post-content');
    const actionsDiv = postElement.querySelector('.post-actions');

    if (editForm) editForm.remove();
    if (contentElement) contentElement.style.display = 'block';
    if (actionsDiv) actionsDiv.style.display = 'flex';
    postElement.classList.remove('post-edit-mode');
}

async function saveEditPost(postId) {
    if (!currentUser) return;

    const newContent = document.getElementById(`edit-post-textarea-${postId}`).value.trim();

    if (!newContent) {
        alert('Post content cannot be empty');
        return;
    }

    try {
        await TauriAPI.invoke('edit_post', {
            postId: postId,
            userId: currentUser.id,
            newContent: newContent
        });

        // Reload posts to show the updated content
        await loadPosts();

        UI.showSuccess('dashboardError', 'Post updated successfully');
    } catch (error) {
        console.error('Failed to edit post:', error);
        UI.showError('dashboardError', 'Failed to edit post: ' + error);
    }
}

async function deletePost(postId) {
    if (!currentUser) return;

    if (!confirm('Are you sure you want to delete this post? This action cannot be undone.')) {
        return;
    }

    try {
        await TauriAPI.invoke('delete_post', {
            postId: postId,
            userId: currentUser.id
        });

        // Remove the post element from DOM immediately for responsive UX
        const postElement = document.querySelector(`[data-post-id="${postId}"]`);
        if (postElement) {
            postElement.remove();
        }

        UI.showSuccess('dashboardError', 'Post deleted successfully');

        // Reload posts to sync state
        await loadPosts();
    } catch (error) {
        console.error('Failed to delete post:', error);
        UI.showError('dashboardError', 'Failed to delete post: ' + error);
        // Reload posts even on error to ensure UI is in sync
        await loadPosts();
    }
}

// Add Enter key listener for login form
document.addEventListener('DOMContentLoaded', () => {
    const loginUsername = document.getElementById('loginUsername');
    const loginPassword = document.getElementById('loginPassword');

    if (loginUsername && loginPassword) {
        [loginUsername, loginPassword].forEach(input => {
            input.addEventListener('keypress', (e) => {
                if (e.key === 'Enter') {
                    handleLogin();
                }
            });
        });
    }
});

// New QR Code and P2P Invite Functions
let cameraStream = null;
let scanningInterval = null;

async function openCameraToScanQR() {
    const modal = document.getElementById('qrScannerModal');
    const video = document.getElementById('qrVideo');
    const canvas = document.getElementById('qrCanvas');

    try {
        // Request camera access with back camera preference
        cameraStream = await navigator.mediaDevices.getUserMedia({
            video: { facingMode: 'environment' }
        });

        video.srcObject = cameraStream;
        video.setAttribute('playsinline', true);
        await video.play();

        // Show modal
        modal.classList.remove('hidden');

        // Start scanning
        startQRScanning(video, canvas);
    } catch (error) {
        console.error('Camera access error:', error);
        UI.showError('dashboardError', 'Could not access camera: ' + error.message);
    }
}

function startQRScanning(video, canvas) {
    const ctx = canvas.getContext('2d');

    scanningInterval = setInterval(() => {
        if (video.readyState === video.HAVE_ENOUGH_DATA) {
            canvas.height = video.videoHeight;
            canvas.width = video.videoWidth;
            ctx.drawImage(video, 0, 0, canvas.width, canvas.height);

            const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
            const code = jsQR(imageData.data, imageData.width, imageData.height, {
                inversionAttempts: 'dontInvert',
            });

            if (code) {
                console.log('QR code detected:', code.data);
                handleScannedQRCode(code.data);
            }
        }
    }, 100); // Scan every 100ms
}

async function handleScannedQRCode(data) {
    // Stop scanning
    closeCameraScanner();

    try {
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('🔵 FRONTEND: QR CODE SCANNING STARTED');
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('[QR-SCAN] Scanned data:', data);

        // Extract public key and optional node info from cipher://add-friend?key=... URI
        let publicKey = data;
        let nodeId = null;
        let relayUrl = null;
        if (data.startsWith('cipher://add-friend?key=')) {
            const url = new URL(data);
            publicKey = url.searchParams.get('key');
            nodeId = url.searchParams.get('node');
            const encodedRelay = url.searchParams.get('relay');
            if (encodedRelay) {
                relayUrl = decodeURIComponent(encodedRelay);
            }
            if (nodeId && relayUrl) {
                console.log('[QR-SCAN] Extracted public key and node info from URI');
                console.log('[QR-SCAN]   Public key:', publicKey);
                console.log('[QR-SCAN]   NodeId:', nodeId);
                console.log('[QR-SCAN]   Relay:', relayUrl);
            } else {
                console.log('[QR-SCAN] Extracted public key from URI (no node info):', publicKey);
            }
        } else {
            console.log('[QR-SCAN] Using raw public key (no node info)');
        }

        if (!publicKey) {
            throw new Error('Invalid QR code format - no public key found');
        }

        // Add friend by public key with optional node info - single function call
        console.log('[QR-SCAN] Adding friend by public key...');
        const addedPublicKey = await TauriAPI.invoke('iroh_add_friend_by_public_key', {
            friendPublicKey: publicKey,
            nodeId: nodeId,
            relayUrl: relayUrl
        });
        console.log('[QR-SCAN] ✓ Friend added successfully:', addedPublicKey);
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('✅ FRONTEND: QR SCANNING COMPLETE - FRIEND ADDED');
        console.log('═══════════════════════════════════════════════════════════════');

        UI.showSuccess('dashboardError', 'Friend added successfully!');

        // Reload friends list to show the new friend
        console.log('[QR-SCAN] Reloading friends list...');
        await new Promise(resolve => setTimeout(resolve, 100));
        if (typeof loadFriends === 'function') {
            await loadFriends();
            console.log('[QR-SCAN] Friends list reloaded');
        }
    } catch (error) {
        console.error('Error handling QR code:', error);
        UI.showError('dashboardError', 'Failed to add friend: ' + error);
    }
}

function closeCameraScanner() {
    const modal = document.getElementById('qrScannerModal');
    modal.classList.add('hidden');

    // Stop scanning
    if (scanningInterval) {
        clearInterval(scanningInterval);
        scanningInterval = null;
    }

    // Stop camera stream
    if (cameraStream) {
        cameraStream.getTracks().forEach(track => track.stop());
        cameraStream = null;
    }
}

async function handleQRCodeFromCamera(event) {
    const file = event.target.files[0];
    if (!file) return;

    try {
        const base64Data = await Utils.fileToBase64(file);

        // Try to scan as structured QR code first (old format)
        try {
            const qrCodeData = await TauriAPI.invoke('scan_qr_code_from_image', { base64Image: base64Data });

            if (qrCodeData && qrCodeData.username && qrCodeData.publicKey) {
                await addFriendByQRCode(qrCodeData.username, qrCodeData.publicKey);
                UI.showSuccess('dashboardError', 'Friend added successfully!');
                return;
            }
        } catch (e) {
            console.log('Not a structured QR code, trying raw decode...');
        }

        // If structured parsing failed, try raw decode for P2P invite codes
        const img = document.createElement('img');
        img.src = base64Data;
        await new Promise(resolve => img.onload = resolve);

        const canvas = document.createElement('canvas');
        canvas.width = img.width;
        canvas.height = img.height;
        const ctx = canvas.getContext('2d');
        ctx.drawImage(img, 0, 0);
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);

        // Use jsQR library if available
        if (window.jsQR) {
            const code = window.jsQR(imageData.data, imageData.width, imageData.height);
            if (code && code.data) {
                // Check if it's a P2P invite code
                if (code.data.startsWith('cipher://p2p-invite?code=')) {
                    const inviteCode = new URL(code.data).searchParams.get('code');
                    if (inviteCode) {
                        await P2P.acceptInvite(inviteCode);
                        UI.showSuccess('dashboardError', 'P2P connection established!');
                        return;
                    }
                }
            }
        }

        UI.showError('dashboardError', 'Could not read QR code');
    } catch (error) {
        UI.showError('dashboardError', 'Failed to scan QR code: ' + error);
    }

    event.target.value = '';
}

async function showMyQRCode() {
    if (!currentUser) return;

    try {
        // Generate P2P invite code QR
        const inviteCode = await P2P.generateInvite();
        const qrData = `cipher://p2p-invite?code=${encodeURIComponent(inviteCode)}`;
        const qrCodeDataUrl = await TauriAPI.invoke('generate_qr_code', { data: qrData });

        // Show in overlay
        const overlay = document.createElement('div');
        overlay.style.cssText = 'position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0, 0, 0, 0.9); z-index: 10000; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--spacing-lg);';
        overlay.innerHTML = `
            <div style="text-align: center;">
                <h2 style="color: white; margin-bottom: var(--spacing-lg);">Scan to Connect</h2>
                <div style="background: white; padding: var(--spacing-lg); border-radius: var(--border-radius-md);">
                    <img src="${qrCodeDataUrl}" alt="My QR Code" style="max-width: 300px; max-height: 300px;">
                </div>
                <p style="color: rgba(255, 255, 255, 0.8); margin-top: var(--spacing-lg); max-width: 400px;">
                    Have your friend scan this code to connect instantly via P2P
                </p>
                <button class="btn" style="margin-top: var(--spacing-lg);" onclick="this.parentElement.parentElement.remove()">Close</button>
            </div>
        `;
        document.body.appendChild(overlay);
        overlay.onclick = (e) => {
            if (e.target === overlay) overlay.remove();
        };
    } catch (error) {
        UI.showError('dashboardError', 'Failed to generate QR code: ' + error);
    }
}

async function showManualAddFriend() {
    const friendKey = prompt('Enter your friend\'s public key:');
    if (!friendKey || !friendKey.trim()) return;

    const friendPublicKey = friendKey.trim();

    if (friendPublicKey === currentUser.publicKey) {
        UI.showError('dashboardError', 'You cannot add yourself as a friend');
        return;
    }

    try {
        const friend = await TauriAPI.invoke('add_friend_from_qr_code', {
            currentUserId: currentUser.id,
            qrData: {
                username: `User_${friendPublicKey.substring(0, 8)}`,
                publicKey: friendPublicKey
            }
        });

        UI.showSuccess('dashboardError', `Successfully added ${friend.username} as a friend!`);

        // Exchange P2P invite codes for bootstrapping
        try {
            const myInvite = await P2P.generateInvite();
            console.log('Generated my invite code for friend:', myInvite);
            // Note: In a real implementation, you'd send this invite to the friend
        } catch (error) {
            console.error('Failed to generate invite code:', error);
        }

        loadFriends();
    } catch (error) {
        UI.showError('dashboardError', 'Failed to add friend: ' + error);
    }
}

// Export functions to global scope for onclick handlers
Object.assign(window, {
    handleLogin, handleLogout, showLogin, showFeed, showPosts, showMessages,
    showFriends, showAddFriend, showCreatePostPage, showCreatePost, createPost, cancelCreatePost,
    createPostFromPage, sendMessage, addFriendByPublicKey, addFriendFromTab, copyPublicKey,
    generateMyQRCode, generateProfileQRCode, scanQRCode, handleQRCodeFile, selectFriend,
    viewMediaAttachment, showEditProfile, handleProfilePictureUpload, saveProfile,
    createFriendInvite, useFriendInvite, exportFriendsList, importFriendsList,
    searchMessages, clearMessageSearch, scrollToMessage, editMessage,
    cancelEditMessage, saveEditMessage, deleteMessage,
    // Hamburger menu functions
    toggleHamburgerMenu, closeHamburgerMenu,
    // New QR and P2P functions
    openCameraToScanQR, closeCameraScanner, handleQRCodeFromCamera, showMyQRCode, showManualAddFriend
});

console.log('[MAIN.JS] Functions exported to window scope');
console.log('[MAIN.JS] window.addFriendByPublicKey exists?', typeof window.addFriendByPublicKey);