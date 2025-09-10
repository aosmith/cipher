/**
 * P2P Hosting Integration
 * Connects local hosting with the P2P network for file discovery and distribution
 */

class P2PHostingIntegration {
  constructor(localHostingManager, webrtcManager) {
    this.localHosting = localHostingManager;
    this.webrtc = webrtcManager;
    this.peers = new Map(); // peerId -> peerInfo
    this.hostRegistry = new Map(); // hostId -> hostInfo
    this.fileRequests = new Map(); // requestId -> requestInfo
    this.isAdvertising = false;
    
    this.setupEventHandlers();
    this.setupWebRTCIntegration();
  }

  setupWebRTCIntegration() {
    if (this.webrtc) {
      // Override WebRTC message handler to include P2P hosting messages
      const originalMessageHandler = this.webrtc.onEncryptedMessage;
      this.webrtc.onEncryptedMessage = async (peerId, message) => {
        if (message.p2pHosting) {
          await this.handleP2PMessage(message.p2pHosting, peerId);
        } else if (originalMessageHandler) {
          originalMessageHandler.call(this.webrtc, peerId, message);
        }
      };
    }
  }

  setupEventHandlers() {
    // Listen for local hosting events
    this.localHosting.onHostingStatusChanged = (active) => {
      if (active) {
        this.startAdvertising();
      } else {
        this.stopAdvertising();
      }
    };
    
    this.localHosting.onFileStored = (fileHash, sizeMB) => {
      this.announceFileAvailability(fileHash, sizeMB);
    };
    
    this.localHosting.onFileDeleted = (fileHash, sizeMB) => {
      this.announceFileUnavailability(fileHash);
    };
  }

  // Start advertising hosting capabilities to the P2P network
  async startAdvertising() {
    if (this.isAdvertising) return;
    
    try {
      const hostInfo = {
        hostId: await this.localHosting.getHostId(),
        capacity: this.localHosting.quota.allocated,
        available: this.localHosting.quota.available,
        reliability: this.localHosting.performanceMetrics.reliability,
        bandwidth: this.localHosting.performanceMetrics.bandwidth,
        filesHosted: this.localHosting.hostedFiles.size,
        endpoint: window.location.origin,
        timestamp: Date.now()
      };
      
      // Broadcast hosting announcement
      if (this.webrtc) {
        this.webrtc.broadcastMessage({
          p2pHosting: {
            type: 'host_announcement',
            data: hostInfo
          }
        });
      }
      
      // Register with blockchain if available
      if (window.cipherWallet?.isInitialized) {
        await this.registerBlockchainHost(hostInfo);
      }
      
      this.isAdvertising = true;
      this.startPeriodicAnnouncements();
      
      console.log('Started advertising hosting capabilities');
    } catch (error) {
      console.error('Failed to start advertising:', error);
    }
  }

  // Stop advertising hosting capabilities
  async stopAdvertising() {
    if (!this.isAdvertising) return;
    
    try {
      if (this.announcementInterval) {
        clearInterval(this.announcementInterval);
      }
      
      // Broadcast hosting shutdown
      if (this.webrtc) {
        this.webrtc.broadcastMessage({
          p2pHosting: {
            type: 'host_shutdown',
            data: {
              hostId: await this.localHosting.getHostId(),
              timestamp: Date.now()
            }
          }
        });
      }
      
      // Unregister from blockchain
      if (window.cipherWallet?.isInitialized) {
        await this.unregisterBlockchainHost();
      }
      
      this.isAdvertising = false;
      console.log('Stopped advertising hosting capabilities');
    } catch (error) {
      console.error('Failed to stop advertising:', error);
    }
  }

  // Start periodic host announcements
  startPeriodicAnnouncements() {
    // Announce every 5 minutes
    this.announcementInterval = setInterval(async () => {
      if (this.isAdvertising) {
        await this.startAdvertising(); // Re-announce current status
      }
    }, 5 * 60 * 1000);
  }

  // Announce file availability to network
  async announceFileAvailability(fileHash, sizeMB) {
    if (!this.isAdvertising) return;
    
    try {
      const announcement = {
        type: 'file_available',
        data: {
          fileHash,
          sizeMB,
          hostId: await this.localHosting.getHostId(),
          timestamp: Date.now()
        }
      };
      
      if (this.webrtc) {
        this.webrtc.broadcastMessage({
          p2pHosting: announcement
        });
      }
      
      console.log(`Announced file availability: ${fileHash}`);
    } catch (error) {
      console.error('Failed to announce file availability:', error);
    }
  }

