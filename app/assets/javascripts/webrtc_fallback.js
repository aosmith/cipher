/**
 * WebRTC STUN/TURN Fallback Manager for Cipher
 * Provides hierarchical fallback for reliable P2P connections
 */

class WebRTCFallbackManager {
  constructor() {
    this.serverTiers = [
      // Tier 1: Primary reliable servers (verified working)
      {
        name: 'Primary STUN servers',
        priority: 1,
        servers: [
          { urls: "stun:stun.l.google.com:19302" },
          { urls: "stun:stun1.l.google.com:19302" }
        ]
      },
      
      // Tier 2: Secondary Google STUN cluster
      {
        name: 'Google STUN cluster',
        priority: 2,
        servers: [
          { urls: "stun:stun2.l.google.com:19302" },
          { urls: "stun:stun3.l.google.com:19302" },
          { urls: "stun:stun4.l.google.com:19302" }
        ]
      },
      
      // Tier 3: Alternative public servers
      {
        name: 'Alternative public servers',
        priority: 3,
        servers: [
          { urls: "stun:stun.services.mozilla.com" },
          { urls: "stun:stun.stunprotocol.org:3478" }
        ]
      },
      
      // Tier 4: Cloudflare servers
      {
        name: 'Cloudflare servers',
        priority: 4,
        servers: [
          { urls: "stun:stun.cloudflare.com:3478" }
        ]
      },
      
      // Tier 5: Additional fallback servers
      {
        name: 'Additional fallbacks',
        priority: 5,
        servers: [
          { urls: "stun:stun.voiparound.com" },
          { urls: "stun:stun.voipbuster.com" },
          { urls: "stun:stun.voipstunt.com" }
        ]
      },
      
      // Tier 6: Self-hosted (when configured)
      {
        name: 'Self-hosted',
        priority: 6,
        servers: this.getSelfHostedServers()
      }
    ];
    
    this.currentTierIndex = 0;
    this.connectionAttempts = 0;
    this.maxRetries = 3;
    this.failedServers = new Set();
    
    this.logConnection('WebRTC Fallback Manager initialized');
  }
  
  /**
   * Get self-hosted servers if configured
   */
  getSelfHostedServers() {
    const servers = [];
    
    // Check if self-hosted STUN/TURN servers are configured
    if (window.CIPHER_CONFIG?.webrtc?.stunServer) {
      servers.push({ urls: window.CIPHER_CONFIG.webrtc.stunServer });
    }
    
    if (window.CIPHER_CONFIG?.webrtc?.turnServer) {
      servers.push({
        urls: window.CIPHER_CONFIG.webrtc.turnServer,
        username: window.CIPHER_CONFIG.webrtc.turnUsername || 'cipher-user',
        credential: window.CIPHER_CONFIG.webrtc.turnCredential || 'secure-password'
      });
    }
    
    return servers;
  }
  
  /**
   * Get current ICE servers configuration
   */
  getIceServers() {
    let servers = [];
    
    // Include all servers from tiers up to current tier
    for (let i = 0; i <= this.currentTierIndex && i < this.serverTiers.length; i++) {
      const tier = this.serverTiers[i];
      const validServers = tier.servers.filter(server => 
        !this.failedServers.has(server.urls)
      );
      servers.push(...validServers);
    }
    
    if (servers.length === 0) {
      this.logConnection('No valid servers available, using minimal fallback', 'warn');
      servers = [{ urls: "stun:stun.l.google.com:19302" }];
    }
    
    this.logConnection(`Using ${servers.length} ICE servers from tiers 0-${this.currentTierIndex}`);
    return servers;
  }
  
  /**
   * Handle connection failure and move to next tier
   */
  onConnectionFailed(failedServer = null) {
    if (failedServer) {
      this.failedServers.add(failedServer);
      this.logConnection(`Marked server as failed: ${failedServer}`, 'warn');
    }
    
    this.connectionAttempts++;
    
    // Try next tier if available
    if (this.currentTierIndex < this.serverTiers.length - 1) {
      this.currentTierIndex++;
      const currentTier = this.serverTiers[this.currentTierIndex];
      this.logConnection(`Falling back to tier ${this.currentTierIndex}: ${currentTier.name}`, 'warn');
      return true;
    }
    
    // All tiers exhausted
    this.logConnection('All STUN/TURN fallbacks exhausted', 'error');
    return false;
  }
  
