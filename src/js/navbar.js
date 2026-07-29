// Navbar Component
// Provides a shared, reusable navigation bar for all pages

const Navbar = {
    // Get the navbar HTML template
    getTemplate: function() {
        return `
            <nav class="navbar" id="mainNavbar">
                <div class="nav-header">
                    <!-- Left side: Brand -->
                    <div class="nav-left">
                        <div class="nav-brand" id="navBrand" onclick="Navbar.goToFeed()" style="cursor: pointer;">🔐<span class="brand-text"> Cipher</span></div>
                    </div>
                    <!-- Right side controls -->
                    <div class="nav-controls">
                        <!-- Combined P2P & Sync Status (logged in only) -->
                        <div id="p2pStatus" class="p2p-status logged-in-only hidden" title="Click for connection details" onclick="showConnectionStatus()">
                            <span class="p2p-status-dot offline"></span>
                            <span class="p2p-status-text">Offline</span>
                        </div>
                        <!-- Theme toggle (always visible) -->
                        <label class="theme-toggle-button" for="color-mode-toggle">
                            <span class="sr-only">Toggle color theme</span>
                            <span aria-hidden="true" class="theme-toggle-button__icon"></span>
                        </label>
                        <!-- Hamburger menu (logged in only) -->
                        <button class="hamburger-menu logged-in-only hidden" id="hamburgerBtn" onclick="toggleHamburgerMenu()">
                            <span></span>
                            <span></span>
                            <span></span>
                        </button>
                    </div>

                    <!-- Notifications Panel -->
                    <div id="notificationsPanel" class="notifications-panel hidden">
                        <div class="notifications-header">
                            <h4>Notifications</h4>
                            <div class="notifications-header-actions">
                                <button class="btn-text" onclick="Navbar.markAllRead()">Mark All Read</button>
                                <button class="notifications-close" onclick="Navbar.closeNotifications()" title="Close">&times;</button>
                            </div>
                        </div>
                        <div id="notificationsList" class="notifications-list">
                            <p class="text-center text-muted">No notifications</p>
                        </div>
                    </div>
                </div>

                <!-- Backdrop overlay -->
                <div class="nav-backdrop" id="navBackdrop" onclick="closeHamburgerMenu()"></div>

                <!-- Hamburger Menu Content -->
                <div class="nav-menu hidden" id="navMenu">
                    <div class="nav-section">
                        <h4 class="nav-section-title">Social</h4>
                        <div class="nav-links">
                            <a class="nav-link hover-slide" id="postsNavLink" onclick="closeHamburgerMenu(); showFeed()">📰 Feed</a>
                            <a class="nav-link hover-slide" id="createPostNavLink" onclick="closeHamburgerMenu(); showCreatePostPage()">✏️ Create Post</a>
                            <a class="nav-link hover-slide" id="messagesNavLink" onclick="closeHamburgerMenu(); showMessages()">💬 Messages</a>
                            <a class="nav-link hover-slide" id="friendsNavLink" onclick="closeHamburgerMenu(); showFriends()">👥 Friends</a>
                            <a class="nav-link hover-slide" id="communitiesNavLink" onclick="closeHamburgerMenu(); showCommunities()">🏘️ Communities</a>
                        </div>
                    </div>

                    <div class="nav-section">
                        <h4 class="nav-section-title">Quick Actions</h4>
                        <div class="nav-links">
                            <a class="nav-action hover-slide" id="navInviteLink" onclick="Navbar.copyInviteLink()">
                                <span>➕ Copy Invite Link</span>
                                <span id="inviteCopyStatus" class="nav-link-status"></span>
                            </a>
                            <a class="nav-action hover-slide" id="navNotificationsLink" onclick="Navbar.toggleNotifications()">
                                <span>🔔 Notifications</span>
                                <span id="notificationBadge" class="notification-badge hidden">0</span>
                            </a>
                        </div>
                    </div>

                    <div class="nav-section">
                        <h4 class="nav-section-title">Account</h4>
                        <div class="nav-links">
                            <a class="nav-link hover-slide" id="settingsNavLink" onclick="closeHamburgerMenu(); showSettings()">⚙️ Settings</a>
                            <a class="nav-link hover-slide nav-signout" onclick="closeHamburgerMenu(); handleLogout()">🚪 Sign Out</a>
                        </div>
                    </div>

                    <div class="nav-section">
                        <h4 class="nav-section-title">Appearance</h4>
                        <div class="nav-links">
                            <label class="nav-link hover-slide" for="color-mode-toggle" style="cursor: pointer;" onclick="closeHamburgerMenu()">
                                <span class="theme-toggle-button__icon" style="display: inline; margin-right: var(--spacing-sm);"></span>
                                <span>Toggle Theme</span>
                            </label>
                        </div>
                    </div>
                </div>
            </nav>
        `;
    },

    // Initialize the navbar in a container
    init: function(containerId) {
        const container = document.getElementById(containerId);
        if (!container) {
            console.error('Navbar container not found:', containerId);
            return;
        }
        container.innerHTML = this.getTemplate();
    },

    // Update navbar state based on login status
    updateLoginState: function(isLoggedIn) {
        const navbar = document.getElementById('mainNavbar');
        const loggedInElements = document.querySelectorAll('.logged-in-only');

        // Toggle logged-in class on navbar
        if (navbar) {
            if (isLoggedIn) {
                navbar.classList.add('logged-in');
            } else {
                navbar.classList.remove('logged-in');
            }
        }

        // Toggle visibility of logged-in-only elements
        loggedInElements.forEach(element => {
            if (isLoggedIn) {
                element.classList.remove('hidden');
            } else {
                element.classList.add('hidden');
            }
        });
    },

    // Copy invite link to clipboard
    async copyInviteLink() {
        const statusEl = document.getElementById('inviteCopyStatus');

        try {
            // Show loading state
            if (statusEl) statusEl.textContent = '...';

            // Generate invite code using P2P
            const inviteCode = await P2P.generateInvite();

            // Copy to clipboard
            await navigator.clipboard.writeText(inviteCode);

            // Show success state
            if (statusEl) statusEl.textContent = 'Copied!';

            // Reset after 2 seconds
            setTimeout(() => {
                if (statusEl) statusEl.textContent = '';
            }, 2000);

        } catch (error) {
            console.error('Failed to copy invite link:', error);
            if (statusEl) statusEl.textContent = 'Failed';
            setTimeout(() => {
                if (statusEl) statusEl.textContent = '';
            }, 2000);
        }
    },

    // Update P2P status in navbar
    // status: 'online', 'offline', or 'connecting'
    updateP2PStatus: function(status, peerCount = 0) {
        const statusDot = document.querySelector('.p2p-status-dot');
        const statusText = document.querySelector('.p2p-status-text');

        if (statusDot && statusText) {
            // Remove all status classes first
            statusDot.classList.remove('online', 'offline', 'connecting');

            if (status === 'online' || status === true) {
                statusDot.classList.add('online');
                statusText.textContent = `Online (${peerCount})`;
            } else if (status === 'connecting') {
                statusDot.classList.add('connecting');
                statusText.textContent = 'Connecting...';
            } else {
                statusDot.classList.add('offline');
                statusText.textContent = 'Offline';
            }
        }
    },

    // Navigate to feed page (when clicking brand logo/text)
    goToFeed: function() {
        // Only navigate if logged in
        const navbar = document.getElementById('mainNavbar');
        if (navbar && navbar.classList.contains('logged-in')) {
            closeHamburgerMenu();
            showFeed();
        }
    },

    // Set active navigation link
    setActiveLink: function(linkId) {
        const navLinks = document.querySelectorAll('.nav-link');
        navLinks.forEach(link => {
            link.classList.remove('active');
        });

        const activeLink = document.getElementById(linkId);
        if (activeLink) {
            activeLink.classList.add('active');
        }
    },

    // Toggle notifications panel
    toggleNotifications: function() {
        const panel = document.getElementById('notificationsPanel');
        if (panel) {
            const isHidden = panel.classList.contains('hidden');
            if (isHidden) {
                this.loadNotifications();
                panel.classList.remove('hidden');
                // Close hamburger menu if open
                closeHamburgerMenu();
            } else {
                panel.classList.add('hidden');
            }
        }
    },

    // Close notifications panel
    closeNotifications: function() {
        const panel = document.getElementById('notificationsPanel');
        if (panel) {
            panel.classList.add('hidden');
        }
    },

    // Load notifications
    loadNotifications: async function() {
        if (typeof currentUser === 'undefined' || !currentUser) return;

        try {
            const notifications = await TauriAPI.invoke('get_notifications', { userId: currentUser.id });
            this.renderNotifications(notifications);
            this.updateBadge(notifications.filter(n => !n.read).length);
        } catch (error) {
            console.error('Failed to load notifications:', error);
        }
    },

    // Render notifications list
    renderNotifications: function(notifications) {
        const list = document.getElementById('notificationsList');
        if (!list) return;

        if (!notifications || notifications.length === 0) {
            list.innerHTML = '<p class="text-center text-muted">No notifications</p>';
            return;
        }

        list.innerHTML = notifications.map(notification => {
            const icon = this.getNotificationIcon(notification.notificationType);
            const timeAgo = this.formatTimeAgo(notification.createdAt);
            const unreadClass = notification.read ? '' : 'unread';

            // Notification fields are derived from peer activity. The old inline
            // onclick only escaped single quotes in `data` (and nothing at all in
            // id/type), so a backslash or a double quote broke out of the handler.
            // Everything now lives in data-* attributes read back via dataset.
            return `
                <div class="notification-item ${unreadClass}"
                     data-notification-id="${Utils.escapeHtml(notification.id)}"
                     data-notification-type="${Utils.escapeHtml(notification.notificationType)}"
                     data-notification-payload="${Utils.escapeHtml(notification.data == null ? '' : notification.data)}">
                    <div class="notification-icon">${Utils.escapeHtml(icon)}</div>
                    <div class="notification-content">
                        <div class="notification-title">${Utils.escapeHtml(notification.title)}</div>
                        <div class="notification-message">${Utils.escapeHtml(notification.message)}</div>
                        <div class="notification-time">${Utils.escapeHtml(timeAgo)}</div>
                    </div>
                    <button class="notification-dismiss" data-notification-dismiss="${Utils.escapeHtml(notification.id)}" title="Dismiss">×</button>
                </div>
            `;
        }).join('');

        this.bindNotificationDelegation();
    },

    // Delegated handlers for the notification list (installed once).
    bindNotificationDelegation: function() {
        const list = document.getElementById('notificationsList');
        if (!list || list.dataset.delegationBound === '1') return;
        list.dataset.delegationBound = '1';

        list.addEventListener('click', (event) => {
            const dismissBtn = event.target.closest('[data-notification-dismiss]');
            if (dismissBtn) {
                event.stopPropagation();
                Navbar.dismissNotification(dismissBtn.dataset.notificationDismiss);
                return;
            }

            const item = event.target.closest('[data-notification-id]');
            if (item) {
                Navbar.handleNotificationClick(
                    item.dataset.notificationId,
                    item.dataset.notificationType,
                    item.dataset.notificationPayload || null
                );
            }
        });
    },

    // Get icon for notification type
    getNotificationIcon: function(type) {
        const icons = {
            'friend_request': '👋',
            'friend_accepted': '🤝',
            'post_reaction': '👍',
            'post_comment': '💬',
            'post_share': '↗️',
            'community_invite': '🏘️',
            'community_post': '📰',
            'message': '✉️',
            'default': '🔔'
        };
        return icons[type] || icons['default'];
    },

    // Format time ago
    formatTimeAgo: function(dateString) {
        const date = new Date(dateString);
        const now = new Date();
        const diffMs = now - date;
        const diffMins = Math.floor(diffMs / 60000);
        const diffHours = Math.floor(diffMs / 3600000);
        const diffDays = Math.floor(diffMs / 86400000);

        if (diffMins < 1) return 'Just now';
        if (diffMins < 60) return diffMins + 'm ago';
        if (diffHours < 24) return diffHours + 'h ago';
        if (diffDays < 7) return diffDays + 'd ago';
        return date.toLocaleDateString();
    },

    // Update notification badge
    updateBadge: function(count) {
        const badge = document.getElementById('notificationBadge');
        if (badge) {
            if (count > 0) {
                badge.textContent = count > 99 ? '99+' : count;
                badge.classList.remove('hidden');
            } else {
                badge.classList.add('hidden');
            }
        }
    },

    // Handle notification click
    handleNotificationClick: async function(notificationId, type, data) {
        if (typeof currentUser === 'undefined' || !currentUser) return;

        try {
            // Mark as read
            await TauriAPI.invoke('mark_notification_read', {
                notificationId: notificationId,
                userId: currentUser.id
            });

            // Update UI
            const item = document.querySelector(`[data-notification-id="${notificationId}"]`);
            if (item) item.classList.remove('unread');

            // Refresh badge count
            const count = await TauriAPI.invoke('get_unread_notification_count', { userId: currentUser.id });
            this.updateBadge(count);

            // Navigate based on type
            this.closeNotifications();
            switch (type) {
                case 'friend_request':
                case 'friend_accepted':
                    showFriends();
                    break;
                case 'post_reaction':
                case 'post_comment':
                case 'post_share':
                case 'community_post':
                    showFeed();
                    break;
                case 'community_invite':
                    showCommunities();
                    break;
                case 'message':
                    showMessages();
                    break;
            }
        } catch (error) {
            console.error('Failed to handle notification:', error);
        }
    },

    // Dismiss notification
    dismissNotification: async function(notificationId) {
        if (typeof currentUser === 'undefined' || !currentUser) return;

        try {
            await TauriAPI.invoke('delete_notification', {
                notificationId: notificationId,
                userId: currentUser.id
            });

            // Remove from UI
            const item = document.querySelector(`[data-notification-id="${notificationId}"]`);
            if (item) {
                item.style.opacity = '0';
                item.style.transform = 'translateX(100%)';
                setTimeout(() => {
                    item.remove();
                    // Check if list is empty
                    const list = document.getElementById('notificationsList');
                    if (list && list.children.length === 0) {
                        list.innerHTML = '<p class="text-center text-muted">No notifications</p>';
                    }
                }, 200);
            }

            // Refresh badge
            const count = await TauriAPI.invoke('get_unread_notification_count', { userId: currentUser.id });
            this.updateBadge(count);
        } catch (error) {
            console.error('Failed to dismiss notification:', error);
        }
    },

    // Mark all notifications as read
    markAllRead: async function() {
        if (typeof currentUser === 'undefined' || !currentUser) return;

        try {
            await TauriAPI.invoke('mark_all_notifications_read', { userId: currentUser.id });

            // Update UI
            document.querySelectorAll('.notification-item.unread').forEach(item => {
                item.classList.remove('unread');
            });

            this.updateBadge(0);
        } catch (error) {
            console.error('Failed to mark all as read:', error);
        }
    },

    // Start notification polling (call when user logs in)
    startNotificationPolling: function() {
        // Load initial count
        this.loadNotifications();

        // Poll every 30 seconds
        if (this._pollInterval) clearInterval(this._pollInterval);
        this._pollInterval = setInterval(() => {
            if (typeof currentUser !== 'undefined' && currentUser) {
                this.refreshBadge();
            }
        }, 30000);
    },

    // Stop notification polling (call when user logs out)
    stopNotificationPolling: function() {
        if (this._pollInterval) {
            clearInterval(this._pollInterval);
            this._pollInterval = null;
        }
        this.updateBadge(0);
    },

    // Refresh just the badge count
    refreshBadge: async function() {
        if (typeof currentUser === 'undefined' || !currentUser) return;

        try {
            const count = await TauriAPI.invoke('get_unread_notification_count', { userId: currentUser.id });
            this.updateBadge(count);
        } catch (error) {
            console.error('Failed to refresh notification count:', error);
        }
    }
};

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = Navbar;
}
