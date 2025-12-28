// Iroh-based P2P Networking Module
const P2P = {
    peers: new Map(), // Map of peer_id -> peer info
    initialized: false,
    initializationPromise: null, // Promise that resolves when initialization completes
    initializationResolve: null, // Resolve function for the promise
    connectedPeers: 0, // Track number of connected peers
    presenceRetryCount: 0,
    presenceRetryTimer: null,
    maxPresenceRetries: 10,
    baseRetryDelay: 1000, // 1 second base delay
    connectionStartTime: null, // Track when we started connecting
    connectionGracePeriod: 15000, // Show "Connecting" for 15s before showing "Offline"

    // Update P2P status indicator in UI
    // status: 'online', 'offline', or 'connecting'
    updateStatus(status) {
        // Use Navbar module if available
        if (typeof Navbar !== 'undefined') {
            Navbar.updateP2PStatus(status, this.connectedPeers);
        }
    },

    // Initialize Iroh network (called on login)
    async initialize(userId, displayName, publicKey, deviceId, force = false) {
        if (this.initialized && !force) {
            console.log('P2P already initialized');
            return;
        }

        // Store credentials for potential re-initialization
        this.userId = userId;
        this.displayName = displayName;
        this.publicKey = publicKey;
        this.deviceId = deviceId;

        // Create a promise that other methods can wait on
        if (!this.initializationPromise || force) {
            this.initializationPromise = new Promise((resolve) => {
                this.initializationResolve = resolve;
            });
        }

        try {
            this.updateStatus('connecting');
            console.log('Initializing Iroh P2P system...');

            // Initialize Iroh network (keypair derived on Rust side from public key)
            await TauriAPI.invoke('iroh_initialize', {
                userId: userId,
                displayName: displayName,
                publicKey: publicKey,
                deviceId: deviceId
            });

            this.initialized = true;

            // Subscribe to all friends' topics
            try {
                const friends = await TauriAPI.invoke('get_friends', { userId: userId });
                console.log(`Found ${friends.length} friends, subscribing to their topics...`);

                for (const friend of friends) {
                    try {
                        await this.subscribeFriend(friend.public_key);
                        this.peers.set(friend.id, {
                            peerId: friend.id,
                            publicKey: friend.public_key,
                            displayName: friend.displayName,
                            state: 'subscribed'
                        });
                        console.log(`Subscribed to ${friend.displayName}'s topic`);
                    } catch (error) {
                        console.error(`Failed to subscribe to ${friend.displayName}:`, error);
                    }
                }
            } catch (error) {
                console.error('Failed to subscribe to friends:', error);
            }

            // Iroh is now initialized and listening - show connecting status (peers not yet discovered)
            this.connectedPeers = 0;
            this.connectionStartTime = Date.now();
            this.updateStatus('connecting');

            // Poll for connected peers periodically
            this.startPeerPolling();

            // Note: Iroh handles device sync automatically via DeviceSyncRequest/Response messages
            // No need for separate sync polling

            // Start periodic presence broadcasting
            this.startPresencePolling();

            // Listen for peer connection events
            this.setupEventListeners();

            console.log('Iroh P2P system initialized');

            // Resolve the initialization promise so waiting callers can proceed
            if (this.initializationResolve) {
                this.initializationResolve();
            }

            return true;
        } catch (error) {
            console.error('Failed to initialize P2P:', error);
            this.updateStatus('offline');
            // Still resolve the promise (with failure state) so waiters don't hang forever
            if (this.initializationResolve) {
                this.initializationResolve();
            }
            throw error;
        }
    },

    // Update peer count and status
    async updatePeerCount() {
        try {
            const status = await TauriAPI.invoke('iroh_get_connection_status');

            // Status object: { listening: bool, connected_peers: number }
            if (!status.listening) {
                this.updateStatus('offline');
                this.connectedPeers = 0;
                this.connectionStartTime = null; // Reset connection timer
                this.adjustPollInterval(true); // Poll faster when offline
                return;
            }

            const previousCount = this.connectedPeers;
            this.connectedPeers = status.connected_peers || 0;

            // If we just gained peers, reset retry counter and try announcing presence
            if (previousCount === 0 && this.connectedPeers > 0) {
                console.log('Peers connected, resetting presence retry counter and announcing');
                this.presenceRetryCount = 0;
                this.connectionStartTime = null; // Clear grace period since we're connected
                if (this.presenceRetryTimer) {
                    clearTimeout(this.presenceRetryTimer);
                    this.presenceRetryTimer = null;
                }
                // Try announcing presence now that we have peers
                await this.announcePresence();
            }

            // Determine status based on peer count and grace period
            let displayStatus;
            if (this.connectedPeers > 0) {
                displayStatus = 'online';
            } else if (this.connectionStartTime &&
                       (Date.now() - this.connectionStartTime) < this.connectionGracePeriod) {
                // Still within grace period - show "Connecting..."
                displayStatus = 'connecting';
            } else {
                // Grace period expired or never set - show "Offline"
                displayStatus = 'offline';
            }

            this.updateStatus(displayStatus);

            // Adjust poll rate based on connection state (offline or connecting = poll faster)
            const needsFasterPolling = this.connectedPeers === 0;
            this.adjustPollInterval(needsFasterPolling);
            this.adjustPresenceInterval(needsFasterPolling);
        } catch (error) {
            console.error('Failed to get connection status:', error);
            // If command doesn't exist yet, assume connecting (not offline) if we just started
            this.connectedPeers = 0;
            if (this.connectionStartTime &&
                (Date.now() - this.connectionStartTime) < this.connectionGracePeriod) {
                this.updateStatus('connecting');
            } else {
                this.updateStatus('offline');
            }
            this.adjustPollInterval(true);
            this.adjustPresenceInterval(true);
        }
    },

    // Adjust polling interval based on connection state
    adjustPollInterval(isOffline) {
        const newInterval = isOffline ? 1000 : 5000; // 1s when offline, 5s when online

        if (this.currentPollInterval !== newInterval) {
            this.currentPollInterval = newInterval;

            // Restart polling with new interval
            if (this.peerPollInterval) {
                clearInterval(this.peerPollInterval);
            }

            this.peerPollInterval = setInterval(async () => {
                if (!this.initialized) {
                    clearInterval(this.peerPollInterval);
                    return;
                }
                await this.updatePeerCount();
            }, newInterval);
        }
    },

    // Start polling for connected peers
    startPeerPolling() {
        // Start with 1 second polling (assume offline initially)
        this.currentPollInterval = 1000;
        this.peerPollInterval = setInterval(async () => {
            if (!this.initialized) {
                clearInterval(this.peerPollInterval);
                return;
            }

            await this.updatePeerCount();
        }, this.currentPollInterval);
    },

    // Note: Device sync is handled automatically by Iroh via DeviceSyncRequest/Response messages
    // No need for manual sync polling

    // Start periodic presence broadcasting with adaptive intervals
    startPresencePolling() {
        // Start with 5 second polling (aggressive when no peers)
        this.currentPresenceInterval = 5000;
        this.presencePollInterval = setInterval(async () => {
            if (!this.initialized) {
                clearInterval(this.presencePollInterval);
                return;
            }

            // Broadcast presence regularly to ensure device discovery
            await this.announcePresence();
        }, this.currentPresenceInterval);

        console.log('Started adaptive presence broadcasting (5s when no peers, 30s when connected)');
    },

    // Adjust presence broadcasting interval based on connection state
    adjustPresenceInterval(isOffline) {
        // 5s when offline/no peers, 30s when connected
        const newInterval = isOffline ? 5000 : 30000;

        if (this.currentPresenceInterval !== newInterval) {
            this.currentPresenceInterval = newInterval;

            // Restart presence polling with new interval
            if (this.presencePollInterval) {
                clearInterval(this.presencePollInterval);
            }

            this.presencePollInterval = setInterval(async () => {
                if (!this.initialized) {
                    clearInterval(this.presencePollInterval);
                    return;
                }
                await this.announcePresence();
            }, newInterval);

            console.log(`Adjusted presence interval to ${newInterval}ms (${isOffline ? 'aggressive' : 'relaxed'})`);
        }
    },

    // Setup event listeners for real-time peer updates
    async setupEventListeners() {
        // Import Tauri event listener (if not already available)
        if (window.__TAURI__?.event) {
            const { listen } = window.__TAURI__.event;

            // Listen for new peer connections
            await listen('peer-connected', async (event) => {
                console.log('Peer connected:', event.payload);
                await this.updatePeerCount();

                // Note: Device sync is handled automatically by Iroh
            });

            // Listen for peer disconnections
            await listen('peer-disconnected', (event) => {
                console.log('Peer disconnected:', event.payload);
                this.updatePeerCount();
            });

            // Listen for incoming P2P messages
            await listen('p2p-message-received', (event) => {
                console.log('P2P message received:', event.payload);
                const { peer_id, message } = event.payload;

                // Handle different message types
                const msgType = message.DirectMessage ? 'DirectMessage'
                    : message.Post ? 'Post'
                    : message.PostWithBlobs ? 'PostWithBlobs'
                    : message.Presence ? 'Presence'
                    : null;

                switch (msgType) {
                    case 'DirectMessage':
                        this.handleIncomingMessage(message.DirectMessage);
                        break;
                    case 'Post':
                        this.handleIncomingPost(message.Post);
                        break;
                    case 'PostWithBlobs':
                        this.handleIncomingPostWithBlobs(message.PostWithBlobs);
                        break;
                    case 'Presence':
                        this.handlePresenceUpdate(message.Presence);
                        break;
                }
            });
        }
    },

    // Handle incoming direct message
    async handleIncomingMessage(msg) {
        console.log('Handling incoming message:', msg);
        // Decrypt and display the message
        // This will be integrated with existing message handling
    },

    // Handle incoming post
    async handleIncomingPost(post) {
        console.log('Handling incoming P2P post:', post);

        try {
            // Save the post to database (it will be created if it doesn't exist)
            const savedPost = await TauriAPI.invoke('create_post', {
                userId: post.user_id,
                content: post.content,
                attachments: null
            });

            console.log('Post saved to database:', savedPost);

            // If the post has attachments, save them too
            if (post.attachments && post.attachments.length > 0) {
                console.log(`Saving ${post.attachments.length} attachments for post ${savedPost.id}`);

                for (const attachment of post.attachments) {
                    await TauriAPI.invoke('upload_media_file', {
                        fileData: attachment.data,
                        filename: 'synced_file',
                        fileType: attachment.file_type,
                        fileSize: attachment.file_size,
                        postId: savedPost.id
                    });
                }

                console.log('All attachments saved');
            }

            // Reload posts to show the new content
            if (typeof loadPosts === 'function') {
                await loadPosts();
            }
        } catch (error) {
            console.error('Error handling incoming post:', error);
        }
    },

    // Handle incoming post with blob references (large attachments)
    async handleIncomingPostWithBlobs(post) {
        console.log('Handling incoming P2P post with blobs:', post);

        try {
            // Save the post to database first (without attachments)
            const savedPost = await TauriAPI.invoke('create_post', {
                userId: post.user_id,
                content: post.content,
                attachments: null
            });

            console.log('Post saved to database:', savedPost);

            // Fetch blob data for each downloaded attachment and save to database
            if (post.blob_refs && post.blob_refs.length > 0) {
                const downloadedBlobs = post.blob_refs.filter(b => b.downloaded);
                console.log(`Processing ${downloadedBlobs.length}/${post.blob_refs.length} downloaded blob attachments for post ${savedPost.id}`);

                for (const blobRef of downloadedBlobs) {
                    try {
                        // BlobReference uses camelCase due to serde rename_all
                        console.log(`Fetching blob ${blobRef.blobHash} (${blobRef.fileSize} bytes) from local store`);

                        // Fetch the blob data from local store (already downloaded by backend)
                        const blobData = await TauriAPI.invoke('iroh_read_blob', {
                            blobHash: blobRef.blobHash
                        });

                        console.log(`Blob ${blobRef.blobHash} fetched, saving as attachment`);

                        // Save as attachment
                        await TauriAPI.invoke('upload_media_file', {
                            fileData: blobData,
                            filename: 'synced_blob',
                            fileType: blobRef.fileType,
                            fileSize: blobRef.fileSize,
                            postId: savedPost.id
                        });

                        console.log(`Attachment saved for blob ${blobRef.blobHash}`);
                    } catch (blobError) {
                        console.error(`Failed to fetch/save blob ${blobRef.blobHash}:`, blobError);
                    }
                }

                // Log any blobs that failed to download
                const failedBlobs = post.blob_refs.filter(b => !b.downloaded);
                if (failedBlobs.length > 0) {
                    console.warn(`${failedBlobs.length} blob(s) failed to download:`, failedBlobs.map(b => b.blobHash));
                }

                console.log('All blob attachments processed');
            }

            // Reload posts to show the new content
            if (typeof loadPosts === 'function') {
                await loadPosts();
            }
        } catch (error) {
            console.error('Error handling incoming post with blobs:', error);
        }
    },

    // Handle presence update
    async handlePresenceUpdate(presence) {
        console.log('Handling presence update:', presence);
        // Update user's online status
    },

    // Shutdown P2P system (called on logout)
    async shutdown() {
        if (!this.initialized) {
            return;
        }

        try {
            if (this.peerPollInterval) {
                clearInterval(this.peerPollInterval);
            }

            if (this.presencePollInterval) {
                clearInterval(this.presencePollInterval);
            }

            if (this.presenceRetryTimer) {
                clearTimeout(this.presenceRetryTimer);
                this.presenceRetryTimer = null;
            }

            await TauriAPI.invoke('iroh_shutdown');

            this.peers.clear();
            this.initialized = false;
            this.connectedPeers = 0;
            this.presenceRetryCount = 0;
            this.connectionStartTime = null;
            this.updateStatus('offline');
            console.log('P2P system shut down');

            return true;
        } catch (error) {
            console.error('Failed to shutdown P2P:', error);
            throw error;
        }
    },

    // Subscribe to a friend's topic
    async subscribeFriend(friendPublicKey) {
        try {
            await TauriAPI.invoke('iroh_subscribe_friend', {
                friendPublicKey: friendPublicKey
            });
            return true;
        } catch (error) {
            console.error('Failed to subscribe to friend:', error);
            throw error;
        }
    },

    // Send encrypted message to a friend
    async sendMessage(toUserId, toPublicKey, encryptedContent) {
        try {
            await TauriAPI.invoke('iroh_send_message', {
                toUserId: toUserId,
                toPublicKey: toPublicKey,
                encryptedContent: encryptedContent
            });
            console.log('Message sent via Iroh');
            return true;
        } catch (error) {
            console.error('Failed to send message:', error);
            throw error;
        }
    },

    // Publish a post to your own topic
    async publishPost(content, postId) {
        try {
            await TauriAPI.invoke('iroh_publish_post', {
                content: content,
                postId: postId
            });
            console.log('Post published via Iroh');
            return true;
        } catch (error) {
            console.error('Failed to publish post:', error);
            throw error;
        }
    },

    // Announce presence to the network with exponential backoff
    async announcePresence() {
        try {
            await TauriAPI.invoke('iroh_announce_presence');
            console.log('Presence announced successfully');
            // Reset retry count on success
            this.presenceRetryCount = 0;
            if (this.presenceRetryTimer) {
                clearTimeout(this.presenceRetryTimer);
                this.presenceRetryTimer = null;
            }
            return true;
        } catch (error) {
            const errorStr = error.toString();

            // Handle "no peers subscribed" error gracefully - this is expected when alone
            if (errorStr.includes('NoPeersSubscribedToTopic') || errorStr.includes('No peers subscribed')) {
                console.log('No peers subscribed to topic yet, will retry with backoff');
                this.schedulePresenceRetry();
                return false; // Don't throw, just return false
            }

            // For other errors, log but don't throw - P2P presence is not critical
            console.warn('Failed to announce presence (non-critical):', error);
            this.schedulePresenceRetry();
            return false;
        }
    },

    // Schedule presence retry with exponential backoff
    schedulePresenceRetry() {
        // Clear any existing retry timer
        if (this.presenceRetryTimer) {
            clearTimeout(this.presenceRetryTimer);
        }

        // Don't retry if we've exceeded max retries
        if (this.presenceRetryCount >= this.maxPresenceRetries) {
            console.log('Max presence retry attempts reached, will retry when peers connect');
            return;
        }

        // Calculate exponential backoff delay: baseDelay * 2^retryCount
        // With jitter to prevent thundering herd
        const exponentialDelay = this.baseRetryDelay * Math.pow(2, this.presenceRetryCount);
        const jitter = Math.random() * 1000; // 0-1000ms jitter
        const delay = Math.min(exponentialDelay + jitter, 60000); // Cap at 60 seconds

        console.log(`Scheduling presence retry #${this.presenceRetryCount + 1} in ${Math.round(delay)}ms`);

        this.presenceRetryTimer = setTimeout(async () => {
            this.presenceRetryCount++;
            await this.announcePresence();
        }, delay);
    },

    // Ensure P2P is initialized and healthy (handles resume/state mismatch)
    async ensureInitialized(retryCount = 0) {
        const maxRetries = 10;
        const retryDelay = 500; // 500ms between retries

        // If already initialized, we're good
        if (this.initialized) {
            return;
        }

        // If there's an initialization in progress, wait for it
        if (this.initializationPromise) {
            console.log('P2P initialization in progress, waiting...');
            await this.initializationPromise;
            // After waiting, check if we're now initialized
            if (this.initialized) {
                console.log('P2P initialization completed while waiting');
                return;
            }
        }

        // Check if Rust-side is actually initialized by checking connection status
        try {
            const status = await TauriAPI.invoke('iroh_get_connection_status');

            // Case 1: Rust is listening but JS thinks we're not initialized
            // This can happen if the promise resolved but UI navigated away
            if (status.listening && !this.initialized) {
                console.log('P2P state mismatch - Rust initialized but JS not, syncing state...');
                this.initialized = true;
                this.startPeerPolling();
                this.startPresencePolling();
                return;
            }

            // Case 2: JS thinks we're initialized but Rust side was reset
            if (!status.listening && this.initialized) {
                console.log('P2P state mismatch detected - Rust side not initialized, reinitializing...');
                // Force re-initialization
                await this.initialize(this.userId, this.displayName, this.publicKey, this.deviceId, true);
                return;
            }

            // Case 3: Neither side is initialized - wait and retry if initialization may be in progress
            if (!status.listening && !this.initialized) {
                // If we have credentials stored, initialization may be happening - wait and retry
                if (retryCount < maxRetries) {
                    console.log(`P2P not ready yet, waiting... (attempt ${retryCount + 1}/${maxRetries})`);
                    await new Promise(resolve => setTimeout(resolve, retryDelay));
                    return this.ensureInitialized(retryCount + 1);
                }
                console.log('P2P not initialized after retries, giving up');
                throw new Error('Iroh not initialized');
            }
        } catch (error) {
            // If the call fails entirely, assume not initialized
            if (this.initialized && this.userId && this.publicKey) {
                console.log('P2P health check failed, attempting reinitialization...');
                await this.initialize(this.userId, this.displayName, this.publicKey, this.deviceId, true);
            } else if (error.message === 'Iroh not initialized') {
                // Re-throw our own error
                throw error;
            } else if (retryCount < maxRetries) {
                // Network or other transient error - retry
                console.log(`P2P health check failed, retrying... (attempt ${retryCount + 1}/${maxRetries})`);
                await new Promise(resolve => setTimeout(resolve, retryDelay));
                return this.ensureInitialized(retryCount + 1);
            } else {
                // Re-throw the error so caller knows P2P isn't ready
                throw error;
            }
        }
    },

    // Comprehensive health check and recovery for app resume
    // This is more thorough than ensureInitialized - it checks actual network health
    async healthCheckAndRecover() {
        if (!this.initialized) {
            console.log('[P2P-HEALTH] Not initialized, skipping health check');
            return false;
        }

        try {
            console.log('[P2P-HEALTH] Running health check...');
            const health = await TauriAPI.invoke('iroh_health_check');

            console.log('[P2P-HEALTH] Status:', JSON.stringify(health));

            if (health.healthy) {
                console.log('[P2P-HEALTH] Network is healthy');
                return true;
            }

            // Network needs recovery
            console.log('[P2P-HEALTH] Network unhealthy, attempting recovery...');

            if (health.needs_reconnect) {
                // Serious issue - try recovery first, then full reinit if that fails
                try {
                    await TauriAPI.invoke('iroh_recover');
                    console.log('[P2P-HEALTH] Recovery successful');

                    // Restart JS-side polling
                    this.startPeerPolling();
                    this.startPresencePolling();

                    return true;
                } catch (recoverError) {
                    console.error('[P2P-HEALTH] Recovery failed, attempting full reinitialization:', recoverError);
                    await this.initialize(this.userId, this.displayName, this.publicKey, this.deviceId, true);
                    return true;
                }
            } else {
                // Less serious - just stale background loops, try recovery
                try {
                    await TauriAPI.invoke('iroh_recover');
                    console.log('[P2P-HEALTH] Background loop recovery successful');
                    return true;
                } catch (recoverError) {
                    console.error('[P2P-HEALTH] Background loop recovery failed:', recoverError);
                    // Try announcing presence as a fallback
                    await this.announcePresence();
                    return false;
                }
            }
        } catch (error) {
            console.error('[P2P-HEALTH] Health check failed:', error);

            // If health check itself fails, try full reinitialization
            if (this.userId && this.publicKey) {
                try {
                    console.log('[P2P-HEALTH] Attempting full reinitialization...');
                    await this.initialize(this.userId, this.displayName, this.publicKey, this.deviceId, true);
                    return true;
                } catch (reinitError) {
                    console.error('[P2P-HEALTH] Reinitialization failed:', reinitError);
                    return false;
                }
            }
            return false;
        }
    },

    // Request manual sync (triggered by pull-to-refresh)
    // Device sync happens automatically via presence announcements,
    // so this just triggers an immediate presence announcement
    async requestSync() {
        console.log('Manual sync requested, announcing presence...');
        return await this.announcePresence();
    },

    // Generate invite code for peer discovery
    // Privacy-preserving: User controls who gets their NodeId
    async generateInvite() {
        // Ensure Rust-side is initialized (handles app resume)
        await this.ensureInitialized();

        try {
            const inviteCode = await TauriAPI.invoke('iroh_generate_invite');
            console.log('Generated invite code');
            return inviteCode;
        } catch (error) {
            console.error('Failed to generate invite code:', error);
            throw error;
        }
    },

};

// Expose P2P to global scope
window.P2P = P2P;