  /**
   * Reset fallback state for new connection attempt
   */
  reset() {
    this.currentTierIndex = 0;
    this.connectionAttempts = 0;
    // Don't reset failedServers - keep that knowledge across attempts
    this.logConnection('Fallback manager reset for new connection');
  }
  
  /**
   * Create RTCPeerConnection with current ICE servers
   */
  createPeerConnection(additionalConfig = {}) {
    const config = {
      iceServers: this.getIceServers(),
      iceTransportPolicy: 'all', // Allow both STUN and TURN
      iceCandidatePoolSize: 10,   // Pre-gather candidates
      ...additionalConfig
    };
    
    const pc = new RTCPeerConnection(config);
    
    // Monitor ICE connection state
    pc.oniceconnectionstatechange = () => {
      this.handleIceConnectionStateChange(pc);
    };
    
    // Monitor ICE gathering state
    pc.onicegatheringstatechange = () => {
      this.logConnection(`ICE gathering state: ${pc.iceGatheringState}`);
    };
    
    // Log ICE candidates
    pc.onicecandidate = (event) => {
      if (event.candidate) {
        this.logConnection(`ICE candidate: ${event.candidate.type} ${event.candidate.protocol}`);
      } else {
        this.logConnection('ICE candidate gathering complete');
      }
    };
    
    return pc;
  }
  
  /**
   * Handle ICE connection state changes
   */
  handleIceConnectionStateChange(pc) {
    const state = pc.iceConnectionState;
    this.logConnection(`ICE connection state: ${state}`);
    
    switch (state) {
      case 'connected':
      case 'completed':
        this.logConnection('WebRTC connection established successfully', 'success');
        break;
        
      case 'disconnected':
        this.logConnection('WebRTC connection disconnected', 'warn');
        // Don't immediately fail - might reconnect
        break;
        
      case 'failed':
        this.logConnection('WebRTC connection failed', 'error');
        this.handleConnectionFailure(pc);
        break;
        
      case 'closed':
        this.logConnection('WebRTC connection closed');
        break;
    }
  }
  
  /**
   * Handle connection failure
   */
  handleConnectionFailure(pc) {
    if (this.onConnectionFailed()) {
      this.logConnection('Attempting reconnection with fallback servers');
      // Emit event for application to handle reconnection
      this.dispatchEvent('connectionFailedRetry', { 
        manager: this,
        peerConnection: pc 
      });
    } else {
      this.logConnection('No more fallbacks available', 'error');
      this.dispatchEvent('connectionFailedFinal', { 
        manager: this,
        peerConnection: pc 
      });
    }
  }
  
  /**
   * Test server connectivity
   */
  async testServer(serverConfig) {
    return new Promise((resolve) => {
      const testPc = new RTCPeerConnection({
        iceServers: [serverConfig],
        iceCandidatePoolSize: 1
      });
      
      let candidateReceived = false;
      const timeout = setTimeout(() => {
        testPc.close();
        resolve({ server: serverConfig, working: candidateReceived });
      }, 5000);
      
      testPc.onicecandidate = (event) => {
        if (event.candidate && event.candidate.type === 'srflx') {
          candidateReceived = true;
          clearTimeout(timeout);
          testPc.close();
          resolve({ server: serverConfig, working: true });
        }
      };
      
      // Create empty data channel to trigger ICE gathering
      testPc.createDataChannel('test');
      testPc.createOffer().then(offer => testPc.setLocalDescription(offer));
    });
  }
  
  /**
   * Test all servers and remove non-working ones
   */
  async validateServers() {
    this.logConnection('Testing STUN/TURN server connectivity...');
    
    for (const tier of this.serverTiers) {
      const results = await Promise.all(
        tier.servers.map(server => this.testServer(server))
      );
      
      results.forEach(result => {
        if (!result.working) {
          this.failedServers.add(result.server.urls);
          this.logConnection(`Server test failed: ${result.server.urls}`, 'warn');
        } else {
          this.logConnection(`Server test passed: ${result.server.urls}`, 'success');
        }
      });
    }
  }
  
  /**
   * Get connection statistics
   */
  getStats() {
    return {
      currentTier: this.currentTierIndex,
      currentTierName: this.serverTiers[this.currentTierIndex]?.name,
      connectionAttempts: this.connectionAttempts,
      failedServersCount: this.failedServers.size,
      availableServers: this.getIceServers().length,
      totalTiers: this.serverTiers.length
    };
  }
  
