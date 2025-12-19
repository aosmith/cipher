console.log('[MAIN.JS] ===== FILE LOADING STARTED =====');
// Main application JavaScript - consolidated and optimized
console.log('[MAIN.JS] JavaScript is loading...');

// Global variables
let currentUser = null;
let tauriInvoke = null;
let allFriends = [];
let selectedRecipients = [];

// Helper function to get display name for a user ID
function getDisplayName(userId) {
    if (!userId) return 'Unknown';
    if (currentUser && userId === currentUser.id) return 'You';

    // Look up friend by ID
    const friend = allFriends.find(f => f.id === userId);
    if (friend && friend.displayName) {
        return friend.displayName;
    }

    // Fallback: show truncated ID
    const idStr = String(userId);
    return idStr.length > 8 ? idStr.substring(0, 8) + '...' : idStr;
}

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

                console.log('fileToBase64 - Validation passed [OK]');
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
            // Verify user exists in database (handles case where app data was cleared but localStorage wasn't)
            try {
                const dbUser = await TauriAPI.invoke('get_user_by_id', { userId: savedUser.id });
                if (!dbUser) {
                    console.log('Auto-login: User not found in database, clearing session');
                    this.clear();
                    return false;
                }
            } catch (error) {
                console.error('Auto-login: Failed to verify user in database:', error);
                this.clear();
                return false;
            }

            currentUser = savedUser;

            // Initialize P2P system for auto-logged in user (non-blocking)
            const displayName = savedUser.displayName || 'User';
            const publicKey = savedUser.publicKey || savedUser.public_key; // Support both formats
            const deviceId = savedUser.deviceId || savedUser.device_id; // Support both formats
            P2P.initialize(savedUser.id, displayName, publicKey, deviceId).then(() => {
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
        const navLinks = ['postsNavLink', 'createPostNavLink', 'messagesNavLink', 'friendsNavLink', 'profileNavLink', 'communitiesNavLink'];
        navLinks.forEach(id => {
            const element = document.getElementById(id);
            if (element) {
                element.classList.toggle('active', id === activeId);
            }
        });
    },

    hideAllTabs() {
        const tabs = ['postsTab', 'createPostTab', 'messagesTab', 'friendsTab', 'profileTab', 'addFriendTab', 'settingsTab', 'communitiesTab', 'communityDetailTab'];
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

    showImageViewer(src) {
        // Create fullscreen image viewer
        const viewer = document.createElement('div');
        viewer.className = 'image-viewer';
        viewer.id = 'imageViewer';
        viewer.innerHTML = `
            <div class="image-viewer-backdrop" onclick="UI.closeImageViewer()"></div>
            <img src="${src}" class="image-viewer-img" onclick="UI.closeImageViewer()">
            <div class="image-viewer-controls">
                <button class="image-viewer-save" onclick="UI.saveImage('${src}')" title="Save image">
                    <span>💾</span> Save
                </button>
                <button class="image-viewer-close-btn" onclick="UI.closeImageViewer()" title="Close">
                    &times;
                </button>
            </div>
        `;
        document.body.appendChild(viewer);

        // Prevent body scroll
        document.body.style.overflow = 'hidden';
    },

    closeImageViewer() {
        const viewer = document.getElementById('imageViewer');
        if (viewer) {
            viewer.remove();
            document.body.style.overflow = '';
        }
    },

    async saveImage(src) {
        try {
            // Generate filename with timestamp
            const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
            const filename = `cipher-image-${timestamp}.png`;

            // For data URLs, extract the base64 data
            if (src.startsWith('data:')) {
                const base64Data = src.split(',')[1];
                const mimeType = src.split(';')[0].split(':')[1];
                const extension = mimeType.split('/')[1] || 'png';
                const finalFilename = `cipher-image-${timestamp}.${extension}`;

                // Try to save via Tauri backend
                try {
                    await TauriAPI.invoke('save_media_to_downloads', {
                        base64Data: base64Data,
                        filename: finalFilename,
                        mimeType: mimeType
                    });
                    UI.showToast('Image saved to Downloads');
                    return;
                } catch (e) {
                    console.log('Tauri save failed, falling back to browser download:', e);
                }

                // Fallback: browser download
                const link = document.createElement('a');
                link.href = src;
                link.download = finalFilename;
                document.body.appendChild(link);
                link.click();
                document.body.removeChild(link);
                UI.showToast('Image download started');
            } else {
                // For regular URLs, fetch and save
                const response = await fetch(src);
                const blob = await response.blob();
                const extension = blob.type.split('/')[1] || 'png';
                const finalFilename = `cipher-image-${timestamp}.${extension}`;

                // Try Tauri save
                try {
                    const arrayBuffer = await blob.arrayBuffer();
                    const base64Data = btoa(String.fromCharCode(...new Uint8Array(arrayBuffer)));
                    await TauriAPI.invoke('save_media_to_downloads', {
                        base64Data: base64Data,
                        filename: finalFilename,
                        mimeType: blob.type
                    });
                    UI.showToast('Image saved to Downloads');
                    return;
                } catch (e) {
                    console.log('Tauri save failed, falling back to browser download:', e);
                }

                // Fallback: browser download
                const url = URL.createObjectURL(blob);
                const link = document.createElement('a');
                link.href = url;
                link.download = finalFilename;
                document.body.appendChild(link);
                link.click();
                document.body.removeChild(link);
                URL.revokeObjectURL(url);
                UI.showToast('Image download started');
            }
        } catch (error) {
            console.error('Failed to save image:', error);
            UI.showToast('Failed to save image');
        }
    },

    showToast(message, type = 'info', duration = 3000) {
        // Remove existing toast
        const existingToast = document.getElementById('uiToast');
        if (existingToast) existingToast.remove();

        const toast = document.createElement('div');
        toast.id = 'uiToast';
        toast.className = `ui-toast ui-toast-${type}`;
        toast.textContent = message;
        document.body.appendChild(toast);

        // Trigger animation
        requestAnimationFrame(() => {
            toast.classList.add('show');
        });

        // Remove after duration
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 300);
        }, duration);
    },

    updateUserInterface() {
        if (!currentUser) return;

        const userGreeting = document.getElementById('userGreeting');
        if (userGreeting) {
            userGreeting.textContent = currentUser.displayName;
        }

        const userPublicKey = document.getElementById('userPublicKey');
        if (userPublicKey && currentUser.publicKey) {
            userPublicKey.textContent = currentUser.publicKey;
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
        // Stop notification polling
        Navbar.stopNotificationPolling();
    }
    UI.clearErrors();

    setTimeout(() => {
        const loginContent = document.querySelector('#loginForm .modal-content');
        if (loginContent) UI.updateModalLayout(loginContent);
    }, 100);
}

