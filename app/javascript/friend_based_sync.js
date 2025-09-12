/**
 * Friend-Based Data Syncing System
 * Only syncs and hosts data for accepted friends
 */

class FriendBasedSync {
  constructor(webrtcManager, localHostingManager, userId) {
    this.webrtc = webrtcManager;
    this.localHosting = localHostingManager;
    this.userId = userId;
    this.friendIds = new Set();
    this.syncedContent = new Map(); // contentId -> { friendId, contentHash, lastSync }
    this.incomingSyncRequests = new Map(); // requestId -> requestData
    
    this.setupEventHandlers();
    this.loadFriendsList();
  }

  async loadFriendsList() {
    try {
      // Fetch current user's friends from server
      const response = await fetch(`/api/v1/users/${this.userId}/friends`);
      const friends = await response.json();
      
      this.friendIds.clear();
      friends.forEach(friend => this.friendIds.add(friend.id));
      
      console.log(`Loaded ${this.friendIds.size} friends for syncing`);
    } catch (error) {
      console.error('Failed to load friends list:', error);
    }
  }

  setupEventHandlers() {
    // Listen for WebRTC messages
    if (this.webrtc) {
      const originalMessageHandler = this.webrtc.onEncryptedMessage;
      this.webrtc.onEncryptedMessage = async (peerId, message) => {
        if (message.friendSync) {
          await this.handleFriendSyncMessage(message.friendSync, peerId);
        } else if (originalMessageHandler) {
          originalMessageHandler.call(this.webrtc, peerId, message);
        }
      };

      // Handle new peer connections - check if they're friends
      this.webrtc.onPeerConnected = async (peerId) => {
        await this.handleNewPeerConnection(peerId);
      };
    }
  }

  async handleNewPeerConnection(peerId) {
    // Send friend verification request
    if (this.webrtc) {
      this.webrtc.sendEncryptedMessage(peerId, {
        friendSync: {
          type: 'friend_verification',
          data: {
            userId: this.userId,
            timestamp: Date.now()
          }
        }
      });
    }
  }

  async handleFriendSyncMessage(message, peerId) {
    switch (message.type) {
      case 'friend_verification':
        await this.handleFriendVerification(message.data, peerId);
        break;
      case 'friend_verified':
        await this.handleFriendVerified(message.data, peerId);
        break;
      case 'content_announcement':
        await this.handleContentAnnouncement(message.data, peerId);
        break;
      case 'sync_request':
        await this.handleSyncRequest(message.data, peerId);
        break;
      case 'sync_response':
        await this.handleSyncResponse(message.data, peerId);
        break;
      case 'content_data':
        await this.handleContentData(message.data, peerId);
        break;
      default:
        console.log('Unknown friend sync message type:', message.type);
    }
  }

  async handleFriendVerification(data, peerId) {
    const senderUserId = data.userId;
    
    // Check if this user is in our friends list
    if (this.friendIds.has(senderUserId)) {
      // Respond with verification
      if (this.webrtc) {
        this.webrtc.sendEncryptedMessage(peerId, {
          friendSync: {
            type: 'friend_verified',
            data: {
              userId: this.userId,
              verified: true,
              timestamp: Date.now()
            }
          }
        });
      }
      
      console.log(`Verified friend connection with user ${senderUserId}`);
      
      // Start syncing process with this friend
      await this.initiateSyncWithFriend(peerId, senderUserId);
    } else {
      // Not a friend - limit interaction
      if (this.webrtc) {
        this.webrtc.sendEncryptedMessage(peerId, {
          friendSync: {
            type: 'friend_verified',
            data: {
              userId: this.userId,
              verified: false,
              reason: 'Not in friends list'
            }
          }
        });
      }
      
      console.log(`Rejected sync request from non-friend: ${senderUserId}`);
    }
  }

  async handleFriendVerified(data, peerId) {
    if (data.verified) {
      console.log(`Friend verification successful with user ${data.userId}`);
      await this.initiateSyncWithFriend(peerId, data.userId);
    } else {
      console.log(`Friend verification failed: ${data.reason}`);
      // Disconnect from non-friend
      if (this.webrtc) {
        this.webrtc.disconnectFromPeer(peerId);
      }
    }
  }

  async initiateSyncWithFriend(peerId, friendUserId) {
    try {
      // Get friend's posts that we should sync
      const response = await fetch(`/api/v1/users/${friendUserId}/posts/for_sync`);
      const friendPosts = await response.json();
      
      // Announce our available content to friend
      await this.announceContentToFriend(peerId, friendUserId);
      
      // Request any missing content from friend
      await this.requestMissingContent(peerId, friendUserId, friendPosts);
      
    } catch (error) {
      console.error('Failed to initiate sync with friend:', error);
    }
  }