  /**
   * Logging helper
   */
  logConnection(message, level = 'info') {
    const timestamp = new Date().toISOString();
    const prefix = '[WebRTC Fallback]';
    
    switch (level) {
      case 'error':
        console.error(`${prefix} ${timestamp}: ${message}`);
        break;
      case 'warn':
        console.warn(`${prefix} ${timestamp}: ${message}`);
        break;
      case 'success':
        console.log(`${prefix} ${timestamp}: ✅ ${message}`);
        break;
      default:
        console.log(`${prefix} ${timestamp}: ${message}`);
    }
  }
  
  /**
   * Event dispatcher for application integration
   */
  dispatchEvent(eventName, detail) {
    const event = new CustomEvent(`webrtc-fallback-${eventName}`, { detail });
    window.dispatchEvent(event);
  }
}

/**
 * High-level WebRTC connection helper with automatic fallback
 */
class CipherWebRTCConnection {
  constructor() {
    this.fallbackManager = new WebRTCFallbackManager();
    this.peerConnection = null;
    this.connectionPromise = null;
    
    // Listen for fallback events
    window.addEventListener('webrtc-fallback-connectionFailedRetry', (event) => {
      this.handleConnectionRetry(event.detail);
    });
    
    window.addEventListener('webrtc-fallback-connectionFailedFinal', (event) => {
      this.handleConnectionFailure(event.detail);
    });
  }
  
  /**
   * Create a new peer connection with fallback support
   */
  async createConnection(config = {}) {
    if (this.connectionPromise) {
      return this.connectionPromise;
    }
    
    this.connectionPromise = this._attemptConnection(config);
    return this.connectionPromise;
  }
  
  async _attemptConnection(config) {
    try {
      // Test servers first (optional, can be disabled for faster startup)
      if (config.testServers !== false) {
        await this.fallbackManager.validateServers();
      }
      
      this.peerConnection = this.fallbackManager.createPeerConnection(config);
      
      // Wait for connection to establish or fail
      await this.waitForConnection();
      
      return this.peerConnection;
      
    } catch (error) {
      this.fallbackManager.logConnection(`Connection attempt failed: ${error.message}`, 'error');
      throw error;
    }
  }
  
  /**
   * Wait for WebRTC connection to establish
   */
  waitForConnection(timeoutMs = 30000) {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('Connection timeout'));
      }, timeoutMs);
      
      const checkConnection = () => {
        const state = this.peerConnection.iceConnectionState;
        
        if (state === 'connected' || state === 'completed') {
          clearTimeout(timeout);
          resolve(this.peerConnection);
        } else if (state === 'failed' || state === 'closed') {
          clearTimeout(timeout);
          reject(new Error(`Connection failed: ${state}`));
        }
      };
      
      this.peerConnection.addEventListener('iceconnectionstatechange', checkConnection);
      checkConnection(); // Check initial state
    });
  }
  
  /**
   * Handle connection retry with fallback
   */
  async handleConnectionRetry(detail) {
    this.fallbackManager.logConnection('Retrying connection with fallback servers');
    
    // Close old connection
    if (this.peerConnection) {
      this.peerConnection.close();
    }
    
    // Reset connection promise to allow new attempt
    this.connectionPromise = null;
    
    // Create new connection with fallback servers
    try {
      await this.createConnection({ testServers: false });
    } catch (error) {
      this.fallbackManager.logConnection(`Retry failed: ${error.message}`, 'error');
    }
  }
  
  /**
   * Handle final connection failure
   */
  handleConnectionFailure(detail) {
    this.fallbackManager.logConnection('All connection attempts failed', 'error');
    
    // Emit application-level event
    const event = new CustomEvent('cipher-webrtc-failed', {
      detail: {
        stats: this.fallbackManager.getStats(),
        error: 'All STUN/TURN servers failed'
      }
    });
    window.dispatchEvent(event);
  }
  
  /**
   * Get connection statistics
   */
  getStats() {
    return this.fallbackManager.getStats();
  }
  
  /**
   * Close connection and cleanup
   */
  close() {
    if (this.peerConnection) {
      this.peerConnection.close();
      this.peerConnection = null;
    }
    this.connectionPromise = null;
    this.fallbackManager.reset();
  }
}

// Global instance for easy access
window.CipherWebRTC = new CipherWebRTCConnection();

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { WebRTCFallbackManager, CipherWebRTCConnection };
}