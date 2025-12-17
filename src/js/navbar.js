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
                        <!-- Invite Friend button (logged in only) -->
                        <button id="navInviteBtn" class="nav-invite-btn logged-in-only hidden" onclick="Navbar.copyInviteLink()" title="Copy invite link">
                            <span class="invite-icon">+</span>
                        </button>
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
                </div>

                <!-- Backdrop overlay -->
                <div class="nav-backdrop" id="navBackdrop" onclick="closeHamburgerMenu()"></div>

                <!-- Hamburger Menu Content -->
                <div class="nav-menu hidden" id="navMenu">
                    <div class="nav-section">
                        <h4 class="nav-section-title">Social</h4>
                        <div class="nav-links">
                            <a class="nav-link hover-slide active" id="postsNavLink" onclick="closeHamburgerMenu(); showFeed()">📰 Feed</a>
                            <a class="nav-link hover-slide" id="createPostNavLink" onclick="closeHamburgerMenu(); showCreatePostPage()">✏️ Create Post</a>
                            <a class="nav-link hover-slide" id="messagesNavLink" onclick="closeHamburgerMenu(); showMessages()">💬 Messages</a>
                            <a class="nav-link hover-slide" id="friendsNavLink" onclick="closeHamburgerMenu(); showFriends()">👥 Friends</a>
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
        const btn = document.getElementById('navInviteBtn');
        const originalContent = btn.innerHTML;

        try {
            // Show loading state
            btn.innerHTML = '<span class="invite-icon">...</span>';
            btn.disabled = true;

            // Generate invite code using P2P
            const inviteCode = await P2P.generateInvite();

            // Copy to clipboard
            await navigator.clipboard.writeText(inviteCode);

            // Show success state
            btn.innerHTML = '<span class="invite-icon">✓</span>';

            // Reset after 2 seconds
            setTimeout(() => {
                btn.innerHTML = originalContent;
                btn.disabled = false;
            }, 2000);

        } catch (error) {
            console.error('Failed to copy invite link:', error);
            btn.innerHTML = '<span class="invite-icon">!</span>';
            setTimeout(() => {
                btn.innerHTML = originalContent;
                btn.disabled = false;
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
    }
};

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = Navbar;
}