function showDashboard() {
    console.log('[DASHBOARD] showDashboard called');
    document.getElementById('loginForm').classList.add('hidden');
    document.getElementById('dashboard').classList.remove('hidden');
    document.body.classList.add('dashboard-view');
    document.body.classList.remove('app-loading');
    // Show logged-in navbar elements using Navbar module
    if (typeof Navbar !== 'undefined') {
        Navbar.updateLoginState(true);
        // Start notification polling
        Navbar.startNotificationPolling();
    }
    UI.clearErrors();
    UI.updateUserInterface();
    console.log('[DASHBOARD] About to call loadPosts');
    loadPosts();
    console.log('[DASHBOARD] About to call showFeed');
    showFeed();
    console.log('[DASHBOARD] showDashboard complete');
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
    console.log('[SHOW-FRIENDS] showFriends() called');
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
                const maxFileSize = 64 * 1024 * 1024; // 64MB per file
                const maxTotalSize = 256 * 1024 * 1024; // 256MB total
                let hasOversizedFile = false;

                for (let i = 0; i < this.files.length; i++) {
                    if (this.files[i].size > maxFileSize) {
                        hasOversizedFile = true;
                        break;
                    }
                }

                if (hasOversizedFile) {
                    countDisplay.innerHTML = `<span style="color: var(--color-error);">⚠️ Some files exceed 64MB limit</span>`;
                } else if (totalSize > maxTotalSize) {
                    countDisplay.innerHTML = `<span style="color: var(--color-error);">⚠️ Total size exceeds 256MB limit</span>`;
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

async function showAddFriend() {
    UI.showTab('addFriendTab', 'addFriendContent', 'addFriendNavLink', async () => {
        // Clear the input field and error/success messages when showing the tab
        const input = document.getElementById('addFriendPublicKey');
        if (input) {
            input.value = '';
            setTimeout(() => input.focus(), 100);
        }

        // Hide any previous error/success messages
        const errorEl = document.getElementById('addFriendTabError');
        const successEl = document.getElementById('addFriendTabSuccess');
        if (errorEl) errorEl.classList.add('hidden');
        if (successEl) successEl.classList.add('hidden');

        // Load the user's invite link
        const inviteLinkInput = document.getElementById('myInviteLink');
        if (inviteLinkInput) {
            try {
                const inviteCode = await P2P.generateInvite();
                inviteLinkInput.value = inviteCode;
            } catch (error) {
                console.error('Failed to generate invite link:', error);
                inviteLinkInput.value = 'Error loading invite link';
            }
        }
    });
}

// Settings page
async function showSettings() {
    UI.showTab('settingsTab', 'settingsContent', 'settingsNavLink', async () => {
        // Load current user profile data from global currentUser
        if (currentUser) {
            const displayNameEl = document.getElementById('settingsDisplayName');
            const bioInput = document.getElementById('settingsBio');
            if (displayNameEl) displayNameEl.textContent = currentUser.displayName || 'Unknown';
            if (bioInput) bioInput.value = currentUser.bio || '';
        }

        // Load settings from backend
        try {
            const settings = await TauriAPI.invoke('get_app_settings');

            // Storage settings
            const storageLimitSelect = document.getElementById('storageLimit');
            const storageLimitDisplay = document.getElementById('storageLimitDisplay');
            const storageUsedEl = document.getElementById('storageUsed');

            if (storageLimitSelect) {
                // Convert bytes to GB for dropdown
                const storageLimitGB = String(Math.round(settings.storageLimitBytes / (1024 * 1024 * 1024)));
                storageLimitSelect.value = storageLimitGB;
            }
            if (storageLimitDisplay) {
                storageLimitDisplay.textContent = formatBytesLimit(settings.storageLimitBytes);
            }
            if (storageUsedEl) {
                storageUsedEl.textContent = formatBytes(settings.storageUsedBytes);
            }
        } catch (error) {
            console.error('Failed to load app settings:', error);
        }

        // Load safety settings (blocked/muted users)
        await SafetyManager.loadAll();

        // Load device list
        await DeviceManager.loadDevices();
    });
}

// Format bytes for display (human readable)
function formatBytes(bytes) {
    if (bytes === 0) return '0 MB';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

// Format bytes limit for display (handles -1 for unlimited)
function formatBytesLimit(bytes) {
    if (bytes === -1) return 'Unlimited';
    if (bytes === 0) return '0 MB';
    if (bytes < 1024 * 1024 * 1024) return `${Math.round(bytes / (1024 * 1024))} MB`;
    return `${Math.round(bytes / (1024 * 1024 * 1024))} GB`;
}

// Save storage contribution limit
async function saveStorageLimit() {
    const storageLimitSelect = document.getElementById('storageLimit');
    const storageLimitDisplay = document.getElementById('storageLimitDisplay');

    if (storageLimitSelect) {
        const limitGB = storageLimitSelect.value;
        // Convert GB to bytes
        const limitBytes = parseInt(limitGB) * 1024 * 1024 * 1024;

        try {
            await TauriAPI.invoke('set_storage_limit', { limitBytes });

            if (storageLimitDisplay) {
                storageLimitDisplay.textContent = formatBytesLimit(limitBytes);
            }

            console.log(`Storage contribution limit set to ${formatBytesLimit(limitBytes)}`);
        } catch (error) {
            console.error('Failed to save storage limit:', error);
        }
    }
}

// Save profile settings (bio only - display name is immutable)
async function saveProfileSettings() {
    const bio = document.getElementById('settingsBio')?.value?.trim();
    const saveBtn = document.getElementById('saveProfileBtn');

    if (!currentUser) {
        alert('Not logged in');
        return;
    }

    // Disable button during processing
    if (saveBtn) {
        saveBtn.disabled = true;
        saveBtn.textContent = 'Saving...';
    }

    try {
        await TauriAPI.invoke('update_user_profile', {
            userId: currentUser.id,
            displayName: null,  // Don't change display name
            bio: bio || null,
            profilePicture: null
        });
        // Update the global currentUser with new values
        currentUser.bio = bio || '';
        // Update the session storage
        UserSession.save(currentUser);

        // Show success state on button
        if (saveBtn) {
            saveBtn.textContent = 'Saved ✓';
            // Re-enable after a delay so user can save again if needed
            setTimeout(() => {
                saveBtn.disabled = false;
                saveBtn.textContent = 'Save Bio';
            }, 2000);
        }
    } catch (error) {
        console.error('Failed to save bio:', error);
        alert('Failed to save bio: ' + error);
        // Re-enable button on error
        if (saveBtn) {
            saveBtn.disabled = false;
            saveBtn.textContent = 'Save Bio';
        }
    }
}


// Copy user's invite link to clipboard
async function copyMyInviteLink() {
    const inviteLinkInput = document.getElementById('myInviteLink');
    if (!inviteLinkInput || !inviteLinkInput.value || inviteLinkInput.value === 'Loading...' || inviteLinkInput.value.startsWith('Error')) {
        return;
    }

    try {
        await navigator.clipboard.writeText(inviteLinkInput.value);
        const successEl = document.getElementById('addFriendTabSuccess');
        if (successEl) {
            successEl.textContent = 'Invite link copied to clipboard!';
            successEl.classList.remove('hidden');
            setTimeout(() => successEl.classList.add('hidden'), 2000);
        }
    } catch (error) {
        console.error('Failed to copy invite link:', error);
    }
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
    const createBtn = document.getElementById('createAccountBtn');

    if (!displayName) {
        UI.showError('loginError', 'Please enter a display name');
        return;
    }

    // Disable button during processing
    if (createBtn) {
        createBtn.disabled = true;
        createBtn.textContent = 'Creating...';
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

            // Keep button disabled after success
            if (createBtn) createBtn.textContent = 'Account Created ✓';

            // Store user for later authentication
            pendingAuthUser = user;

            // Show recovery phrase modal
            showRecoveryPhraseModal(recoveryPhrase);
        } else {
            console.error('[CREATE_ACCOUNT] Missing user or recoveryPhrase!', {user, recoveryPhrase});
            UI.showError('loginError', 'Failed to create account - invalid response from server');
            // Re-enable button on error
            if (createBtn) {
                createBtn.disabled = false;
                createBtn.textContent = 'Create New Account';
            }
        }
    } catch (error) {
        console.error('[CREATE_ACCOUNT] Exception:', error);
        await TauriAPI.debugLog('Account creation error: ' + error.toString());
        UI.showError('loginError', 'Account creation failed: ' + error);
        // Re-enable button on error
        if (createBtn) {
            createBtn.disabled = false;
            createBtn.textContent = 'Create New Account';
        }
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
    const restoreBtn = document.getElementById('restoreAccountBtn');

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

    // Disable button during processing
    if (restoreBtn) {
        restoreBtn.disabled = true;
        restoreBtn.textContent = 'Restoring...';
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
            // Keep button disabled after success
            if (restoreBtn) restoreBtn.textContent = 'Account Restored ✓';
            await completeAuthentication(user);
        } else {
            UI.showError('loginError', 'Failed to restore account');
            // Re-enable button on error
            if (restoreBtn) {
                restoreBtn.disabled = false;
                restoreBtn.textContent = 'Restore Account';
            }
        }
    } catch (error) {
        await TauriAPI.debugLog('Account restoration error: ' + error.toString());
        UI.showError('loginError', 'Account restoration failed: ' + error);
        // Re-enable button on error
        if (restoreBtn) {
            restoreBtn.disabled = false;
            restoreBtn.textContent = 'Restore Account';
        }
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
        await P2P.initialize(user.id, user.displayName || 'User', user.publicKey, user.deviceId);
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
        console.log('[POST] Creating post:', { userId: currentUser.id, contentLength: content.length, hasAttachments: !!attachments });

        try {
            // Validate file sizes before processing
            if (attachments && attachments.length > 0) {
                const maxFileSize = 64 * 1024 * 1024; // 64MB per file
                const maxTotalSize = 256 * 1024 * 1024; // 256MB total
                let totalSize = 0;

                for (let i = 0; i < attachments.length; i++) {
                    const file = attachments[i];
                    if (file.size > maxFileSize) {
                        throw new Error(`File "${file.name}" is too large. Maximum file size is 64MB.`);
                    }
                    totalSize += file.size;
                    if (totalSize > maxTotalSize) {
                        throw new Error('Total attachment size exceeds 256MB limit.');
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
                console.log('createMediaPreview - Base64 validation passed [OK]');
                console.log('createMediaPreview - Creating image with data URL (length:', dataUrl.length, ')');
                return `<img src="${dataUrl}" alt="Image" class="post-image" onclick="UI.showImageViewer(this.src)">`;
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

// Post Interactions - Reactions, Comments, Sharing, Edit
const PostInteractions = {
    EMOJIS: ['👍', '❤️', '😂', '😮', '😢', '😡'],

    // Get reaction summary for a post (emoji -> count)
    async getReactionSummary(postId) {
        try {
            return await TauriAPI.invoke('get_post_reaction_summary', { postId });
        } catch (error) {
            console.warn('Failed to get reaction summary:', error);
            return [];
        }
    },

    // Get the current user's reaction on a post
    async getUserReaction(postId) {
        if (!currentUser) return null;
        try {
            const reactions = await TauriAPI.invoke('get_post_reactions', { postId });
            const userReaction = reactions.find(r => r.userId === currentUser.id);
            return userReaction ? userReaction.emoji : null;
        } catch (error) {
            console.warn('Failed to get user reaction:', error);
            return null;
        }
    },

    // Get comment count for a post
    async getCommentCount(postId) {
        try {
            return await TauriAPI.invoke('get_post_comment_count', { postId });
        } catch (error) {
            console.warn('Failed to get comment count:', error);
            return 0;
        }
    },

    // Render reaction summary HTML
    renderReactionSummary(summary, userReaction) {
        if (!summary || summary.length === 0) {
            return '<span class="no-reactions">Be first to react</span>';
        }
        return summary.map(([emoji, count]) => `
            <button class="reaction-chip ${userReaction === emoji ? 'user-reacted' : ''}"
                    onclick="PostInteractions.toggleReaction(this, '${emoji}')"
                    data-emoji="${emoji}">
                <span class="reaction-emoji">${emoji}</span>
                <span class="reaction-count">${count}</span>
            </button>
        `).join('');
    },

    // Toggle a reaction on/off
    async toggleReaction(element, emoji) {
        const postEl = element.closest('.post');
        const postId = postEl?.dataset.postId;
        if (!postId || !currentUser) return;

        const wasSelected = element.classList.contains('user-reacted');

        try {
            if (wasSelected) {
                await TauriAPI.invoke('remove_post_reaction', { postId, userId: currentUser.id });
            } else {
                // Remove any existing reaction first, then add new one
                try {
                    await TauriAPI.invoke('remove_post_reaction', { postId, userId: currentUser.id });
                } catch (e) { /* ignore if no existing reaction */ }
                await TauriAPI.invoke('add_post_reaction', { postId, userId: currentUser.id, emoji });
            }
            // Refresh the reactions display
            await this.refreshReactions(postId);
        } catch (error) {
            console.error('Failed to toggle reaction:', error);
        }
    },

    // Show reaction picker popup
    showReactionPicker(event, postId) {
        event.stopPropagation();
        // Remove any existing picker
        this.closeReactionPicker();

        const picker = document.createElement('div');
        picker.className = 'reaction-picker';
        picker.id = 'reactionPicker';
        picker.innerHTML = this.EMOJIS.map(emoji =>
            `<button class="picker-emoji" onclick="PostInteractions.addReaction('${postId}', '${emoji}')">${emoji}</button>`
        ).join('');

        // Position near the button, but ensure it stays on screen
        const rect = event.target.getBoundingClientRect();
        const pickerWidth = 280; // Approximate width of picker
        const padding = 16;

        picker.style.position = 'fixed';
        picker.style.top = `${rect.bottom + 5}px`;

        // Check if picker would overflow right edge
        if (rect.left + pickerWidth > window.innerWidth - padding) {
            // Align to right edge of screen
            picker.style.right = `${padding}px`;
            picker.style.left = 'auto';
        } else {
            picker.style.left = `${Math.max(padding, rect.left)}px`;
        }

        document.body.appendChild(picker);

        // Close picker when clicking outside
        setTimeout(() => {
            document.addEventListener('click', this.closeReactionPicker, { once: true });
        }, 0);
    },

    closeReactionPicker() {
        const picker = document.getElementById('reactionPicker');
        if (picker) picker.remove();
    },

    // Add a reaction from the picker
    async addReaction(postId, emoji) {
        this.closeReactionPicker();
        if (!currentUser) return;

        try {
            // Remove existing reaction first
            try {
                await TauriAPI.invoke('remove_post_reaction', { postId, userId: currentUser.id });
            } catch (e) { /* ignore */ }
            await TauriAPI.invoke('add_post_reaction', { postId, userId: currentUser.id, emoji });
            await this.refreshReactions(postId);
        } catch (error) {
            console.error('Failed to add reaction:', error);
        }
    },

    // Refresh reactions display for a post
    async refreshReactions(postId) {
        const summary = await this.getReactionSummary(postId);
        const userReaction = await this.getUserReaction(postId);
        const container = document.getElementById(`reactions-${postId}`);
        if (container) {
            container.innerHTML = this.renderReactionSummary(summary, userReaction);
        }
    },

    // Toggle comments section visibility
    async toggleComments(postId) {
        const section = document.getElementById(`comments-section-${postId}`);
        if (!section) return;

        const isHidden = section.classList.contains('hidden');
        if (isHidden) {
            section.classList.remove('hidden');
            await this.loadComments(postId);
        } else {
            section.classList.add('hidden');
        }
    },

    // Load comments for a post
    async loadComments(postId) {
        const container = document.getElementById(`comments-list-${postId}`);
        if (!container) return;

        try {
            const comments = await TauriAPI.invoke('get_post_comments', { postId });
            if (comments.length === 0) {
                container.innerHTML = '<p class="no-comments">No comments yet. Be the first!</p>';
            } else {
                container.innerHTML = comments.map(comment => this.renderComment(comment, postId)).join('');
            }
        } catch (error) {
            console.error('Failed to load comments:', error);
            container.innerHTML = '<p class="comment-error">Failed to load comments</p>';
        }
    },

    // Render a single comment
    renderComment(comment, postId) {
        const isOwn = currentUser && comment.userId === currentUser.id;
        const timeAgo = this.formatTimeAgo(new Date(comment.createdAt));
        const displayName = getDisplayName(comment.userId);

        return `
            <div class="comment" data-comment-id="${comment.id}" style="margin-left: ${(comment.depth || 0) * 20}px">
                <div class="comment-header">
                    <span class="comment-author">${Utils.escapeHtml(displayName)}</span>
                    <span class="comment-time">${timeAgo}</span>
                </div>
                <div class="comment-content">${Utils.escapeHtml(comment.content)}</div>
                <div class="comment-actions">
                    <button class="comment-action" onclick="PostInteractions.showReplyInput('${postId}', '${comment.id}')">Reply</button>
                    ${isOwn ? `<button class="comment-action comment-delete" onclick="PostInteractions.deleteComment('${postId}', '${comment.id}')">Delete</button>` : ''}
                </div>
                <div class="reply-input-wrapper hidden" id="reply-input-${comment.id}">
                    <input type="text" class="comment-input" placeholder="Write a reply..." onkeypress="if(event.key==='Enter') PostInteractions.submitReply('${postId}', '${comment.id}')">
                    <button class="comment-submit-btn" onclick="PostInteractions.submitReply('${postId}', '${comment.id}')">Reply</button>
                </div>
            </div>
        `;
    },

    // Format time ago
    formatTimeAgo(date) {
        const seconds = Math.floor((new Date() - date) / 1000);
        if (seconds < 60) return 'just now';
        const minutes = Math.floor(seconds / 60);
        if (minutes < 60) return `${minutes}m ago`;
        const hours = Math.floor(minutes / 60);
        if (hours < 24) return `${hours}h ago`;
        const days = Math.floor(hours / 24);
        if (days < 7) return `${days}d ago`;
        return date.toLocaleDateString();
    },

    // Submit a new comment
    async submitComment(postId) {
        const input = document.getElementById(`comment-input-${postId}`);
        if (!input || !currentUser) return;

        const content = input.value.trim();
        if (!content) return;

        try {
            await TauriAPI.invoke('add_post_comment', {
                postId,
                userId: currentUser.id,
                content,
                parentId: null
            });
            input.value = '';
            await this.loadComments(postId);

            // Update comment count in button
            const count = await this.getCommentCount(postId);
            const postEl = document.querySelector(`[data-post-id="${postId}"]`);
            if (postEl) {
                const countEl = postEl.querySelector('.action-count');
                if (countEl) countEl.textContent = count;
            }
        } catch (error) {
            console.error('Failed to add comment:', error);
            alert('Failed to add comment');
        }
    },

    // Show reply input
    showReplyInput(postId, commentId) {
        const wrapper = document.getElementById(`reply-input-${commentId}`);
        if (wrapper) {
            wrapper.classList.toggle('hidden');
            if (!wrapper.classList.contains('hidden')) {
                wrapper.querySelector('input')?.focus();
            }
        }
    },

    // Submit a reply
    async submitReply(postId, parentId) {
        const wrapper = document.getElementById(`reply-input-${parentId}`);
        const input = wrapper?.querySelector('input');
        if (!input || !currentUser) return;

        const content = input.value.trim();
        if (!content) return;

        try {
            await TauriAPI.invoke('add_post_comment', {
                postId,
                userId: currentUser.id,
                content,
                parentId
            });
            input.value = '';
            wrapper.classList.add('hidden');
            await this.loadComments(postId);

            // Update comment count
            const count = await this.getCommentCount(postId);
            const postEl = document.querySelector(`[data-post-id="${postId}"]`);
            if (postEl) {
                const countEl = postEl.querySelector('.action-count');
                if (countEl) countEl.textContent = count;
            }
        } catch (error) {
            console.error('Failed to add reply:', error);
            alert('Failed to add reply');
        }
    },

    // Delete a comment
    async deleteComment(postId, commentId) {
        if (!confirm('Delete this comment?')) return;
        if (!currentUser) return;

        try {
            await TauriAPI.invoke('delete_post_comment', {
                commentId,
                userId: currentUser.id
            });
            await this.loadComments(postId);

            // Update comment count
            const count = await this.getCommentCount(postId);
            const postEl = document.querySelector(`[data-post-id="${postId}"]`);
            if (postEl) {
                const countEl = postEl.querySelector('.action-count');
                if (countEl) countEl.textContent = count;
            }
        } catch (error) {
            console.error('Failed to delete comment:', error);
            alert('Failed to delete comment');
        }
    },

    // Show share modal
    showShareModal(postId) {
        const modal = document.getElementById('sharePostModal');
        if (!modal) {
            this.createShareModal();
        }

        // Store postId for later use
        document.getElementById('sharePostModal').dataset.postId = postId;

        // Load post preview
        const postEl = document.querySelector(`[data-post-id="${postId}"]`);
        if (postEl) {
            const content = postEl.querySelector('.post-content')?.textContent || '';
            const author = postEl.querySelector('.post-meta')?.textContent?.split('•')[0]?.trim() || 'Unknown';
            document.getElementById('sharePreviewContent').textContent =
                content.length > 100 ? content.substring(0, 100) + '...' : content;
            document.getElementById('sharePreviewAuthor').textContent = `@${author}`;
        }

        document.getElementById('sharePostModal').classList.remove('hidden');
        document.getElementById('shareComment').value = '';
    },

    createShareModal() {
        const modal = document.createElement('div');
        modal.id = 'sharePostModal';
        modal.className = 'modal hidden';
        modal.innerHTML = `
            <div class="modal-backdrop" onclick="PostInteractions.closeShareModal()"></div>
            <div class="modal-dialog" style="max-width: 400px;">
                <div class="modal-header">
                    <h3>Share Post</h3>
                    <button class="modal-close" onclick="PostInteractions.closeShareModal()">&times;</button>
                </div>
                <div class="modal-body">
                    <div class="share-preview">
                        <p id="sharePreviewContent" class="share-preview-text"></p>
                        <p id="sharePreviewAuthor" class="share-preview-author"></p>
                    </div>
                    <div class="form-group" style="margin-top: var(--spacing-lg);">
                        <label for="shareComment">Add your thoughts (optional)</label>
                        <textarea id="shareComment" class="textarea" rows="2" placeholder="What do you think?"></textarea>
                    </div>
                </div>
                <div class="modal-footer">
                    <button class="btn btn-secondary" onclick="PostInteractions.closeShareModal()">Cancel</button>
                    <button class="btn btn-primary" onclick="PostInteractions.confirmShare()">Share</button>
                </div>
            </div>
        `;
        document.body.appendChild(modal);
    },

    closeShareModal() {
        const modal = document.getElementById('sharePostModal');
        if (modal) modal.classList.add('hidden');
    },

    async confirmShare() {
        const modal = document.getElementById('sharePostModal');
        const postId = modal?.dataset.postId;
        if (!postId || !currentUser) return;

        const comment = document.getElementById('shareComment')?.value.trim() || null;

        try {
            await TauriAPI.invoke('share_post', {
                postId,
                userId: currentUser.id,
                comment
            });
            this.closeShareModal();
            alert('Post shared successfully!');
            // Reload posts to show shared post
            await loadPosts();
        } catch (error) {
            console.error('Failed to share post:', error);
            alert('Failed to share post: ' + error);
        }
    },

    // Show post menu (edit, delete)
    showPostMenu(event, postId) {
        event.stopPropagation();
        this.closePostMenu();

        const menu = document.createElement('div');
        menu.className = 'post-menu';
        menu.id = 'postMenu';
        menu.innerHTML = `
            <button class="post-menu-item" onclick="PostInteractions.editPost('${postId}')">
                ✏️ Edit Post
            </button>
            <button class="post-menu-item post-menu-danger" onclick="PostInteractions.deletePost('${postId}')">
                🗑️ Delete Post
            </button>
        `;

        const rect = event.target.getBoundingClientRect();
        menu.style.position = 'fixed';
        menu.style.right = `${window.innerWidth - rect.right}px`;
        menu.style.top = `${rect.bottom + 5}px`;

        document.body.appendChild(menu);

        setTimeout(() => {
            document.addEventListener('click', this.closePostMenu, { once: true });
        }, 0);
    },

    closePostMenu() {
        const menu = document.getElementById('postMenu');
        if (menu) menu.remove();
    },

    // Edit a post
    async editPost(postId) {
        this.closePostMenu();

        const postEl = document.querySelector(`[data-post-id="${postId}"]`);
        if (!postEl) return;

        const contentEl = postEl.querySelector('.post-content');
        const currentContent = contentEl?.textContent || '';

        // Replace content with edit form
        const originalContent = contentEl.innerHTML;
        contentEl.innerHTML = `
            <div class="edit-post-wrapper">
                <textarea class="textarea edit-post-textarea" id="edit-content-${postId}">${Utils.escapeHtml(currentContent)}</textarea>
                <div class="edit-post-actions">
                    <button class="btn btn-sm btn-primary" onclick="PostInteractions.saveEdit('${postId}')">Save</button>
                    <button class="btn btn-sm btn-secondary" onclick="PostInteractions.cancelEdit('${postId}', '${encodeURIComponent(originalContent)}')">Cancel</button>
                </div>
            </div>
        `;
        document.getElementById(`edit-content-${postId}`)?.focus();
    },

    async saveEdit(postId) {
        const textarea = document.getElementById(`edit-content-${postId}`);
        if (!textarea || !currentUser) return;

        const newContent = textarea.value.trim();
        if (!newContent) {
            alert('Post content cannot be empty');
            return;
        }

        try {
            await TauriAPI.invoke('edit_post', {
                postId,
                userId: currentUser.id,
                content: newContent
            });
            await loadPosts();
        } catch (error) {
            console.error('Failed to edit post:', error);
            alert('Failed to edit post: ' + error);
        }
    },

    cancelEdit(postId, encodedOriginal) {
        const postEl = document.querySelector(`[data-post-id="${postId}"]`);
        const contentEl = postEl?.querySelector('.post-content');
        if (contentEl) {
            contentEl.innerHTML = decodeURIComponent(encodedOriginal);
        }
    },

    // Delete a post
    async deletePost(postId) {
        this.closePostMenu();
        if (!confirm('Are you sure you want to delete this post?')) return;
        if (!currentUser) return;

        try {
            await TauriAPI.invoke('delete_post', {
                postId,
                userId: currentUser.id
            });
            await loadPosts();
        } catch (error) {
            console.error('Failed to delete post:', error);
            alert('Failed to delete post: ' + error);
        }
    }
};

// Safety Manager - Block & Mute functionality
const SafetyManager = {
    pendingBlockUserId: null,
    pendingMuteUserId: null,

    // Show block user modal
    showBlockModal(userId, displayName) {
        this.pendingBlockUserId = userId;
        document.getElementById('blockUserName').textContent = displayName || 'this user';
        document.getElementById('blockReason').value = '';
        document.getElementById('blockUserModal').classList.remove('hidden');
    },

    closeBlockModal() {
        this.pendingBlockUserId = null;
        document.getElementById('blockUserModal').classList.add('hidden');
    },

    async confirmBlock() {
        if (!this.pendingBlockUserId || !currentUser) return;

        const reason = document.getElementById('blockReason')?.value.trim() || null;

        try {
            await TauriAPI.invoke('block_user', {
                blockerId: currentUser.id,
                blockedId: this.pendingBlockUserId,
                reason
            });
            this.closeBlockModal();
            alert('User blocked successfully');
            // Refresh blocked list in settings
            await this.loadBlockedUsers();
            // Refresh posts to hide blocked user's content
            await loadPosts();
        } catch (error) {
            console.error('Failed to block user:', error);
            alert('Failed to block user: ' + error);
        }
    },

    async unblockUser(blockedId) {
        if (!currentUser) return;
        if (!confirm('Unblock this user?')) return;

        try {
            await TauriAPI.invoke('unblock_user', {
                blockerId: currentUser.id,
                blockedId
            });
            await this.loadBlockedUsers();
        } catch (error) {
            console.error('Failed to unblock user:', error);
            alert('Failed to unblock user: ' + error);
        }
    },

    async loadBlockedUsers() {
        if (!currentUser) return;

        try {
            const blocked = await TauriAPI.invoke('get_blocked_users', { userId: currentUser.id });
            const container = document.getElementById('blockedUsersList');
            const countEl = document.getElementById('blockedCount');

            if (countEl) countEl.textContent = blocked.length;

            if (!container) return;

            if (blocked.length === 0) {
                container.innerHTML = '<p style="color: var(--color-text-muted); font-size: var(--font-size-xs); text-align: center; padding: var(--spacing-sm);">No blocked users</p>';
            } else {
                container.innerHTML = blocked.map(user => `
                    <div class="safety-item">
                        <span class="safety-item-name">${Utils.escapeHtml(getDisplayName(user.blockedId))}</span>
                        <button class="btn btn-sm btn-secondary" onclick="SafetyManager.unblockUser('${user.blockedId}')">Unblock</button>
                    </div>
                `).join('');
            }
        } catch (error) {
            console.error('Failed to load blocked users:', error);
        }
    },

    // Show mute user modal
    showMuteModal(userId, displayName) {
        this.pendingMuteUserId = userId;
        document.getElementById('muteUserName').textContent = displayName || 'this user';
        document.getElementById('muteNotifications').checked = true;
        document.getElementById('muteMessages').checked = true;
        document.getElementById('mutePosts').checked = true;
        document.getElementById('muteDuration').value = '0';
        document.getElementById('muteUserModal').classList.remove('hidden');
    },

    closeMuteModal() {
        this.pendingMuteUserId = null;
        document.getElementById('muteUserModal').classList.add('hidden');
    },

    async confirmMute() {
        if (!this.pendingMuteUserId || !currentUser) return;

        const muteNotifications = document.getElementById('muteNotifications').checked;
        const muteMessages = document.getElementById('muteMessages').checked;
        const mutePosts = document.getElementById('mutePosts').checked;
        const durationHours = parseInt(document.getElementById('muteDuration').value) || 0;

        // Calculate expiry time if duration is set
        let expiresAt = null;
        if (durationHours > 0) {
            const expiry = new Date();
            expiry.setHours(expiry.getHours() + durationHours);
            expiresAt = expiry.toISOString();
        }

        try {
            await TauriAPI.invoke('mute_user', {
                muterId: currentUser.id,
                mutedId: this.pendingMuteUserId,
                muteNotifications,
                muteMessages,
                mutePosts,
                expiresAt
            });
            this.closeMuteModal();
            alert('User muted successfully');
            await this.loadMutedUsers();
        } catch (error) {
            console.error('Failed to mute user:', error);
            alert('Failed to mute user: ' + error);
        }
    },

    async unmuteUser(mutedId) {
        if (!currentUser) return;
        if (!confirm('Unmute this user?')) return;

        try {
            await TauriAPI.invoke('unmute_user', {
                muterId: currentUser.id,
                mutedId
            });
            await this.loadMutedUsers();
        } catch (error) {
            console.error('Failed to unmute user:', error);
            alert('Failed to unmute user: ' + error);
        }
    },

    async loadMutedUsers() {
        if (!currentUser) return;

        try {
            const muted = await TauriAPI.invoke('get_muted_users', { userId: currentUser.id });
            const container = document.getElementById('mutedUsersList');
            const countEl = document.getElementById('mutedCount');

            if (countEl) countEl.textContent = muted.length;

            if (!container) return;

            if (muted.length === 0) {
                container.innerHTML = '<p style="color: var(--color-text-muted); font-size: var(--font-size-xs); text-align: center; padding: var(--spacing-sm);">No muted users</p>';
            } else {
                container.innerHTML = muted.map(user => {
                    const flags = [];
                    if (user.muteNotifications) flags.push('🔔');
                    if (user.muteMessages) flags.push('💬');
                    if (user.mutePosts) flags.push('📰');
                    const expiry = user.expiresAt ? `until ${new Date(user.expiresAt).toLocaleDateString()}` : 'forever';

                    return `
                        <div class="safety-item">
                            <div class="safety-item-info">
                                <span class="safety-item-name">${Utils.escapeHtml(getDisplayName(user.mutedId))}</span>
                                <span class="safety-item-details">${flags.join(' ')} · ${expiry}</span>
                            </div>
                            <button class="btn btn-sm btn-secondary" onclick="SafetyManager.unmuteUser('${user.mutedId}')">Unmute</button>
                        </div>
                    `;
                }).join('');
            }
        } catch (error) {
            console.error('Failed to load muted users:', error);
        }
    },

    // Load all safety settings
    async loadAll() {
        await Promise.all([
            this.loadBlockedUsers(),
            this.loadMutedUsers()
        ]);
    }
};

// Device Manager - Device management functionality
const DeviceManager = {
    async loadDevices() {
        if (!currentUser) return;

        try {
            const devices = await TauriAPI.invoke('get_user_devices', { userId: currentUser.id });
            const container = document.getElementById('devicesList');
            if (!container) return;

            if (devices.length === 0) {
                container.innerHTML = '<p style="color: var(--color-text-muted); font-size: var(--font-size-xs); text-align: center; padding: var(--spacing-sm);">No devices found</p>';
            } else {
                container.innerHTML = devices.map(device => {
                    const isCurrentDevice = device.id === currentUser.deviceId;
                    const lastSync = device.lastSync ? PostInteractions.formatTimeAgo(new Date(device.lastSync)) : 'Never';
                    const deviceIcon = device.deviceName?.toLowerCase().includes('iphone') ? '📱' :
                                       device.deviceName?.toLowerCase().includes('ipad') ? '📱' :
                                       device.deviceName?.toLowerCase().includes('mac') ? '💻' : '📱';

                    return `
                        <div class="device-item">
                            <div class="device-info">
                                <span class="device-icon">${deviceIcon}</span>
                                <div class="device-details">
                                    <span class="device-name">${Utils.escapeHtml(device.deviceName || 'Unknown Device')} ${isCurrentDevice ? '(This device)' : ''}</span>
                                    <span class="device-sync">Last active: ${isCurrentDevice ? 'Now' : lastSync}</span>
                                </div>
                            </div>
                            <div class="device-actions">
                                <button class="btn btn-sm btn-secondary" onclick="DeviceManager.renameDevice('${device.id}')">Rename</button>
                                ${!isCurrentDevice ? `<button class="btn btn-sm btn-danger" onclick="DeviceManager.removeDevice('${device.id}')">Remove</button>` : ''}
                            </div>
                        </div>
                    `;
                }).join('');
            }
        } catch (error) {
            console.error('Failed to load devices:', error);
        }
    },

    async renameDevice(deviceId) {
        const newName = prompt('Enter new device name:');
        if (!newName || !newName.trim()) return;

        try {
            await TauriAPI.invoke('update_device_name', {
                deviceId,
                deviceName: newName.trim()
            });
            await this.loadDevices();
        } catch (error) {
            console.error('Failed to rename device:', error);
            alert('Failed to rename device: ' + error);
        }
    },

    async removeDevice(deviceId) {
        if (!confirm('Remove this device? It will need to be re-authenticated to use your account.')) return;

        try {
            await TauriAPI.invoke('remove_device', { deviceId });
            await this.loadDevices();
        } catch (error) {
            console.error('Failed to remove device:', error);
            alert('Failed to remove device: ' + error);
        }
    }
};

// Recent Contacts - Quick access to recently messaged contacts
const RecentContacts = {
    async load() {
        if (!currentUser) return;

        const container = document.getElementById('recentContacts');
        if (!container) return;

        try {
            // Get messages and extract unique contacts
            const messages = await TauriAPI.invoke('get_messages_for_user', { userId: currentUser.id });

            // Build a map of contact -> most recent message time
            const contactMap = new Map();
            for (const msg of messages) {
                const contactId = msg.senderId === currentUser.id ? msg.recipientId : msg.senderId;
                const msgTime = new Date(msg.createdAt).getTime();

                if (!contactMap.has(contactId) || contactMap.get(contactId).time < msgTime) {
                    contactMap.set(contactId, { id: contactId, time: msgTime });
                }
            }

            // Sort by most recent and take top 5
            const recentContactIds = Array.from(contactMap.values())
                .sort((a, b) => b.time - a.time)
                .slice(0, 5)
                .map(c => c.id);

            if (recentContactIds.length === 0) {
                container.innerHTML = '<span class="no-recent">No recent contacts</span>';
                return;
            }

            // Get user details for each contact
            const contacts = await Promise.all(recentContactIds.map(async (id) => {
                try {
                    const user = await TauriAPI.invoke('get_user_by_id', { userId: id });
                    return user;
                } catch {
                    return null;
                }
            }));

            const validContacts = contacts.filter(c => c);

            container.innerHTML = validContacts.map(contact => `
                <button class="recent-contact-chip" onclick="RecentContacts.select('${contact.id}')" title="${Utils.escapeHtml(contact.displayName)}">
                    <span class="recent-contact-avatar">${this.getInitials(contact.displayName)}</span>
                    <span class="recent-contact-name">${Utils.escapeHtml(contact.displayName)}</span>
                </button>
            `).join('');
        } catch (error) {
            console.error('Failed to load recent contacts:', error);
            container.innerHTML = '<span class="no-recent">Failed to load</span>';
        }
    },

    getInitials(name) {
        if (!name) return '?';
        const parts = name.trim().split(/\s+/);
        if (parts.length >= 2) {
            return (parts[0][0] + parts[1][0]).toUpperCase();
        }
        return name.slice(0, 2).toUpperCase();
    },

    async select(userId) {
        // Get user details and add to selected recipients
        try {
            const user = await TauriAPI.invoke('get_user_by_id', { userId: userId });
            if (user) {
                // Check if already selected
                if (!selectedRecipients.find(r => r.id === user.id)) {
                    selectedRecipients.push(user);
                    updateSelectedRecipientsUI();
                }
                // Focus the message input
                document.getElementById('messageContent')?.focus();
            }
        } catch (error) {
            console.error('Failed to select contact:', error);
        }
    }
};

// Load functions
async function loadPosts() {
    try {
        console.log('[LOADPOSTS] Starting loadPosts, currentUser:', currentUser?.id);
        if (!currentUser) {
            console.log('[LOADPOSTS] No currentUser, returning early');
            return;
        }
        const posts = await TauriAPI.invoke('get_all_posts', { userId: currentUser.id });
        console.log('[LOADPOSTS] Got posts:', posts?.length);
        const postsContainer = document.getElementById('posts');
        const postsStatusMessage = document.getElementById('postsStatusMessage');

        if (posts.length === 0) {
            postsContainer.innerHTML = '';
            postsStatusMessage.innerHTML = `
                <div style="text-align: center; padding: var(--spacing-3xl) var(--spacing-lg);">
                    <h2 style="color: var(--color-text-primary); margin-bottom: var(--spacing-lg); font-size: var(--font-size-2xl);">No Posts Yet</h2>
                    <p style="color: var(--color-text-secondary); margin-bottom: var(--spacing-xl); font-size: var(--font-size-lg);">Share your thoughts or connect with friends to see their posts</p>
                    <div style="display: flex; flex-direction: column; gap: var(--spacing-md); align-items: center;">
                        <button class="btn btn-primary" onclick="showCreatePostPage()" style="max-width: 200px;">Create Post</button>
                        <button class="btn btn-secondary" onclick="showFriends()" style="max-width: 200px;">Add Friends</button>
                    </div>
                </div>
            `;
        } else {
            postsStatusMessage.innerHTML = '';
            // Load posts with media and reactions
            const postsWithData = await Promise.all(posts.map(async post => {
                const mediaAttachments = await PostManager.getMediaAttachments(post.id);
                const reactionSummary = await PostInteractions.getReactionSummary(post.id);
                const commentCount = await PostInteractions.getCommentCount(post.id);
                const userReaction = await PostInteractions.getUserReaction(post.id);
                return { ...post, mediaAttachments, reactionSummary, commentCount, userReaction };
            }));

            postsContainer.innerHTML = postsWithData.map(post => `
                <div class="post glass-card hover-lift-md" data-post-id="${post.id}">
                    <div class="post-header">
                        <div class="post-meta">
                            ${post.displayName || 'Unknown User'} • ${new Date(post.createdAt).toLocaleDateString()}
                        </div>
                        ${post.userId === currentUser?.id ? `
                            <button class="post-menu-btn" onclick="PostInteractions.showPostMenu(event, '${post.id}')" title="More options">⋯</button>
                        ` : ''}
                    </div>
                    ${post.mediaAttachments && post.mediaAttachments.length > 0 ? `
                        <div class="post-media">
                            ${post.mediaAttachments.map(media => PostManager.createMediaPreview(media)).join('')}
                        </div>
                    ` : ''}
                    <div class="post-content">${Utils.escapeHtml(post.content)}</div>

                    <!-- Post Footer: Reactions + Actions -->
                    <div class="post-footer">
                        <div class="post-reactions-bar">
                            <div class="reactions-summary" id="reactions-${post.id}">
                                ${PostInteractions.renderReactionSummary(post.reactionSummary, post.userReaction)}
                            </div>
                            <button class="reaction-add-btn" onclick="PostInteractions.showReactionPicker(event, '${post.id}')" title="Add reaction">
                                <span class="reaction-icon">+</span>
                            </button>
                        </div>
                        <div class="post-actions">
                            <button class="post-action-btn" onclick="PostInteractions.toggleComments('${post.id}')">
                                <span class="action-icon">💬</span>
                                <span class="action-count">${post.commentCount || 0}</span>
                            </button>
                            <button class="post-action-btn" onclick="PostInteractions.showShareModal('${post.id}')">
                                <span class="action-icon">↗️</span>
                                <span class="action-text">Share</span>
                            </button>
                        </div>
                    </div>

                    <!-- Comments Section (hidden by default) -->
                    <div class="post-comments-section hidden" id="comments-section-${post.id}">
                        <div class="comments-list" id="comments-list-${post.id}">
                            <!-- Comments loaded dynamically -->
                        </div>
                        <div class="comment-input-wrapper">
                            <input type="text" class="comment-input" id="comment-input-${post.id}" placeholder="Write a comment..." onkeypress="if(event.key==='Enter') PostInteractions.submitComment('${post.id}')">
                            <button class="comment-submit-btn" onclick="PostInteractions.submitComment('${post.id}')">Post</button>
                        </div>
                    </div>
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

    // Load recent contacts quick access
    RecentContacts.load();

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
                                ${getDisplayName(message.senderId)}
                                → ${getDisplayName(message.recipientId)}
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
                    // Calculate remaining time for disappearing messages
                    const disappearInfo = message.disappearsAt ? getDisappearTimeRemaining(message.disappearsAt) : null;
                    return `
                        <div class="post glass-card hover-lift-md ${disappearInfo ? 'disappearing-message' : ''}" data-message-id="${message.id}">
                            <div class="post-meta">
                                ${getDisplayName(message.senderId)}
                                → ${getDisplayName(message.recipientId)}
                                • ${new Date(message.createdAt).toLocaleDateString()}
                                ${message.encrypted ? ' • 🔒 Encrypted' : ''}
                                ${disappearInfo ? ` • <span class="disappear-timer" title="Disappears in ${disappearInfo}">⏱️ ${disappearInfo}</span>` : ''}
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
        const threadMessages = await TauriAPI.invoke('get_message_thread', { threadId: threadId });

        // Display thread in a modal or expanded view
        const threadHtml = threadMessages.map(message => `
            <div class="thread-message">
                <div class="post-meta">
                    ${getDisplayName(message.senderId)}
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
    if (selectedRecipients.length === 0) {
        UI.showError('dashboardError', 'Please select at least one recipient for the voice message');
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

            // Send to all selected recipients
            const sendPromises = selectedRecipients.map(recipient =>
                TauriAPI.invoke('send_voice_message', {
                    senderId: currentUser.id,
                    recipientId: recipient.id,
                    audioData: base64Audio,
                    durationSeconds: duration,
                    waveform: waveform,
                    threadId: replyToId ? parseInt(replyToId) : null
                })
            );

            await Promise.all(sendPromises);

            const count = selectedRecipients.length;
            UI.showSuccess('dashboardError', `Voice message sent to ${count} recipient${count > 1 ? 's' : ''}!`);
            clearSelectedRecipients();
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
    console.log('[LOAD-FRIENDS] loadFriends() called');
    console.log('[LOAD-FRIENDS] currentUser:', currentUser ? currentUser.displayName : 'null');

    if (!currentUser) {
        console.log('[LOAD-FRIENDS] No current user, returning early');
        return;
    }

    console.log('[LOAD-FRIENDS] Starting to load friends...');
    try {
        console.log('[LOAD-FRIENDS] Calling Tauri API for pending/outgoing/friends...');
        // Load pending (incoming), outgoing, and accepted friends
        const [pendingRequests, outgoingRequests, friends] = await Promise.all([
            TauriAPI.invoke('get_pending_friend_requests', { userId: currentUser.id }),
            TauriAPI.invoke('get_outgoing_friend_requests', { userId: currentUser.id }),
            TauriAPI.invoke('get_friends', { userId: currentUser.id })
        ]);

        const friendsContainer = document.getElementById('friends');
        let html = '';

        // Show pending incoming friend requests first (with Accept/Decline)
        if (pendingRequests && pendingRequests.length > 0) {
            console.log('[FRIEND-REQUESTS] Raw pending requests:', JSON.stringify(pendingRequests, null, 2));
            html += '<div class="friend-requests-section">';
            html += `<h3>Pending Friend Requests (${pendingRequests.length})</h3>`;
            html += pendingRequests.map(request => {
                console.log('[FRIEND-REQUESTS] Request object:', request);
                console.log('[FRIEND-REQUESTS] Request ID:', request.id, 'type:', typeof request.id);
                return `
                <div class="friend-request-card">
                    <div class="friend-request-badge">Friend Request</div>
                    <div class="friend-request-username">${Utils.escapeHtml(request.displayName || 'Unknown User')}</div>
                    <div class="friend-request-message">wants to connect with you</div>
                    <div class="public-key-display">
                        ${request.publicKey ? request.publicKey.substring(0, 32) + '...' : 'No public key'}
                    </div>
                    <div class="friend-request-actions">
                        <button class="btn btn-accept" data-accept-friend="${request.id}">Accept</button>
                        <button class="btn btn-reject" data-reject-friend="${request.id}">Decline</button>
                    </div>
                </div>
            `;
            }).join('');
            html += '</div>';
        }

        // Show outgoing friend requests (with Cancel button)
        if (outgoingRequests && outgoingRequests.length > 0) {
            console.log('[FRIEND-REQUESTS] Outgoing requests:', JSON.stringify(outgoingRequests, null, 2));
            html += '<div class="friend-requests-section outgoing-requests">';
            html += `<h3>Sent Requests (${outgoingRequests.length})</h3>`;
            html += outgoingRequests.map(request => {
                return `
                <div class="friend-request-card outgoing">
                    <div class="friend-request-badge pending-badge">Pending</div>
                    <div class="friend-request-username">${Utils.escapeHtml(request.displayName || 'Unknown User')}</div>
                    <div class="friend-request-message">waiting for response</div>
                    <div class="public-key-display">
                        ${request.publicKey ? request.publicKey.substring(0, 32) + '...' : 'No public key'}
                    </div>
                    <div class="friend-request-actions">
                        <button class="btn btn-reject" data-cancel-friend="${request.id}">Cancel</button>
                    </div>
                </div>
            `;
            }).join('');
            html += '</div>';
        }

        // Show accepted friends
        const hasPendingOrOutgoing = (pendingRequests && pendingRequests.length > 0) || (outgoingRequests && outgoingRequests.length > 0);
        if (friends.length === 0 && !hasPendingOrOutgoing) {
            html += `
                <div class="friends-empty">
                    <div class="friends-empty-icon">👥</div>
                    <div class="friends-empty-title">No friends yet</div>
                    <div class="friends-empty-message">Add friends using invite codes to start connecting!</div>
                </div>
            `;
        } else if (friends.length > 0) {
            html += '<div class="friends-section"><h3>Friends</h3>';
            html += friends.map(friend => {
                const initial = (friend.displayName || 'U').charAt(0).toUpperCase();
                return `
                <div class="friend-card">
                    <div class="friend-avatar">${initial}</div>
                    <div class="friend-info">
                        <div class="friend-name">${Utils.escapeHtml(friend.displayName || 'Unknown')}</div>
                        <div class="friend-meta">Added ${friend.createdAt ? new Date(friend.createdAt).toLocaleDateString() : 'Unknown'}</div>
                        <div class="public-key-display" style="margin-top: var(--spacing-xs);">
                            ${friend.publicKey ? friend.publicKey.substring(0, 24) + '...' : 'No public key'}
                        </div>
                    </div>
                </div>`;
            }).join('');
            html += '</div>';
        }

        friendsContainer.innerHTML = html;
        console.log('[LOAD-FRIENDS] Friends HTML updated, calling generateMyQRCode...');

        await generateMyQRCode();
        console.log('[LOAD-FRIENDS] generateMyQRCode completed');
        setTimeout(() => UI.updateModalLayout(document.getElementById('friendsContent')), 100);
    } catch (error) {
        console.error('[LOAD-FRIENDS] ERROR:', error);
        UI.showError('dashboardError', 'Failed to load friends: ' + error);
    }
}

async function acceptFriendRequest(friendUserId) {
    console.log('[ACCEPT-FRIEND] acceptFriendRequest called with friendUserId:', friendUserId);
    console.log('[ACCEPT-FRIEND] currentUser:', currentUser);

    if (!currentUser) {
        console.error('[ACCEPT-FRIEND] No current user!');
        return;
    }

    try {
        console.log('[ACCEPT-FRIEND] Calling accept_friend_request with:', {
            userId: currentUser.id,
            friendUserId: friendUserId
        });
        await TauriAPI.invoke('accept_friend_request', {
            userId: currentUser.id,
            friendUserId: friendUserId
        });
        console.log('[ACCEPT-FRIEND] Friend request accepted!');
        // Reload the friends list to show updated state
        await loadFriends();
    } catch (error) {
        console.error('[ACCEPT-FRIEND] Failed to accept friend request:', error);
        alert('Failed to accept friend request: ' + error);
    }
}

async function rejectFriendRequest(friendUserId) {
    if (!currentUser) return;

    try {
        await TauriAPI.invoke('reject_friend_request', {
            userId: currentUser.id,
            friendUserId: friendUserId
        });
        console.log('Friend request rejected');
        // Reload the friends list to show updated state
        await loadFriends();
    } catch (error) {
        console.error('Failed to reject friend request:', error);
        alert('Failed to reject friend request: ' + error);
    }
}

async function cancelFriendRequest(friendUserId) {
    console.log('[CANCEL-FRIEND] cancelFriendRequest called with friendUserId:', friendUserId);
    if (!currentUser) {
        console.error('[CANCEL-FRIEND] No current user!');
        return;
    }

    try {
        await TauriAPI.invoke('cancel_friend_request', {
            userId: currentUser.id,
            friendUserId: friendUserId
        });
        console.log('[CANCEL-FRIEND] Friend request canceled');
        // Reload the friends list to show updated state
        await loadFriends();
    } catch (error) {
        console.error('[CANCEL-FRIEND] Failed to cancel friend request:', error);
        alert('Failed to cancel friend request: ' + error);
    }
}

// Window-scoped handlers for inline onclick (most reliable in Tauri WebViews)
window.handleAcceptFriend = function(friendId) {
    console.log('[WINDOW-HANDLER] Accept button clicked for friendId:', friendId);
    acceptFriendRequest(friendId);
};

window.handleRejectFriend = function(friendId) {
    console.log('[WINDOW-HANDLER] Reject button clicked for friendId:', friendId);
    rejectFriendRequest(friendId);
};

window.handleCancelFriend = function(friendId) {
    console.log('[WINDOW-HANDLER] Cancel button clicked for friendId:', friendId);
    cancelFriendRequest(friendId);
};

// Event delegation for friend request buttons - handles dynamically added elements
document.addEventListener('click', function(event) {
    // Check for accept button
    const acceptBtn = event.target.closest('[data-accept-friend]');
    if (acceptBtn) {
        event.preventDefault();
        event.stopPropagation();
        const friendId = acceptBtn.dataset.acceptFriend;
        console.log('[DELEGATION] Accept button clicked for friendId:', friendId);
        acceptFriendRequest(friendId);
        return;
    }

    // Check for reject button
    const rejectBtn = event.target.closest('[data-reject-friend]');
    if (rejectBtn) {
        event.preventDefault();
        event.stopPropagation();
        const friendId = rejectBtn.dataset.rejectFriend;
        console.log('[DELEGATION] Reject button clicked for friendId:', friendId);
        rejectFriendRequest(friendId);
        return;
    }

    // Check for cancel button (outgoing requests)
    const cancelBtn = event.target.closest('[data-cancel-friend]');
    if (cancelBtn) {
        event.preventDefault();
        event.stopPropagation();
        const friendId = cancelBtn.dataset.cancelFriend;
        console.log('[DELEGATION] Cancel button clicked for friendId:', friendId);
        cancelFriendRequest(friendId);
        return;
    }
});
console.log('[MAIN.JS] Event delegation for friend request buttons initialized');

// Set up Tauri event listeners for P2P notifications
function setupTauriEventListeners() {
    if (!window.__TAURI__ || !window.__TAURI__.event) {
        console.log('[EVENTS] Tauri event API not available');
        return;
    }

    const { listen } = window.__TAURI__.event;

    // Listen for incoming friend requests
    listen('friend-request-received', (event) => {
        console.log('[EVENT] Friend request received:', event.payload);
        // Refresh friends list if on friends tab
        const friendsTab = document.getElementById('friendsTab');
        if (friendsTab && !friendsTab.classList.contains('hidden')) {
            loadFriends();
        }
    });

    // Listen for friend request acceptances
    listen('friend-accepted', (event) => {
        console.log('[EVENT] Friend request accepted:', event.payload);
        // Refresh friends list if on friends tab
        const friendsTab = document.getElementById('friendsTab');
        if (friendsTab && !friendsTab.classList.contains('hidden')) {
            loadFriends();
        }
        // Also refresh posts since we can now see their posts
        const postsTab = document.getElementById('postsTab');
        if (postsTab && !postsTab.classList.contains('hidden')) {
            loadPosts();
        }
    });

    // Listen for friend name changes (security feature)
    listen('friend-name-changed', (event) => {
        console.log('[EVENT] Friend name changed:', event.payload);
        const { oldName, newName, signatureValid, warning, message } = event.payload;

        if (warning) {
            // Security warning - invalid signature, possible impersonation
            UI.showToast(`⚠️ SECURITY WARNING: "${oldName}" changed to "${newName}" with INVALID signature! ${message || ''}`, 'error', 10000);
        } else if (signatureValid) {
            // Legitimate name change with valid signature
            UI.showToast(`ℹ️ "${oldName}" changed their name to "${newName}"`, 'info', 5000);
        }

        // Refresh friends list to show new name
        const friendsTab = document.getElementById('friendsTab');
        if (friendsTab && !friendsTab.classList.contains('hidden')) {
            loadFriends();
        }
    });

    // Listen for decrypted posts from sealed envelopes (Phase 2 encryption)
    listen('sealed-post-received', async (event) => {
        console.log('[EVENT] Sealed post received:', event.payload);
        const { user_id, public_key, content, timestamp, attachments } = event.payload;

        try {
            // Save the post to database
            const savedPost = await TauriAPI.invoke('create_post', {
                userId: user_id,
                content: content,
                attachments: null
            });
            console.log('[EVENT] Sealed post saved:', savedPost.id);

            // Save attachments if present
            if (attachments && attachments.length > 0) {
                for (const attachment of attachments) {
                    await TauriAPI.invoke('upload_media_file', {
                        fileData: attachment.data,
                        filename: 'synced_file',
                        fileType: attachment.file_type,
                        fileSize: attachment.file_size,
                        postId: savedPost.id
                    });
                }
            }

            // Refresh posts if on posts tab
            const postsTab = document.getElementById('postsTab');
            if (postsTab && !postsTab.classList.contains('hidden')) {
                loadPosts();
            }
        } catch (error) {
            console.error('[EVENT] Error saving sealed post:', error);
        }
    });

    // Listen for device sync completion
    listen('device-sync-completed', (event) => {
        console.log('[EVENT] Device sync completed from:', event.payload);
        // Refresh posts and friends after sync
        const postsTab = document.getElementById('postsTab');
        if (postsTab && !postsTab.classList.contains('hidden')) {
            loadPosts();
        }
        const friendsTab = document.getElementById('friendsTab');
        if (friendsTab && !friendsTab.classList.contains('hidden')) {
            loadFriends();
        }
    });

    console.log('[EVENTS] Tauri event listeners registered');
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
    const friendsList = document.getElementById('friendsList');

    if (!friendsList) return;

    // Render the friends list
    renderFriendsList();

    // Filter on input
    if (searchInput) {
        searchInput.addEventListener('input', function() {
            renderFriendsList(this.value.toLowerCase().trim());
        });
    }
}

function renderFriendsList(filter = '') {
    const friendsList = document.getElementById('friendsList');
    if (!friendsList) return;

    if (allFriends.length === 0) {
        friendsList.innerHTML = '<div class="friends-list-empty">No friends yet. Add friends from the Friends tab.</div>';
        return;
    }

    // Filter friends if search query provided
    const filteredFriends = filter
        ? allFriends.filter(friend => {
            const name = (friend.displayName || '').toLowerCase();
            const pubKey = (friend.publicKey || '').toLowerCase();
            return name.includes(filter) || pubKey.startsWith(filter);
        })
        : allFriends;

    if (filteredFriends.length === 0) {
        friendsList.innerHTML = '<div class="friends-list-empty">No friends match your filter</div>';
        return;
    }

    friendsList.innerHTML = filteredFriends.map(friend => {
        const name = friend.displayName || 'Unknown';
        const initial = name.charAt(0).toUpperCase();
        const isSelected = selectedRecipients.some(r => r.id === friend.id);
        return `
            <div class="friend-select-item ${isSelected ? 'selected' : ''}" onclick="toggleFriendSelection('${friend.id}', '${Utils.escapeHtml(name)}')">
                <div class="friend-avatar">${initial}</div>
                <span class="friend-name">${Utils.escapeHtml(name)}</span>
                ${isSelected ? '<span class="friend-check">✓</span>' : ''}
            </div>
        `;
    }).join('');
}

function toggleFriendSelection(friendId, displayName) {
    const existingIndex = selectedRecipients.findIndex(r => r.id === friendId);
    if (existingIndex >= 0) {
        // Remove if already selected
        selectedRecipients.splice(existingIndex, 1);
    } else {
        // Add if not selected
        selectedRecipients.push({ id: friendId, displayName: displayName });
    }
    updateSelectedRecipientsUI();
    renderFriendsList(document.getElementById('friendSearch')?.value?.toLowerCase().trim() || '');
}

function selectFriend(friendId, displayName) {
    // Don't add duplicates
    if (selectedRecipients.some(r => r.id === friendId)) {
        document.getElementById('friendSearch').value = '';
        document.getElementById('friendSearchResults').classList.add('hidden');
        return;
    }

    selectedRecipients.push({ id: friendId, displayName: displayName });
    updateSelectedRecipientsUI();

    document.getElementById('friendSearch').value = '';
    document.getElementById('friendSearchResults').classList.add('hidden');
}

function removeRecipient(friendId) {
    selectedRecipients = selectedRecipients.filter(r => r.id !== friendId);
    updateSelectedRecipientsUI();
}

function updateSelectedRecipientsUI() {
    const selectedContainer = document.getElementById('selectedRecipient');

    if (selectedRecipients.length === 0) {
        selectedContainer.innerHTML = '';
        selectedContainer.style.display = 'none';
        return;
    }

    selectedContainer.style.display = 'flex';
    selectedContainer.innerHTML = selectedRecipients.map(recipient => `
        <div class="recipient-chip">
            <span class="recipient-name">${Utils.escapeHtml(recipient.displayName)}</span>
            <button class="recipient-remove" onclick="removeRecipient('${recipient.id}')" title="Remove">×</button>
        </div>
    `).join('');
}

function clearSelectedRecipients() {
    selectedRecipients = [];
    updateSelectedRecipientsUI();
}

async function sendMessage() {
    if (!currentUser) return;

    const messageContentInput = document.getElementById('messageContent');
    const content = messageContentInput.value.trim();
    const replyToId = messageContentInput.getAttribute('data-reply-to');

    // Get disappearing message timer value
    const timerSelect = document.getElementById('messageTimer');
    const disappearAfterSeconds = timerSelect ? parseInt(timerSelect.value) || null : null;

    console.log('[SEND_MESSAGE] selectedRecipients:', selectedRecipients);
    console.log('[SEND_MESSAGE] content:', content);
    console.log('[SEND_MESSAGE] currentUser.id:', currentUser.id);
    console.log('[SEND_MESSAGE] disappearAfterSeconds:', disappearAfterSeconds);

    if (selectedRecipients.length === 0 || !content) {
        UI.showError('dashboardError', 'Please select at least one recipient and enter a message');
        return;
    }

    try {
        // Send to all selected recipients
        const sendPromises = selectedRecipients.map(async (recipient) => {
            console.log('[SEND_MESSAGE] Sending to recipient:', recipient);
            if (replyToId) {
                // Send as a reply using the reply_to_message command
                await TauriAPI.invoke('reply_to_message', {
                    originalMessageId: parseInt(replyToId),
                    senderId: currentUser.id,
                    recipientId: recipient.id,
                    content: content
                });
            } else {
                // Send as a regular message with optional disappearing timer
                await TauriAPI.invoke('send_encrypted_message', {
                    senderId: currentUser.id,
                    recipientId: recipient.id,
                    content: content,
                    disappearAfterSeconds: disappearAfterSeconds
                });
            }
        });

        await Promise.all(sendPromises);

        const recipientCount = selectedRecipients.length;
        messageContentInput.value = '';
        messageContentInput.placeholder = 'Enter your message';
        messageContentInput.removeAttribute('data-reply-to');

        // Reset timer to Off after sending
        if (timerSelect) timerSelect.value = '0';

        clearSelectedRecipients();

        let successMsg = replyToId
            ? 'Reply sent successfully!'
            : `Message sent to ${recipientCount} recipient${recipientCount > 1 ? 's' : ''}!`;
        if (disappearAfterSeconds) {
            successMsg += ' (disappears after ' + formatDisappearTime(disappearAfterSeconds) + ')';
        }
        UI.showSuccess('dashboardError', successMsg);
        loadMessages();
    } catch (error) {
        UI.showError('dashboardError', 'Failed to send message: ' + error);
    }
}

// Format disappearing time for display
function formatDisappearTime(seconds) {
    if (seconds < 60) return seconds + 's';
    if (seconds < 3600) return Math.floor(seconds / 60) + 'm';
    if (seconds < 86400) return Math.floor(seconds / 3600) + 'h';
    return Math.floor(seconds / 86400) + 'd';
}

// Get remaining time until message disappears
function getDisappearTimeRemaining(disappearsAt) {
    if (!disappearsAt) return null;
    const now = new Date();
    const expires = new Date(disappearsAt);
    const diffMs = expires - now;
    if (diffMs <= 0) return 'expired';
    const diffSeconds = Math.floor(diffMs / 1000);
    return formatDisappearTime(diffSeconds);
}

// Friend management
async function addFriendFromTab() {
    console.log('[ADD_FRIEND_TAB] Function called');
    if (!currentUser) {
        console.log('[ADD_FRIEND_TAB] No current user');
        return;
    }

    const errorEl = document.getElementById('addFriendTabError');
    const successEl = document.getElementById('addFriendTabSuccess');
    const addBtn = document.getElementById('addFriendBtn');
    errorEl.classList.add('hidden');
    successEl.classList.add('hidden');

    const inputValue = document.getElementById('addFriendPublicKey').value.trim();
    console.log('[ADD_FRIEND_TAB] Input:', inputValue);

    if (!inputValue) {
        errorEl.textContent = 'Please paste an invite link';
        errorEl.classList.remove('hidden');
        return;
    }

    // Accept both old (cipher://add-friend?...) and new (cipher://f/...) formats
    if (!inputValue.startsWith('cipher://')) {
        errorEl.textContent = 'Invalid invite link. Must start with cipher://';
        errorEl.classList.remove('hidden');
        return;
    }

    // Disable button during processing
    if (addBtn) {
        addBtn.disabled = true;
        addBtn.textContent = 'Adding...';
    }

    try {
        // Parse the invite using Rust backend (handles both old and new formats)
        const parsed = await TauriAPI.invoke('parse_invite_code', { inviteCode: inputValue });
        console.log('[ADD_FRIEND_TAB] Parsed invite:', parsed);

        if (parsed.publicKey === currentUser.publicKey) {
            errorEl.textContent = 'You cannot add yourself as a friend';
            errorEl.classList.remove('hidden');
            if (addBtn) { addBtn.disabled = false; addBtn.textContent = 'Add Friend'; }
            return;
        }

        console.log('[ADD_FRIEND_TAB] Adding friend:', {
            publicKey: parsed.publicKey,
            nodeId: parsed.nodeId,
            relayUrl: parsed.relayUrl,
            displayName: parsed.displayName
        });

        // Use the P2P-enabled add friend command
        await TauriAPI.invoke('iroh_add_friend_by_public_key', {
            friendPublicKey: parsed.publicKey,
            nodeId: parsed.nodeId,
            relayUrl: parsed.relayUrl || null,
            displayName: parsed.displayName || null,
            signature: parsed.signature || null
        });

        document.getElementById('addFriendPublicKey').value = '';
        successEl.textContent = 'Friend added successfully!';
        successEl.classList.remove('hidden');

        // Keep button disabled after success
        if (addBtn) addBtn.textContent = 'Friend Added ✓';

        // Go back to friends list after a short delay
        setTimeout(() => {
            showFriends();
            // Reset button when navigating away
            if (addBtn) { addBtn.disabled = false; addBtn.textContent = 'Add Friend'; }
        }, 1500);
    } catch (error) {
        console.log('[ADD_FRIEND_TAB] Error:', error);
        errorEl.textContent = 'Failed to add friend: ' + error;
        errorEl.classList.remove('hidden');
        // Re-enable button on error
        if (addBtn) { addBtn.disabled = false; addBtn.textContent = 'Add Friend'; }
    }
}

// QR Code functions
async function generateQRCode(containerId, options = {}) {
    if (!currentUser) return;

    const { maxWidth = '200px', showSuccess = false } = options;
    const qrContainer = document.getElementById(containerId);

    try {
        console.log('═══════════════════════════════════════════════════════════════');
        console.log('🔵 FRONTEND: QR CODE GENERATION STARTED');
        console.log('═══════════════════════════════════════════════════════════════');

        // Show loading state while waiting for P2P
        if (qrContainer) {
            qrContainer.innerHTML = '<p style="color: var(--color-text-muted); text-align: center;">Connecting to P2P network...</p>';
        }

        console.log('[QR-GEN] Calling P2P.generateInvite() (waits for P2P initialization)...');
        // Use P2P.generateInvite() which properly waits for P2P initialization
        const inviteCode = await P2P.generateInvite();
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

        if (qrContainer) {
            qrContainer.innerHTML = `<img src="${qrCodeDataUrl}" alt="Your QR Code" style="max-width: ${maxWidth}; max-height: ${maxWidth}; border-radius: var(--border-radius-md);">`;
        }

        if (showSuccess) {
            UI.showSuccess('dashboardError', 'QR code generated successfully!');
        }
    } catch (error) {
        console.error('[QR] Failed to generate QR code:', error);

        // Show user-friendly error in the QR container
        if (qrContainer) {
            qrContainer.innerHTML = '<p style="color: var(--color-error); text-align: center;">P2P network not ready. Please wait a moment and try again.</p>';
        }

        if (showSuccess) {
            UI.showError('dashboardError', 'Failed to generate QR code: ' + error);
        }
    }
}

// Convenience wrappers for backward compatibility
// Store the last generated invite code for copy functionality
let lastGeneratedInviteCode = null;

async function generateMyQRCode() {
    console.log('[QR-GEN] generateMyQRCode() called');

    if (!currentUser) {
        console.log('[QR-GEN] No current user, returning early');
        return;
    }

    console.log('[QR-GEN] Current user:', currentUser.displayName);
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

        // Store for copy functionality
        lastGeneratedInviteCode = inviteCode;

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

        // Show the copy button
        const copyBtn = document.getElementById('copyInviteLinkBtn');
        if (copyBtn) {
            copyBtn.style.display = 'inline-block';
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

        // Hide the copy button on error
        const copyBtn = document.getElementById('copyInviteLinkBtn');
        if (copyBtn) {
            copyBtn.style.display = 'none';
        }
    }
}

async function copyInviteLink() {
    if (!lastGeneratedInviteCode) {
        console.log('[COPY] No invite code available');
        return;
    }

    try {
        await navigator.clipboard.writeText(lastGeneratedInviteCode);
        console.log('[COPY] Invite link copied to clipboard');

        // Show feedback
        const copyBtn = document.getElementById('copyInviteLinkBtn');
        if (copyBtn) {
            const originalText = copyBtn.textContent;
            copyBtn.textContent = 'Copied!';
            copyBtn.disabled = true;
            setTimeout(() => {
                copyBtn.textContent = originalText;
                copyBtn.disabled = false;
            }, 2000);
        }
    } catch (error) {
        console.error('[COPY] Failed to copy:', error);
        // Fallback for older browsers
        const textArea = document.createElement('textarea');
        textArea.value = lastGeneratedInviteCode;
        textArea.style.position = 'fixed';
        textArea.style.left = '-9999px';
        document.body.appendChild(textArea);
        textArea.select();
        try {
            document.execCommand('copy');
            const copyBtn = document.getElementById('copyInviteLinkBtn');
            if (copyBtn) {
                const originalText = copyBtn.textContent;
                copyBtn.textContent = 'Copied!';
                copyBtn.disabled = true;
                setTimeout(() => {
                    copyBtn.textContent = originalText;
                    copyBtn.disabled = false;
                }, 2000);
            }
        } catch (e) {
            console.error('[COPY] Fallback copy failed:', e);
        }
        document.body.removeChild(textArea);
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

                // Parse invite code (supports both old and new compressed formats)
                if (!result.content.startsWith('cipher://')) {
                    UI.showError('dashboardError', 'Invalid QR code. Must be a Cipher invite link.');
                    return;
                }

                try {
                    // Use Rust backend to parse invite (handles compression/decompression)
                    const parsed = await TauriAPI.invoke('parse_invite_code', { inviteCode: result.content });
                    console.log('[QR] Parsed invite:', parsed);

                    // Add friend by public key with node info
                    console.log('[QR] Adding friend by public key...');
                    const addedPublicKey = await TauriAPI.invoke('iroh_add_friend_by_public_key', {
                        friendPublicKey: parsed.publicKey,
                        nodeId: parsed.nodeId,
                        relayUrl: parsed.relayUrl || null,
                        displayName: parsed.displayName || null,
                        signature: parsed.signature || null
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

        if (qrCodeData && qrCodeData.displayName && qrCodeData.publicKey) {
            document.getElementById('friendPublicKey').value = qrCodeData.publicKey;
            await addFriendByQRCode(qrCodeData.displayName, qrCodeData.publicKey);
        } else {
            UI.showError('dashboardError', 'Invalid QR code or QR code does not contain friend data');
        }
    } catch (error) {
        UI.showError('dashboardError', 'Failed to scan QR code: ' + error);
    }

    event.target.value = '';
}

async function addFriendByQRCode(displayName, publicKey, peerId, peerAddr) {
    if (!currentUser) return;

    if (publicKey === currentUser.publicKey) {
        UI.showError('dashboardError', 'You cannot add yourself as a friend');
        return;
    }

    try {
        const friend = await TauriAPI.invoke('get_user_by_public_key', { publicKey: publicKey });

        if (!friend) {
            UI.showError('dashboardError', `No user found with display name ${displayName}`);
            return;
        }

        await TauriAPI.invoke('add_friend', {
            userId: currentUser.id,
            friendUserId: friend.id
        });

        UI.showSuccess('dashboardError', `Successfully added ${displayName} as a friend!`);
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

            UI.showSuccess('dashboardError', `Successfully added ${friend.displayName} as a friend!`);
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

        // Set up Tauri event listeners for P2P events
        setupTauriEventListeners();

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
            const contents = ['postsContent', 'messagesContent', 'friendsContent', 'profileContent', 'settingsContent'];
            const tabs = ['postsTab', 'messagesTab', 'friendsTab', 'profileTab', 'settingsTab'];

            contents.forEach((contentId, index) => {
                const content = document.getElementById(contentId);
                const tab = document.getElementById(tabs[index]);
                if (content && tab && !tab.classList.contains('hidden')) {
                    UI.updateModalLayout(content);
                }
            });
        });

        // Handle app coming back from background - check P2P health and announce presence
        document.addEventListener('visibilitychange', async () => {
            if (!document.hidden && P2P.initialized) {
                console.log('App became visible, checking P2P health and announcing presence...');
                try {
                    // Ensure Rust-side P2P is still initialized (may have been reset on mobile)
                    await P2P.ensureInitialized();
                    await P2P.announcePresence();
                } catch (error) {
                    console.error('Failed to restore P2P or announce presence:', error);
                }
            }
        });

        // Handle page focus (additional safeguard for mobile)
        window.addEventListener('focus', async () => {
            if (P2P.initialized && currentUser) {
                console.log('Window focused, checking P2P health and announcing presence...');
                try {
                    await P2P.ensureInitialized();
                    await P2P.announcePresence();
                } catch (error) {
                    console.error('Failed to restore P2P or announce presence:', error);
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

// Note: Login form uses form onsubmit handlers (handleCreateAccount, handleRestoreAccount)
// No additional Enter key listener needed - forms handle Enter key automatically

// New QR Code and P2P Invite Functions
let cameraStream = null;
let scanningInterval = null;

async function openCameraToScanQR() {
    // Check if we're on mobile (Android/iOS) - use native Tauri scanner
    const isMobile = window.__TAURI__?.core && (
        navigator.userAgent.includes('Android') ||
        navigator.userAgent.includes('iPhone') ||
        navigator.userAgent.includes('iPad')
    );

    if (isMobile) {
        try {
            console.log('[QR] Using native Tauri barcode scanner');
            // Use Tauri's native barcode scanner plugin
            const { scan, cancel } = window.__TAURI__.barcodescanner || {};

            if (!scan) {
                // Try invoking directly if plugin not in global scope
                const result = await TauriAPI.invoke('plugin:barcode-scanner|scan', {
                    windowed: false,
                    formats: ['QR_CODE']
                });

                if (result && result.content) {
                    console.log('[QR] Native scan result:', result.content);
                    await handleScannedQRCode(result.content);
                }
                return;
            }

            const result = await scan({ windowed: false, formats: ['QR_CODE'] });
            if (result && result.content) {
                console.log('[QR] Native scan result:', result.content);
                await handleScannedQRCode(result.content);
            }
        } catch (error) {
            console.error('[QR] Native scanner error:', error);
            // Fallback to web-based scanner
            await openWebCameraScanner();
        }
        return;
    }

    // Desktop: use web-based camera scanner
    await openWebCameraScanner();
}

async function openWebCameraScanner() {
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

        // Parse invite code (supports both old and new compressed formats)
        if (!data.startsWith('cipher://')) {
            throw new Error('Invalid QR code format - must be a Cipher invite link');
        }

        // Use Rust backend to parse invite (handles compression/decompression)
        const parsed = await TauriAPI.invoke('parse_invite_code', { inviteCode: data });
        console.log('[QR-SCAN] Parsed invite:', parsed);

        // Add friend by public key with node info
        console.log('[QR-SCAN] Adding friend by public key...');
        const addedPublicKey = await TauriAPI.invoke('iroh_add_friend_by_public_key', {
            friendPublicKey: parsed.publicKey,
            nodeId: parsed.nodeId,
            relayUrl: parsed.relayUrl || null,
            displayName: parsed.displayName || null,
            signature: parsed.signature || null
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

            if (qrCodeData && qrCodeData.displayName && qrCodeData.publicKey) {
                await addFriendByQRCode(qrCodeData.displayName, qrCodeData.publicKey);
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
    // Show the Add Friend modal
    const modal = document.getElementById('addFriendModal');
    const input = document.getElementById('addFriendModalPublicKey');
    const errorEl = document.getElementById('addFriendModalError');
    const successEl = document.getElementById('addFriendModalSuccess');

    // Clear previous state
    if (input) input.value = '';
    if (errorEl) {
        errorEl.textContent = '';
        errorEl.classList.add('hidden');
    }
    if (successEl) {
        successEl.textContent = '';
        successEl.classList.add('hidden');
    }

    modal.classList.remove('hidden');

    // Focus the input after a short delay
    setTimeout(() => {
        if (input) input.focus();
    }, 100);
}

function closeAddFriendModal(event) {
    // If called with event, only close if clicking the backdrop
    if (event && event.target !== event.currentTarget) {
        return;
    }
    const modal = document.getElementById('addFriendModal');
    modal.classList.add('hidden');
}

async function addFriendFromModal() {
    if (!currentUser) {
        console.log('[ADD_FRIEND_MODAL] No current user');
        return;
    }

    const input = document.getElementById('addFriendModalPublicKey');
    const errorEl = document.getElementById('addFriendModalError');
    const successEl = document.getElementById('addFriendModalSuccess');

    const inputValue = input.value.trim();

    // Hide previous messages
    errorEl.classList.add('hidden');
    successEl.classList.add('hidden');

    if (!inputValue) {
        errorEl.textContent = 'Please enter a valid input';
        errorEl.classList.remove('hidden');
        return;
    }

    // Accept both old (cipher://add-friend?...) and new (cipher://f/...) formats
    if (!inputValue.startsWith('cipher://')) {
        errorEl.textContent = 'Invalid invite link. Must start with cipher://';
        errorEl.classList.remove('hidden');
        return;
    }

    try {
        // Parse the invite using Rust backend (handles both old and new formats)
        const parsed = await TauriAPI.invoke('parse_invite_code', { inviteCode: inputValue });
        console.log('[ADD_FRIEND_MODAL] Parsed invite:', parsed);

        if (parsed.publicKey === currentUser.publicKey) {
            errorEl.textContent = 'You cannot add yourself as a friend';
            errorEl.classList.remove('hidden');
            return;
        }

        console.log('[ADD_FRIEND_MODAL] Adding friend:', {
            publicKey: parsed.publicKey,
            nodeId: parsed.nodeId,
            relayUrl: parsed.relayUrl,
            displayName: parsed.displayName
        });

        // Use the P2P-enabled add friend command
        await TauriAPI.invoke('iroh_add_friend_by_public_key', {
            friendPublicKey: parsed.publicKey,
            nodeId: parsed.nodeId,
            relayUrl: parsed.relayUrl || null,
            displayName: parsed.displayName || null,
            signature: parsed.signature || null
        });

        successEl.textContent = 'Friend added successfully!';
        successEl.classList.remove('hidden');
        input.value = '';

        // Close modal after a short delay and refresh friends list
        setTimeout(() => {
            closeAddFriendModal();
            loadFriends();
        }, 1500);
    } catch (error) {
        errorEl.textContent = 'Failed to add friend: ' + error;
        errorEl.classList.remove('hidden');
    }
}

// Connection Status Modal Functions
let connectionStatusInterval = null;

async function showConnectionStatus() {
    const modal = document.getElementById('connectionStatusModal');
    modal.classList.remove('hidden');
    await refreshConnectionStatus(true); // Show loading on first load

    // Start real-time updates every 1 second
    if (connectionStatusInterval) {
        clearInterval(connectionStatusInterval);
    }
    connectionStatusInterval = setInterval(async () => {
        // Only update if modal is still visible
        if (!modal.classList.contains('hidden')) {
            await refreshConnectionStatus();
        } else {
            clearInterval(connectionStatusInterval);
            connectionStatusInterval = null;
        }
    }, 1000);
}

function closeConnectionStatus(event) {
    // If called with event, only close if clicking the backdrop
    if (event && event.target !== event.currentTarget) return;
    const modal = document.getElementById('connectionStatusModal');
    modal.classList.add('hidden');

    // Stop real-time updates
    if (connectionStatusInterval) {
        clearInterval(connectionStatusInterval);
        connectionStatusInterval = null;
    }
}

async function refreshConnectionStatus(showLoading = false) {
    const body = document.getElementById('connectionStatusBody');
    if (showLoading) {
        body.innerHTML = '<p style="text-align: center; color: var(--color-text-secondary);">Loading...</p>';
    }

    try {
        // Add timeout to prevent hanging forever
        const timeoutPromise = new Promise((_, reject) =>
            setTimeout(() => reject(new Error('Timeout')), 5000)
        );
        const status = await Promise.race([
            TauriAPI.invoke('iroh_get_connection_status'),
            timeoutPromise
        ]);
        console.log('[CONNECTION-STATUS] Received:', status);

        const truncateId = (id, len = 8) => {
            if (!id) return 'N/A';
            return id.length > len * 2 ? `${id.substring(0, len)}...${id.substring(id.length - 4)}` : id;
        };

        // Online = listening AND has connected peers (consistent with navbar)
        const isOnline = status.listening && (status.connected_peers || 0) > 0;
        const isConnecting = status.listening && (status.connected_peers || 0) === 0 &&
            P2P.connectionStartTime && (Date.now() - P2P.connectionStartTime) < P2P.connectionGracePeriod;
        const statusClass = isOnline ? 'online' : (isConnecting ? 'connecting' : 'offline');
        const statusText = isOnline ? 'Online' : (isConnecting ? 'Connecting...' : (status.listening ? 'Offline (No Peers)' : 'Offline'));
        let html = `
            <div class="status-section">
                <div class="status-section-title">Network Status</div>
                <div class="status-row">
                    <span class="status-label">Status</span>
                    <span class="status-value ${statusClass}">
                        ${statusText}
                    </span>
                </div>
                <div class="status-row">
                    <span class="status-label">Connected Peers</span>
                    <span class="status-value">${status.connected_peers || 0}</span>
                </div>
                <div class="status-row">
                    <span class="status-label">Active Peers</span>
                    <span class="status-value">${status.active_peers?.length || 0}</span>
                </div>
                <div class="status-row">
                    <span class="status-label">Subscribed Topics</span>
                    <span class="status-value">${status.topic_count || 0}</span>
                </div>
            </div>

            <div class="status-section">
                <div class="status-section-title">Identity</div>
                <div class="status-row">
                    <span class="status-label">Node ID</span>
                    <span class="status-value" title="${status.node_id || ''}">${truncateId(status.node_id, 12)}</span>
                </div>
                <div class="status-row">
                    <span class="status-label">Public Key</span>
                    <span class="status-value" title="${status.public_key || ''}">${truncateId(status.public_key, 12)}</span>
                </div>
                <div class="status-row">
                    <span class="status-label">Device ID</span>
                    <span class="status-value">${status.device_id || 'N/A'}</span>
                </div>
            </div>

            <div class="status-section">
                <div class="status-section-title">Relay</div>
                <div class="status-row">
                    <span class="status-label">Relay URL</span>
                    <span class="status-value">${status.relay_url ? status.relay_url.replace('https://', '').replace(/\/$/, '') : 'N/A'}</span>
                </div>
            </div>
        `;

        // Connected Peers section
        if (status.peer_ids && status.peer_ids.length > 0) {
            html += `
                <div class="status-section">
                    <div class="status-section-title">Connected Peers (${status.peer_ids.length})</div>
                    <div class="status-list">
                        ${status.peer_ids.map(id => `<div class="status-list-item">${truncateId(id, 16)}</div>`).join('')}
                    </div>
                </div>
            `;
        }

        // Subscribed Topics section
        if (status.subscribed_topics && status.subscribed_topics.length > 0) {
            html += `
                <div class="status-section">
                    <div class="status-section-title">Subscribed Topics (${status.subscribed_topics.length})</div>
                    <div class="status-list">
                        ${status.subscribed_topics.map(topic => `<div class="status-list-item">${topic}</div>`).join('')}
                    </div>
                </div>
            `;
        }

        body.innerHTML = html;
    } catch (error) {
        console.error('[CONNECTION-STATUS] Error:', error);
        body.innerHTML = `
            <div class="status-section">
                <div class="status-section-title">Network Status</div>
                <div class="status-row">
                    <span class="status-label">Status</span>
                    <span class="status-value offline">Not Connected</span>
                </div>
                <p class="status-empty">P2P network not initialized. Log in to connect.</p>
            </div>
        `;
    }
}

// ============================================
// Community Functions
// ============================================

// Current community being viewed
let currentCommunityId = null;

// Show communities list
async function showCommunities() {
    UI.hideAllTabs();
    document.getElementById('communitiesTab').classList.remove('hidden');
    Navbar.setActiveLink('communitiesNavLink');
    await loadCommunities();
}

// Load communities list
async function loadCommunities() {
    const container = document.getElementById('communitiesList');
    if (!currentUser) return;

    try {
        const communities = await TauriAPI.invoke('get_my_communities', { userId: currentUser.id });

        if (!communities || communities.length === 0) {
            container.innerHTML = '<p style="text-align: center; color: var(--color-text-muted); padding: var(--spacing-xl);">No communities yet. Create one or join with an invite code.</p>';
            return;
        }

        container.innerHTML = communities.map(c => `
            <div class="friend-item" style="cursor: pointer;" onclick="showCommunityDetail('${c.id}')">
                <div class="friend-info">
                    <div class="friend-name" style="font-weight: 600;">${Utils.escapeHtml(c.name)}</div>
                    <div class="friend-username" style="font-size: var(--font-size-sm); color: var(--color-text-secondary);">
                        ${c.memberCount} member${c.memberCount !== 1 ? 's' : ''}
                    </div>
                </div>
                <span style="color: var(--color-text-muted);">→</span>
            </div>
        `).join('');
    } catch (error) {
        console.error('Failed to load communities:', error);
        container.innerHTML = '<p style="text-align: center; color: var(--color-error);">Failed to load communities</p>';
    }
}

// Show create community modal
function showCreateCommunityModal() {
    console.log('[COMMUNITY] showCreateCommunityModal called');
    const modal = document.getElementById('createCommunityModal');
    console.log('[COMMUNITY] Modal element:', modal);
    if (modal) {
        modal.classList.remove('hidden');
        console.log('[COMMUNITY] Modal classes after remove hidden:', modal.className);
    } else {
        console.error('[COMMUNITY] createCommunityModal element not found!');
    }
    document.getElementById('newCommunityName').value = '';
    document.getElementById('newCommunityDescription').value = '';
    document.getElementById('createCommunityError').classList.add('hidden');
}

function closeCreateCommunityModal() {
    document.getElementById('createCommunityModal').classList.add('hidden');
}

// Create a new community
async function createCommunity() {
    console.log('[COMMUNITY] createCommunity called');
    const name = document.getElementById('newCommunityName').value.trim();
    const description = document.getElementById('newCommunityDescription').value.trim() || null;
    const errorEl = document.getElementById('createCommunityError');

    if (!name) {
        errorEl.textContent = 'Please enter a community name';
        errorEl.classList.remove('hidden');
        return;
    }

    if (!currentUser) {
        console.error('[COMMUNITY] No user found');
        errorEl.textContent = 'Please log in first';
        errorEl.classList.remove('hidden');
        return;
    }

    try {
        console.log('[COMMUNITY] Creating community:', name);
        const community = await TauriAPI.invoke('create_community', {
            userId: currentUser.id,
            name,
            description
        });
        console.log('[COMMUNITY] Community created:', community);

        closeCreateCommunityModal();
        await loadCommunities();
        showCommunityDetail(community.id);
    } catch (error) {
        console.error('[COMMUNITY] Failed to create community:', error);
        errorEl.textContent = error.toString();
        errorEl.classList.remove('hidden');
    }
}

// Show community detail view
async function showCommunityDetail(communityId) {
    currentCommunityId = communityId;
    UI.hideAllTabs();
    document.getElementById('communityDetailTab').classList.remove('hidden');
    Navbar.setActiveLink('communitiesNavLink');

    try {
        const data = await TauriAPI.invoke('get_community', { communityId });
        if (!data) {
            showCommunities();
            return;
        }

        document.getElementById('communityDetailName').textContent = data.community.name;
        document.getElementById('communityDetailDescription').textContent = data.community.description || '';

        await loadCommunityFeed(communityId);
    } catch (error) {
        console.error('Failed to load community:', error);
        showCommunities();
    }
}

// Load community feed
async function loadCommunityFeed(communityId) {
    const container = document.getElementById('communityFeed');
    console.log('[COMMUNITY] Loading feed for community:', communityId);

    try {
        const posts = await TauriAPI.invoke('get_community_feed', { communityId });
        console.log('[COMMUNITY] Received posts:', posts?.length || 0);

        if (!posts || posts.length === 0) {
            container.innerHTML = '<p style="text-align: center; color: var(--color-text-muted); padding: var(--spacing-xl);">No posts yet. Be the first to share something!</p>';
            return;
        }

        // Load media attachments for each post
        const postsWithMedia = await Promise.all(posts.map(async post => {
            const mediaAttachments = await PostManager.getMediaAttachments(post.id);
            return { ...post, mediaAttachments };
        }));

        container.innerHTML = postsWithMedia.map(post => `
            <div class="post" style="max-width: 600px; margin: 0 auto var(--spacing-md) auto;">
                <div class="post-header">
                    <span class="post-author">${Utils.escapeHtml(post.displayName || 'Unknown')}</span>
                    <span class="post-time">${PostInteractions.formatTimeAgo(new Date(post.createdAt))}</span>
                </div>
                ${post.mediaAttachments && post.mediaAttachments.length > 0 ? `
                    <div class="post-media" style="margin: var(--spacing-sm) 0;">
                        ${post.mediaAttachments.map(media => PostManager.createMediaPreview(media)).join('')}
                    </div>
                ` : ''}
                ${post.content ? `<div class="post-content">${Utils.escapeHtml(post.content)}</div>` : ''}
            </div>
        `).join('');
    } catch (error) {
        console.error('Failed to load community feed:', error);
        container.innerHTML = `<p style="text-align: center; color: var(--color-error);">Failed to load posts: ${error}</p>`;
    }
}

// Preview community post attachments
function previewCommunityAttachments(event) {
    const files = event.target.files;
    const previewContainer = document.getElementById('communityAttachmentPreview');

    if (!files || files.length === 0) {
        previewContainer.style.display = 'none';
        previewContainer.innerHTML = '';
        return;
    }

    previewContainer.style.display = 'flex';
    previewContainer.style.flexWrap = 'wrap';
    previewContainer.style.gap = 'var(--spacing-sm)';
    previewContainer.innerHTML = '';

    for (let i = 0; i < files.length; i++) {
        const file = files[i];
        const preview = document.createElement('div');
        preview.style.cssText = 'position: relative; width: 80px; height: 80px; border-radius: var(--radius-md); overflow: hidden; background: var(--glass-bg);';

        if (file.type.startsWith('image/')) {
            const img = document.createElement('img');
            img.style.cssText = 'width: 100%; height: 100%; object-fit: cover;';
            img.src = URL.createObjectURL(file);
            preview.appendChild(img);
        } else if (file.type.startsWith('video/')) {
            preview.innerHTML = '<div style="display: flex; align-items: center; justify-content: center; height: 100%; color: var(--color-text-secondary);">🎬</div>';
        }

        // Add remove button
        const removeBtn = document.createElement('button');
        removeBtn.innerHTML = '×';
        removeBtn.style.cssText = 'position: absolute; top: 2px; right: 2px; background: rgba(0,0,0,0.7); color: white; border: none; border-radius: 50%; width: 20px; height: 20px; cursor: pointer; font-size: 14px; line-height: 1;';
        removeBtn.onclick = () => clearCommunityAttachments();
        preview.appendChild(removeBtn);

        previewContainer.appendChild(preview);
    }
}

function clearCommunityAttachments() {
    const fileInput = document.getElementById('communityPostAttachments');
    const previewContainer = document.getElementById('communityAttachmentPreview');
    fileInput.value = '';
    previewContainer.style.display = 'none';
    previewContainer.innerHTML = '';
}

// Create a post in the current community
async function createCommunityPost() {
    if (!currentCommunityId) {
        console.error('[COMMUNITY] No currentCommunityId set');
        return;
    }

    const content = document.getElementById('communityPostContent').value.trim();
    const fileInput = document.getElementById('communityPostAttachments');
    const hasFiles = fileInput && fileInput.files && fileInput.files.length > 0;

    // Require either content or attachments
    if (!content && !hasFiles) return;

    if (!currentUser) {
        console.error('[COMMUNITY] No currentUser set');
        return;
    }

    try {
        console.log('[COMMUNITY] Creating post in community:', currentCommunityId, 'by user:', currentUser.id);

        // Create the post (always show in main feed)
        const post = await TauriAPI.invoke('create_community_post', {
            communityId: currentCommunityId,
            userId: currentUser.id,
            content: content || '',
            showInMainFeed: true
        });
        console.log('[COMMUNITY] Post created:', post.id);

        // Upload attachments if any
        if (hasFiles) {
            console.log('[COMMUNITY] Uploading', fileInput.files.length, 'attachments');
            await PostManager.uploadAttachments(post.id, fileInput.files);
        }

        // Publish to community members via P2P (non-critical, don't block)
        try {
            await TauriAPI.invoke('publish_community_post', {
                communityId: currentCommunityId,
                postId: post.id
            });
            console.log('[COMMUNITY] Post published via P2P');
        } catch (publishError) {
            console.warn('[COMMUNITY] P2P publish failed (non-critical):', publishError);
        }

        // Clear input and reload feed
        document.getElementById('communityPostContent').value = '';
        clearCommunityAttachments();
        await loadCommunityFeed(currentCommunityId);
    } catch (error) {
        console.error('Failed to create community post:', error);
        alert('Failed to create post: ' + error);
    }
}

// Join community by invite code
async function joinCommunityByInvite() {
    const inviteCode = document.getElementById('communityInviteCode').value.trim().toUpperCase();
    if (!inviteCode) return;

    if (!currentUser) return;

    try {
        const community = await TauriAPI.invoke('join_community_by_invite', {
            userId: currentUser.id,
            inviteCode
        });

        if (community) {
            document.getElementById('communityInviteCode').value = '';

            // Announce ourselves to the community
            await TauriAPI.invoke('announce_community_member', {
                communityId: community.id,
                newMemberId: currentUser.id
            });

            await loadCommunities();
            showCommunityDetail(community.id);
        } else {
            alert('Invalid or expired invite code');
        }
    } catch (error) {
        console.error('Failed to join community:', error);
        alert('Failed to join community: ' + error);
    }
}

// Show community settings modal
async function showCommunitySettings() {
    if (!currentCommunityId) return;

    document.getElementById('communitySettingsModal').classList.remove('hidden');
    document.getElementById('communityInviteResult').classList.add('hidden');

    try {
        const members = await TauriAPI.invoke('get_community_members', { communityId: currentCommunityId });
        document.getElementById('communityMemberCount').textContent = members.length;

        const container = document.getElementById('communityMembersList');
        container.innerHTML = members.map(m => `
            <div style="padding: var(--spacing-sm); border-bottom: 1px solid var(--glass-border);">
                <span style="font-weight: ${m.role === 'creator' ? '600' : '400'};">
                    ${Utils.escapeHtml(m.displayName || 'Unknown')}
                </span>
                ${m.role === 'creator' ? '<span style="color: var(--color-primary); font-size: var(--font-size-sm);"> (creator)</span>' : ''}
            </div>
        `).join('');
    } catch (error) {
        console.error('Failed to load community members:', error);
    }
}

function closeCommunitySettings() {
    document.getElementById('communitySettingsModal').classList.add('hidden');
}

// Generate community invite
async function generateCommunityInvite() {
    if (!currentCommunityId) return;

    if (!currentUser) return;

    try {
        const invite = await TauriAPI.invoke('create_community_invite', {
            communityId: currentCommunityId,
            creatorId: currentUser.id,
            usesRemaining: 10, // Default: 10 uses
            hoursValid: 24 * 7 // 1 week
        });

        document.getElementById('communityInviteCodeDisplay').textContent = invite.inviteCode;
        document.getElementById('communityInviteResult').classList.remove('hidden');
    } catch (error) {
        console.error('Failed to generate invite:', error);
        alert('Failed to generate invite: ' + error);
    }
}

// Copy community invite code
function copyCommunityInvite() {
    const code = document.getElementById('communityInviteCodeDisplay').textContent;
    navigator.clipboard.writeText(code).then(() => {
        alert('Invite code copied!');
    });
}

// Leave the current community
async function leaveCommunity() {
    if (!currentCommunityId) return;

    if (!confirm('Are you sure you want to leave this community?')) return;

    if (!currentUser) return;

    try {
        const result = await TauriAPI.invoke('leave_community', {
            communityId: currentCommunityId,
            userId: currentUser.id
        });

        if (result) {
            closeCommunitySettings();
            currentCommunityId = null;
            showCommunities();
        } else {
            alert('Cannot leave community. You may be the creator.');
        }
    } catch (error) {
        console.error('Failed to leave community:', error);
        alert('Failed to leave community: ' + error);
    }
}

// Listen for community-related P2P events
if (typeof window.__TAURI__ !== 'undefined') {
    window.__TAURI__.event.listen('community-post-received', (event) => {
        console.log('Received community post:', event.payload);
        // Refresh feed if viewing the relevant community
        if (currentCommunityId && event.payload.communityId === currentCommunityId) {
            loadCommunityFeed(currentCommunityId);
        }
    });

    window.__TAURI__.event.listen('community-member-added', (event) => {
        console.log('New community member:', event.payload);
        // Refresh member list if viewing settings
        if (currentCommunityId && event.payload.communityId === currentCommunityId) {
            if (!document.getElementById('communitySettingsModal').classList.contains('hidden')) {
                showCommunitySettings();
            }
        }
    });
}

// Export functions to global scope for onclick handlers
Object.assign(window, {
    handleLogout, showLogin, showFeed, showPosts, showMessages,
    showFriends, showAddFriend, showCreatePostPage, showCreatePost, createPost, cancelCreatePost,
    createPostFromPage, sendMessage, addFriendFromTab,
    generateMyQRCode, generateProfileQRCode, scanQRCode, handleQRCodeFile, selectFriend, removeRecipient, toggleFriendSelection, copyInviteLink,
    viewMediaAttachment, showEditProfile, handleProfilePictureUpload, saveProfile,
    createFriendInvite, useFriendInvite, exportFriendsList, importFriendsList,
    searchMessages, clearMessageSearch, scrollToMessage, editMessage,
    cancelEditMessage, saveEditMessage, deleteMessage,
    // Hamburger menu functions
    toggleHamburgerMenu, closeHamburgerMenu,
    // New QR and P2P functions
    openCameraToScanQR, closeCameraScanner, handleQRCodeFromCamera, showMyQRCode, showManualAddFriend,
    // Add Friend modal functions
    closeAddFriendModal, addFriendFromModal,
    // Friend request functions
    acceptFriendRequest, rejectFriendRequest,
    // Connection status functions
    showConnectionStatus, closeConnectionStatus, refreshConnectionStatus,
    // Community functions
    showCommunities, showCreateCommunityModal, closeCreateCommunityModal, createCommunity,
    showCommunityDetail, createCommunityPost, joinCommunityByInvite,
    showCommunitySettings, closeCommunitySettings, generateCommunityInvite, copyCommunityInvite, leaveCommunity,
    // Post interactions
    PostInteractions,
    // Safety & Device management
    SafetyManager, DeviceManager
});

console.log('[MAIN.JS] Functions exported to window scope');