  // Announce file unavailability
  async announceFileUnavailability(fileHash) {
    if (!this.isAdvertising) return;
    
    try {
      const announcement = {
        type: 'file_unavailable',
        data: {
          fileHash,
          hostId: await this.localHosting.getHostId(),
          timestamp: Date.now()
        }
      };
      
      if (this.webrtc) {
        this.webrtc.broadcastMessage({
          p2pHosting: announcement
        });
      }
      
      console.log(`Announced file unavailability: ${fileHash}`);
    } catch (error) {
      console.error('Failed to announce file unavailability:', error);
    }
  }

  // Handle incoming P2P messages
  async handleP2PMessage(message, peerId) {
    switch (message.type) {
      case 'file_request':
        await this.handleFileRequest(message.data, peerId);
        break;
      case 'file_response':
        await this.handleFileResponse(message.data, peerId);
        break;
      case 'host_announcement':
        await this.handleHostAnnouncement(message.data, peerId);
        break;
      case 'file_available':
        await this.handleFileAvailabilityAnnouncement(message.data, peerId);
        break;
      case 'file_unavailable':
        await this.handleFileUnavailabilityAnnouncement(message.data, peerId);
        break;
      case 'host_discovery':
        await this.handleHostDiscovery(message.data, peerId);
        break;
      case 'host_info':
        await this.handleHostInfo(message.data, peerId);
        break;
      case 'host_shutdown':
        await this.handleHostShutdown(message.data, peerId);
        break;
      default:
        console.log('Unknown P2P message type:', message.type);
    }
  }

  // Handle file unavailability announcement  
  async handleFileUnavailabilityAnnouncement(fileData, peerId) {
    const fileHash = fileData.fileHash;
    const existingHosts = this.getFileHosts(fileHash);
    if (existingHosts) {
      existingHosts.delete(fileData.hostId);
      this.updateFileHostRegistry(fileHash, existingHosts);
    }
    console.log(`File ${fileHash} no longer available at host ${fileData.hostId}`);
  }

  // Handle host info response
  async handleHostInfo(hostData, peerId) {
    this.hostRegistry.set(hostData.hostId, {
      ...hostData,
      peerId,
      lastSeen: Date.now()
    });
    console.log(`Updated host info: ${hostData.hostId}`);
  }

  // Handle host shutdown announcement
  async handleHostShutdown(shutdownData, peerId) {
    this.hostRegistry.delete(shutdownData.hostId);
    console.log(`Host ${shutdownData.hostId} shutdown`);
  }

  // Handle file request from peer
  async handleFileRequest(requestData, peerId) {
    const { fileHash, requestId } = requestData;
    
    try {
      // Check if we have the file
      if (this.localHosting.hostedFiles.has(fileHash)) {
        // Retrieve file data
        const fileData = await this.localHosting.retrieveFile(fileHash);
        
        // Send file to peer
        if (this.webrtc) {
          this.webrtc.sendEncryptedMessage(peerId, {
            p2pHosting: {
              type: 'file_response',
              data: {
                requestId,
                fileHash,
                fileData: Array.from(new Uint8Array(fileData)),
                success: true
              }
            }
          });
        }
        
        console.log(`Served file ${fileHash} to peer ${peerId}`);
      } else {
        // File not found
        if (this.webrtc) {
          this.webrtc.sendEncryptedMessage(peerId, {
            p2pHosting: {
              type: 'file_response',
              data: {
                requestId,
                fileHash,
                success: false,
                error: 'File not found'
              }
            }
          });
        }
      }
    } catch (error) {
      console.error('Failed to handle file request:', error);
      
      // Send error response
      if (this.webrtc) {
        this.webrtc.sendEncryptedMessage(peerId, {
          p2pHosting: {
            type: 'file_response',
            data: {
              requestId,
              fileHash,
              success: false,
              error: error.message
            }
          }
        });
      }
    }
  }

  // Handle host announcement from peer
  async handleHostAnnouncement(hostData, peerId) {
    this.hostRegistry.set(hostData.hostId, {
      ...hostData,
      peerId,
      lastSeen: Date.now()
    });
    
    console.log(`Registered host: ${hostData.hostId}`);
  }

  // Handle file availability announcement
  async handleFileAvailabilityAnnouncement(fileData, peerId) {
    // Store information about where files are available
    const fileHash = fileData.fileHash;
    const existingHosts = this.getFileHosts(fileHash) || new Set();
    existingHosts.add(fileData.hostId);
    
    // Update file host registry
    this.updateFileHostRegistry(fileHash, existingHosts);
    
    console.log(`File ${fileHash} available at host ${fileData.hostId}`);
  }