  async announceContentToFriend(peerId, friendUserId) {
    try {
      // Get our posts that this friend should have access to
      const response = await fetch(`/api/v1/posts/my_posts_for_friends`);
      const myPosts = await response.json();
      
      const contentAnnouncement = {
        userId: this.userId,
        availableContent: myPosts.map(post => ({
          postId: post.id,
          contentHash: post.content_hash,
          timestamp: post.timestamp,
          hasAttachments: post.attachments_count > 0,
          attachmentHashes: post.attachment_hashes || []
        }))
      };
      
      if (this.webrtc) {
        this.webrtc.sendEncryptedMessage(peerId, {
          friendSync: {
            type: 'content_announcement',
            data: contentAnnouncement
          }
        });
      }
      
    } catch (error) {
      console.error('Failed to announce content to friend:', error);
    }
  }

  async handleContentAnnouncement(data, peerId) {
    const friendUserId = data.userId;
    
    // Only process if they're actually our friend
    if (!this.friendIds.has(friendUserId)) {
      console.log(`Ignoring content announcement from non-friend: ${friendUserId}`);
      return;
    }
    
    // Check what content we're missing from this friend
    const missingContent = [];
    
    for (const contentInfo of data.availableContent) {
      // Check if we already have this content locally
      const hasContent = await this.hasContentLocally(contentInfo.contentHash);
      
      if (!hasContent) {
        missingContent.push(contentInfo);
      }
    }
    
    // Request missing content
    if (missingContent.length > 0) {
      await this.requestSpecificContent(peerId, friendUserId, missingContent);
    }
    
    console.log(`Friend ${friendUserId} announced ${data.availableContent.length} items, requesting ${missingContent.length} missing items`);
  }

  async requestSpecificContent(peerId, friendUserId, contentList) {
    const requestId = Date.now().toString();
    
    if (this.webrtc) {
      this.webrtc.sendEncryptedMessage(peerId, {
        friendSync: {
          type: 'sync_request',
          data: {
            requestId,
            requestedContent: contentList.map(content => ({
              postId: content.postId,
              contentHash: content.contentHash,
              includeAttachments: content.hasAttachments
            }))
          }
        }
      });
    }
    
    // Track the request
    this.incomingSyncRequests.set(requestId, {
      peerId,
      friendUserId,
      requestedAt: Date.now(),
      status: 'pending'
    });
  }

  async handleSyncRequest(data, peerId) {
    const { requestId, requestedContent } = data;
    
    try {
      const syncResponse = {
        requestId,
        content: []
      };
      
      // Process each requested content item
      for (const request of requestedContent) {
        try {
          // Fetch post data from database
          const response = await fetch(`/api/v1/posts/${request.postId}/sync_data`);
          
          if (response.ok) {
            const postData = await response.json();
            
            // Verify this is our post (security check)
            if (postData.user_id === this.userId) {
              const contentItem = {
                postId: request.postId,
                contentHash: request.contentHash,
                postData: {
                  content_encrypted: postData.content_encrypted,
                  timestamp: postData.timestamp,
                  signature: postData.signature,
                  user_id: postData.user_id
                },
                success: true
              };
              
              // Include attachments if requested
              if (request.includeAttachments && postData.attachments) {
                contentItem.attachments = postData.attachments;
              }
              
              syncResponse.content.push(contentItem);
            }
          }
        } catch (error) {
          console.error(`Failed to fetch post ${request.postId} for sync:`, error);
          syncResponse.content.push({
            postId: request.postId,
            success: false,
            error: error.message
          });
        }
      }
      
      // Send response
      if (this.webrtc) {
        this.webrtc.sendEncryptedMessage(peerId, {
          friendSync: {
            type: 'sync_response',
            data: syncResponse
          }
        });
      }
      
    } catch (error) {
      console.error('Failed to handle sync request:', error);
    }
  }

  async handleSyncResponse(data, peerId) {
    const { requestId, content } = data;
    
    const request = this.incomingSyncRequests.get(requestId);
    if (!request) {
      console.log(`Received response for unknown request: ${requestId}`);
      return;
    }
    
    // Process received content
    for (const contentItem of content) {
      if (contentItem.success) {
        await this.storeSyncedContent(contentItem, request.friendUserId);
      } else {
        console.error(`Failed to sync post ${contentItem.postId}: ${contentItem.error}`);
      }
    }
    
    // Clean up request
    this.incomingSyncRequests.delete(requestId);
    
    console.log(`Sync completed for request ${requestId}: received ${content.filter(c => c.success).length} items`);
  }

