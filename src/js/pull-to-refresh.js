/**
 * Pull-to-Refresh Implementation
 * Allows users to trigger sync by pulling down from the top of the feed
 */

const PullToRefresh = {
    // Configuration
    threshold: 80, // Pixels to pull before triggering refresh
    maxPull: 120, // Maximum pull distance
    deadZone: 15, // Minimum pull before engaging (prevents accidental triggers)

    // State
    startY: 0,
    currentY: 0,
    pulling: false,
    canPull: false,
    engaged: false, // Whether pull-to-refresh has actually engaged (past dead zone)
    indicator: null,
    icon: null,
    text: null,
    activeContainer: null, // Track which container triggered the refresh

    // Initialize pull-to-refresh
    init() {
        console.log('Initializing pull-to-refresh');

        // Create indicator element
        this.createIndicator();

        // Add event listeners to scrollable containers
        const scrollableContainers = document.querySelectorAll('.modal-scrollable');
        scrollableContainers.forEach(container => {
            this.setupContainer(container);
        });

        console.log('Pull-to-refresh initialized');
    },

    // Create the pull-to-refresh indicator
    createIndicator() {
        this.indicator = document.createElement('div');
        this.indicator.className = 'pull-to-refresh-indicator';

        this.icon = document.createElement('div');
        this.icon.className = 'pull-to-refresh-icon';
        this.icon.textContent = '↓';

        this.text = document.createElement('div');
        this.text.className = 'pull-to-refresh-text';
        this.text.textContent = 'Pull down to sync';

        this.indicator.appendChild(this.icon);
        this.indicator.appendChild(this.text);

        document.body.appendChild(this.indicator);
    },

    // Setup event listeners on a container
    setupContainer(container) {
        // Touch events for mobile
        container.addEventListener('touchstart', (e) => this.handleTouchStart(e, container), { passive: true });
        container.addEventListener('touchmove', (e) => this.handleTouchMove(e, container), { passive: false });
        container.addEventListener('touchend', (e) => this.handleTouchEnd(e, container), { passive: true });

        // Mouse events for desktop testing
        container.addEventListener('mousedown', (e) => this.handleMouseDown(e, container));
        container.addEventListener('mousemove', (e) => this.handleMouseMove(e, container));
        container.addEventListener('mouseup', (e) => this.handleMouseUp(e, container));
    },

    // Check if container is scrolled to top (with small tolerance for Android)
    isAtTop(container) {
        return container.scrollTop <= 1;
    },

    // Touch event handlers
    handleTouchStart(e, container) {
        // Reset state for new touch
        this.canPull = false;
        this.pulling = false;
        this.engaged = false;

        // Only enable pull-to-refresh if starting at the very top
        if (this.isAtTop(container)) {
            this.canPull = true;
            this.startY = e.touches[0].clientY;
            this.activeContainer = container;
        }
    },

    handleTouchMove(e, container) {
        if (!this.canPull) return;

        this.currentY = e.touches[0].clientY;
        const pullDistance = this.currentY - this.startY;

        // If user scrolls up (negative pull), disable pull-to-refresh for this touch
        if (pullDistance < -5) {
            this.canPull = false;
            this.engaged = false;
            return;
        }

        // Only engage after passing the dead zone AND still at top
        if (pullDistance > this.deadZone && this.isAtTop(container)) {
            // Now we're engaged - prevent default scrolling
            if (!this.engaged) {
                this.engaged = true;
            }
            e.preventDefault();

            this.pulling = true;
            const effectiveDistance = pullDistance - this.deadZone;
            const clampedDistance = Math.min(effectiveDistance, this.maxPull);

            // Show indicator
            this.indicator.classList.add('visible');

            // Update indicator state
            if (clampedDistance >= this.threshold) {
                this.indicator.classList.add('pulling');
                this.text.textContent = 'Release to sync';
            } else {
                this.indicator.classList.remove('pulling');
                this.text.textContent = 'Pull down to sync';
            }
        } else if (this.engaged) {
            // Already engaged but moved back up - keep preventing default
            e.preventDefault();
        }
    },

    handleTouchEnd(e, container) {
        if (!this.pulling) {
            this.canPull = false;
            this.engaged = false;
            return;
        }

        const pullDistance = this.currentY - this.startY;
        const effectiveDistance = pullDistance - this.deadZone;

        if (effectiveDistance >= this.threshold) {
            // Trigger refresh
            this.triggerSync();
        } else {
            // Reset indicator
            this.resetIndicator();
        }

        this.pulling = false;
        this.canPull = false;
        this.engaged = false;
    },

    // Mouse event handlers (for desktop testing)
    handleMouseDown(e, container) {
        if (this.isAtTop(container)) {
            this.canPull = true;
            this.startY = e.clientY;
            this.activeContainer = container;
        }
    },

    handleMouseMove(e, container) {
        if (!this.canPull) return;

        this.currentY = e.clientY;
        const pullDistance = this.currentY - this.startY;

        if (pullDistance > 0 && this.isAtTop(container)) {
            this.pulling = true;
            const clampedDistance = Math.min(pullDistance, this.maxPull);

            // Show indicator
            this.indicator.classList.add('visible');

            // Update indicator state
            if (clampedDistance >= this.threshold) {
                this.indicator.classList.add('pulling');
                this.text.textContent = 'Release to sync';
            } else {
                this.indicator.classList.remove('pulling');
                this.text.textContent = 'Pull down to sync';
            }
        }
    },

    handleMouseUp(e, container) {
        if (!this.pulling) {
            this.canPull = false;
            return;
        }

        const pullDistance = this.currentY - this.startY;

        if (pullDistance >= this.threshold) {
            // Trigger refresh
            this.triggerSync();
        } else {
            // Reset indicator
            this.resetIndicator();
        }

        this.pulling = false;
        this.canPull = false;
    },

    // Trigger sync
    async triggerSync() {
        console.log('Pull-to-refresh: Triggering sync');

        // Update indicator to show syncing
        this.indicator.classList.remove('pulling');
        this.indicator.classList.add('syncing');
        this.text.textContent = 'Syncing...';

        // Scroll container back to top immediately
        if (this.activeContainer) {
            this.activeContainer.scrollTop = 0;
        }

        try {
            // Trigger P2P sync if available
            if (window.P2P && P2P.initialized) {
                await P2P.requestSync();

                // Show success
                this.text.textContent = 'Synced!';
                this.icon.textContent = '[OK]';

                setTimeout(() => {
                    this.resetIndicator();
                }, 1500);
            } else {
                // P2P not initialized
                this.text.textContent = 'P2P not connected';
                setTimeout(() => {
                    this.resetIndicator();
                }, 2000);
            }
        } catch (error) {
            console.error('Pull-to-refresh sync failed:', error);
            this.text.textContent = 'Sync failed';
            setTimeout(() => {
                this.resetIndicator();
            }, 2000);
        }
    },

    // Reset indicator to hidden state
    resetIndicator() {
        this.indicator.classList.remove('visible', 'pulling', 'syncing');
        this.icon.textContent = '↓';
        this.text.textContent = 'Pull down to sync';
        this.activeContainer = null;
        this.engaged = false;
    }
};

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => PullToRefresh.init());
} else {
    PullToRefresh.init();
}

// Re-initialize when navigating to tabs with scrollable content
window.addEventListener('tabchange', () => {
    setTimeout(() => {
        const scrollableContainers = document.querySelectorAll('.modal-scrollable');
        scrollableContainers.forEach(container => {
            // Only setup if not already set up
            if (!container.dataset.pullToRefreshInit) {
                PullToRefresh.setupContainer(container);
                container.dataset.pullToRefreshInit = 'true';
            }
        });
    }, 100);
});