  // Handle host discovery request
  async handleHostDiscovery(requestData, peerId) {
    if (this.isAdvertising) {
      // Respond with our host info
      const hostInfo = {
        hostId: await this.localHosting.getHostId(),
        capacity: this.localHosting.quota.allocated,
        available: this.localHosting.quota.available,
        reliability: this.localHosting.performanceMetrics.reliability,
        filesHosted: this.localHosting.hostedFiles.size
      };
      
      if (this.webrtc) {
        this.webrtc.sendEncryptedMessage(peerId, {
          p2pHosting: {
            type: 'host_info',
            data: hostInfo
          }
        });
      }
    }
  }

  // Request file from network
  async requestFileFromNetwork(fileHash) {
    const hosts = this.getFileHosts(fileHash);
    
    if (!hosts || hosts.size === 0) {
      throw new Error('No hosts found for file');
    }
    
    // Try each host until successful
    for (const hostId of hosts) {
      const hostInfo = this.hostRegistry.get(hostId);
      if (!hostInfo) continue;
      
      try {
        const fileData = await this.requestFileFromHost(fileHash, hostInfo.peerId);
        return fileData;
      } catch (error) {
        console.warn(`Failed to get file from host ${hostId}:`, error);
        continue;
      }
    }
    
    throw new Error('Failed to retrieve file from any host');
  }

  // Request file from specific host
  async requestFileFromHost(fileHash, peerId) {
    return new Promise((resolve, reject) => {
      const requestId = Date.now().toString();
      const timeout = setTimeout(() => {
        this.fileRequests.delete(requestId);
        reject(new Error('Request timeout'));
      }, 30000); // 30 second timeout
      
      this.fileRequests.set(requestId, {
        fileHash,
        peerId,
        resolve,
        reject,
        timeout
      });
      
      // Send request
      if (this.webrtc) {
        this.webrtc.sendEncryptedMessage(peerId, {
          p2pHosting: {
            type: 'file_request',
            data: { fileHash, requestId }
          }
        });
      } else {
        clearTimeout(timeout);
        this.fileRequests.delete(requestId);
        reject(new Error('WebRTC not available'));
      }
    });
  }

  // Handle file response
  async handleFileResponse(responseData, peerId) {
    const { requestId, fileHash, fileData, success, error } = responseData;
    
    const request = this.fileRequests.get(requestId);
    if (!request) return;
    
    clearTimeout(request.timeout);
    this.fileRequests.delete(requestId);
    
    if (success && fileData) {
      const fileBuffer = new Uint8Array(fileData).buffer;
      request.resolve(fileBuffer);
    } else {
      request.reject(new Error(error || 'File request failed'));
    }
  }

  // Discover available hosts
  async discoverHosts() {
    if (this.webrtc) {
      this.webrtc.broadcastMessage({
        p2pHosting: {
          type: 'host_discovery',
          data: { timestamp: Date.now() }
        }
      });
    }
    
    // Clean up old hosts (older than 10 minutes)
    const cutoff = Date.now() - (10 * 60 * 1000);
    for (const [hostId, hostInfo] of this.hostRegistry.entries()) {
      if (hostInfo.lastSeen < cutoff) {
        this.hostRegistry.delete(hostId);
      }
    }
  }

  // Blockchain integration methods
  async registerBlockchainHost(hostInfo) {
    try {
      // This would integrate with the existing blockchain host registration
      // For now, just log the registration
      console.log('Would register blockchain host:', hostInfo);
      
      // In a full implementation:
      // await window.cipherWallet.token.registerAsHost(hostInfo.capacity * 1024);
    } catch (error) {
      console.error('Failed to register blockchain host:', error);
    }
  }

  async unregisterBlockchainHost() {
    try {
      // This would unregister from blockchain
      console.log('Would unregister blockchain host');
      
      // In a full implementation:
      // await window.cipherWallet.token.unregisterAsHost();
    } catch (error) {
      console.error('Failed to unregister blockchain host:', error);
    }
  }

  // Utility methods
  getFileHosts(fileHash) {
    // In a real implementation, this would query a distributed hash table
    // For now, return from memory
    return this.fileHostRegistry?.get(fileHash);
  }

  updateFileHostRegistry(fileHash, hosts) {
    if (!this.fileHostRegistry) {
      this.fileHostRegistry = new Map();
    }
    this.fileHostRegistry.set(fileHash, hosts);
  }

  getNetworkStats() {
    return {
      totalHosts: this.hostRegistry.size,
      isAdvertising: this.isAdvertising,
      filesKnown: this.fileHostRegistry?.size || 0,
      pendingRequests: this.fileRequests.size
    };
  }
}

// Export for ES6 modules
export default P2PHostingIntegration;

// Also make available globally
window.P2PHostingIntegration = P2PHostingIntegration;