  async storeSyncedContent(contentItem, friendUserId) {
    try {
      // Store the synced post in our local database
      const response = await fetch('/api/v1/posts/sync_store', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-CSRF-Token': document.querySelector('meta[name="csrf-token"]')?.getAttribute('content')
        },
        body: JSON.stringify({
          post: contentItem.postData,
          original_user_id: friendUserId,
          synced_at: new Date().toISOString()
        })
      });
      
      if (response.ok) {
        // Also store in local hosting if we're hosting for friends
        if (this.localHosting && this.localHosting.isHostingActive()) {
          await this.storeInLocalHosting(contentItem);
        }
        
        // Track synced content
        this.syncedContent.set(contentItem.contentHash, {
          friendId: friendUserId,
          contentHash: contentItem.contentHash,
          lastSync: Date.now()
        });
        
        console.log(`Successfully stored synced content: ${contentItem.postId}`);
      } else {
        throw new Error(`Server error: ${response.status}`);
      }
      
    } catch (error) {
      console.error('Failed to store synced content:', error);
    }
  }

  async storeInLocalHosting(contentItem) {
    try {
      // Convert content to storage format for local hosting
      const contentData = JSON.stringify(contentItem.postData);
      const contentBuffer = new TextEncoder().encode(contentData);
      
      const metadata = {
        postId: contentItem.postId,
        contentType: 'application/json',
        syncedContent: true,
        originalUserId: contentItem.postData.user_id,
        storedAt: Date.now()
      };
      
      await this.localHosting.storeFile(contentItem.contentHash, contentBuffer, metadata);
      
      // Store attachments if present
      if (contentItem.attachments) {
        for (const attachment of contentItem.attachments) {
          if (attachment.data_encrypted) {
            const attachmentBuffer = new TextEncoder().encode(attachment.data_encrypted);
            await this.localHosting.storeFile(attachment.checksum, attachmentBuffer, {
              filename: attachment.filename,
              contentType: attachment.content_type,
              parentPost: contentItem.postId,
              syncedContent: true
            });
          }
        }
      }
      
    } catch (error) {
      console.error('Failed to store content in local hosting:', error);
    }
  }

  async hasContentLocally(contentHash) {
    try {
      // Check local database first
      const response = await fetch(`/api/v1/content/${contentHash}/exists`);
      if (response.ok) {
        const data = await response.json();
        if (data.exists) return true;
      }
      
      // Check local hosting
      if (this.localHosting) {
        const files = await this.localHosting.getHostedFiles();
        return files.some(file => file.hash === contentHash);
      }
      
      return false;
    } catch (error) {
      console.error('Error checking content locally:', error);
      return false;
    }
  }

  // Public API methods
  
  async addFriend(friendUserId) {
    this.friendIds.add(friendUserId);
    
    // If we have an active connection to this user, start syncing
    if (this.webrtc) {
      const connectedPeers = this.webrtc.getConnectedPeers();
      // Find peer by userId (this would need to be implemented)
      // and initiate sync if connected
    }
  }

  async removeFriend(friendUserId) {
    this.friendIds.delete(friendUserId);
    
    // Stop syncing with this user and optionally remove their content
    const syncedItems = Array.from(this.syncedContent.entries())
      .filter(([_, syncInfo]) => syncInfo.friendId === friendUserId);
    
    for (const [contentHash, _] of syncedItems) {
      this.syncedContent.delete(contentHash);
      
      // Optionally remove from local hosting
      if (this.localHosting) {
        try {
          await this.localHosting.removeFile(contentHash);
        } catch (error) {
          console.error(`Failed to remove content ${contentHash}:`, error);
        }
      }
    }
    
    console.log(`Removed sync data for ex-friend ${friendUserId}`);
  }

  getSyncStats() {
    return {
      friendCount: this.friendIds.size,
      syncedContentCount: this.syncedContent.size,
      pendingRequests: this.incomingSyncRequests.size,
      isActive: this.webrtc?.getConnectedPeers().length > 0
    };
  }

  async forceSyncWithAllFriends() {
    if (!this.webrtc) return;
    
    const connectedPeers = this.webrtc.getConnectedPeers();
    
    for (const peerId of connectedPeers) {
      // This would need to map peerId to userId
      // await this.initiateSyncWithFriend(peerId, friendUserId);
    }
    
    console.log(`Initiated sync with ${connectedPeers.length} connected peers`);
  }
}

// Export for ES6 modules
export default FriendBasedSync;

// Make available globally
window.FriendBasedSync = FriendBasedSync;