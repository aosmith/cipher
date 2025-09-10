/**
 * Local Hosting System for Cipher
 * Manages local storage quota allocation and content hosting for earning CPH tokens
 */

class LocalHostingManager {
  constructor() {
    this.db = null;
    this.quota = {
      allocated: 0,    // MB allocated by user
      used: 0,        // MB currently used
      available: 0    // MB still available
    };
    this.hostingActive = false;
    this.earnings = {
      totalEarned: 0,
      pendingPayouts: 0,
      lastPayout: null
    };
    this.hostedFiles = new Map(); // fileHash -> fileData
    this.performanceMetrics = {
      uptime: 0,
      reliability: 100,
      bandwidth: 0
    };
    
    this.initializeDB();
  }

  // Initialize IndexedDB for local storage
  async initializeDB() {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open('CipherLocalHost', 1);
      
      request.onerror = () => reject(request.error);
      
      request.onsuccess = () => {
        this.db = request.result;
        this.loadStorageQuota();
        resolve();
      };
      
      request.onupgradeneeded = (event) => {
        const db = event.target.result;
        
        // Store hosted file content
        const filesStore = db.createObjectStore('hostedFiles', { keyPath: 'fileHash' });
        filesStore.createIndex('uploadTime', 'uploadTime', { unique: false });
        filesStore.createIndex('size', 'size', { unique: false });
        
        // Store hosting configuration and metrics
        const configStore = db.createObjectStore('hostingConfig', { keyPath: 'key' });
        
        // Store earnings and transaction history
        const earningsStore = db.createObjectStore('earnings', { keyPath: 'id', autoIncrement: true });
        earningsStore.createIndex('timestamp', 'timestamp', { unique: false });
        earningsStore.createIndex('type', 'type', { unique: false });
      };
    });
  }

  // Set storage quota allocation (in MB)
  async setStorageQuota(quotaMB) {
    if (quotaMB < 100) {
      throw new Error('Minimum quota is 100MB');
    }
    
    // Check available browser storage
    const estimate = await navigator.storage.estimate();
    const availableMB = Math.floor((estimate.quota - estimate.usage) / (1024 * 1024));
    
    if (quotaMB > availableMB * 0.8) { // Use max 80% of available space
      throw new Error(`Only ${Math.floor(availableMB * 0.8)}MB available. Please free up space or reduce quota.`);
    }
    
    this.quota.allocated = quotaMB;
    this.quota.available = quotaMB - this.quota.used;
    
    await this.saveConfig('storageQuota', {
      allocated: this.quota.allocated,
      used: this.quota.used,
      available: this.quota.available,
      setAt: Date.now()
    });
    
    this.onQuotaUpdated?.(this.quota);
    return this.quota;
  }

  // Load storage configuration from IndexedDB
  async loadStorageQuota() {
    const config = await this.getConfig('storageQuota');
    if (config) {
      this.quota = {
        allocated: config.allocated || 0,
        used: config.used || 0,
        available: config.available || 0
      };
    }
    
    // Load hosting status
    const hostingConfig = await this.getConfig('hostingActive');
    this.hostingActive = hostingConfig?.value || false;
    
    // Calculate current usage
    await this.calculateCurrentUsage();
    
    return this.quota;
  }

  // Calculate actual storage usage
  async calculateCurrentUsage() {
    const transaction = this.db.transaction(['hostedFiles'], 'readonly');
    const store = transaction.objectStore('hostedFiles');
    const request = store.getAll();
    
    return new Promise((resolve) => {
      request.onsuccess = () => {
        let totalUsed = 0;
        const files = request.result;
        
        files.forEach(file => {
          totalUsed += file.size || 0;
          this.hostedFiles.set(file.fileHash, file);
        });
        
        this.quota.used = Math.ceil(totalUsed / (1024 * 1024)); // Convert to MB
        this.quota.available = this.quota.allocated - this.quota.used;
        
        this.saveConfig('storageQuota', this.quota);
        resolve(this.quota);
      };
    });
  }

  // Start hosting (enable receiving and storing files)
  async startHosting() {
    if (this.quota.allocated < 100) {
      throw new Error('Please set a storage quota of at least 100MB before starting hosting');
    }
    
    this.hostingActive = true;
    await this.saveConfig('hostingActive', { value: true, startedAt: Date.now() });
    
    // Register with the network as an active host
    await this.registerWithNetwork();
    
    // Start performance monitoring
    this.startPerformanceMonitoring();
    
    this.onHostingStatusChanged?.(true);
    return true;
  }

  // Stop hosting
  async stopHosting() {
    this.hostingActive = false;
    await this.saveConfig('hostingActive', { value: false, stoppedAt: Date.now() });
    
    // Unregister from network
    await this.unregisterFromNetwork();
    
    // Stop performance monitoring
    this.stopPerformanceMonitoring();
    
    this.onHostingStatusChanged?.(false);
    return true;
  }

  // Store a file locally for hosting
  async storeFile(fileHash, encryptedData, metadata) {
    if (!this.hostingActive) {
      throw new Error('Hosting is not active');
    }
    
    const fileSizeMB = Math.ceil(encryptedData.byteLength / (1024 * 1024));
    
    if (fileSizeMB > this.quota.available) {
      throw new Error(`Insufficient storage space. Need ${fileSizeMB}MB, have ${this.quota.available}MB available.`);
    }
    
    const fileData = {
      fileHash,
      encryptedData: Array.from(new Uint8Array(encryptedData)), // Store as array for IndexedDB
      metadata: {
        ...metadata,
        storedAt: Date.now(),
        size: encryptedData.byteLength,
        hostId: await this.getHostId()
      }
    };
    
    // Store in IndexedDB
    const transaction = this.db.transaction(['hostedFiles'], 'readwrite');
    const store = transaction.objectStore('hostedFiles');
    
    await new Promise((resolve, reject) => {
      const request = store.put(fileData);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
    
    // Update quota usage
    this.quota.used += fileSizeMB;
    this.quota.available -= fileSizeMB;
    this.hostedFiles.set(fileHash, fileData);
    
    await this.saveConfig('storageQuota', this.quota);
    
    // Record hosting event for earnings calculation
    await this.recordHostingEvent('file_stored', {
      fileHash,
      size: encryptedData.byteLength,
      timestamp: Date.now()
    });
    
    this.onFileStored?.(fileHash, fileSizeMB);
    return fileData;
  }

  // Retrieve a hosted file
  async retrieveFile(fileHash) {
    if (!this.hostedFiles.has(fileHash)) {
      // Try loading from IndexedDB
      const transaction = this.db.transaction(['hostedFiles'], 'readonly');
      const store = transaction.objectStore('hostedFiles');
      const request = store.get(fileHash);
      
      const fileData = await new Promise((resolve) => {
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => resolve(null);
      });
      
      if (!fileData) {
        throw new Error('File not found');
      }
      
      this.hostedFiles.set(fileHash, fileData);
    }
    
    const fileData = this.hostedFiles.get(fileHash);
    
    // Record bandwidth event for earnings
    await this.recordHostingEvent('file_served', {
      fileHash,
      size: fileData.metadata.size,
      timestamp: Date.now()
    });
    
    // Convert array back to Uint8Array
    const encryptedData = new Uint8Array(fileData.encryptedData);
    
    this.onFileServed?.(fileHash, Math.ceil(fileData.metadata.size / (1024 * 1024)));
    return encryptedData.buffer;
  }

  // Delete a hosted file
  async deleteFile(fileHash) {
    const fileData = this.hostedFiles.get(fileHash);
    if (!fileData) {
      return false;
    }
    
    // Remove from IndexedDB
    const transaction = this.db.transaction(['hostedFiles'], 'readwrite');
    const store = transaction.objectStore('hostedFiles');
    
    await new Promise((resolve) => {
      const request = store.delete(fileHash);
      request.onsuccess = () => resolve();
      request.onerror = () => resolve();
    });
    
    // Update quota
    const fileSizeMB = Math.ceil(fileData.metadata.size / (1024 * 1024));
    this.quota.used -= fileSizeMB;
    this.quota.available += fileSizeMB;
    
    this.hostedFiles.delete(fileHash);
    
    await this.saveConfig('storageQuota', this.quota);
    
    this.onFileDeleted?.(fileHash, fileSizeMB);
    return true;
  }

  // Register as host with the network
  async registerWithNetwork() {
    try {
      const hostInfo = {
        hostId: await this.getHostId(),
        capacityMB: this.quota.allocated,
        availableMB: this.quota.available,
        reliability: this.performanceMetrics.reliability,
        endpoint: await this.getLocalEndpoint()
      };
      
      // Register with blockchain if wallet connected
      if (window.cipherWallet?.isInitialized) {
        await window.cipherWallet.token.registerAsHost(this.quota.allocated * 1024); // Convert MB to KB
      }
      
      // Register with P2P network
      if (window.cipherP2P) {
        await window.cipherP2P.announceHosting(hostInfo);
      }
      
      return hostInfo;
    } catch (error) {
      console.error('Failed to register with network:', error);
      throw error;
    }
  }

  // Unregister from network
  async unregisterFromNetwork() {
    try {
      // Unregister from blockchain
      if (window.cipherWallet?.isInitialized) {
        await window.cipherWallet.token.unregisterAsHost();
      }
      
      // Unregister from P2P network
      if (window.cipherP2P) {
        await window.cipherP2P.stopHosting();
      }
    } catch (error) {
      console.error('Failed to unregister from network:', error);
    }
  }

  // Get hosting statistics
  getHostingStats() {
    return {
      quota: { ...this.quota },
      hostingActive: this.hostingActive,
      filesHosted: this.hostedFiles.size,
      earnings: { ...this.earnings },
      performance: { ...this.performanceMetrics },
      efficiency: this.quota.allocated > 0 ? (this.quota.used / this.quota.allocated * 100).toFixed(1) : 0
    };
  }

  // Start performance monitoring
  startPerformanceMonitoring() {
    // Monitor uptime
    this.uptimeStart = Date.now();
    this.uptimeInterval = setInterval(() => {
      this.performanceMetrics.uptime = Date.now() - this.uptimeStart;
    }, 60000); // Update every minute
    
    // Monitor connection quality
    this.monitorConnection();
  }

  stopPerformanceMonitoring() {
    if (this.uptimeInterval) {
      clearInterval(this.uptimeInterval);
    }
  }

  async monitorConnection() {
    if ('connection' in navigator) {
      const connection = navigator.connection;
      this.performanceMetrics.bandwidth = connection.downlink || 0;
      
      connection.addEventListener('change', () => {
        this.performanceMetrics.bandwidth = connection.downlink || 0;
      });
    }
  }

  // Record hosting events for earnings calculation
  async recordHostingEvent(type, data) {
    const event = {
      type,
      data,
      timestamp: Date.now(),
      earnedAmount: this.calculateEventEarnings(type, data)
    };
    
    const transaction = this.db.transaction(['earnings'], 'readwrite');
    const store = transaction.objectStore('earnings');
    
    await new Promise((resolve) => {
      const request = store.add(event);
      request.onsuccess = () => resolve();
      request.onerror = () => resolve();
    });
    
    this.earnings.totalEarned += event.earnedAmount;
    this.earnings.pendingPayouts += event.earnedAmount;
  }

  // Calculate earnings for hosting events
  calculateEventEarnings(type, data) {
    const sizeKB = Math.ceil(data.size / 1024);
    
    switch (type) {
      case 'file_stored':
        // Earn from storage: base rate per KB per day
        return sizeKB * 0.001; // 0.001 CPH per KB per day
      case 'file_served':
        // Earn from bandwidth: per KB served
        return sizeKB * 0.8; // 80% of 1 CPH per KB
      default:
        return 0;
    }
  }

  // Helper methods
  async getHostId() {
    let hostId = await this.getConfig('hostId');
    if (!hostId) {
      // Generate unique host ID
      const array = new Uint8Array(16);
      crypto.getRandomValues(array);
      hostId = Array.from(array, byte => byte.toString(16).padStart(2, '0')).join('');
      await this.saveConfig('hostId', hostId);
    }
    return hostId.value || hostId;
  }

  async getLocalEndpoint() {
    // In a real P2P implementation, this would return the WebRTC endpoint
    return `local-${await this.getHostId()}`;
  }

  async saveConfig(key, value) {
    const transaction = this.db.transaction(['hostingConfig'], 'readwrite');
    const store = transaction.objectStore('hostingConfig');
    
    return new Promise((resolve) => {
      const request = store.put({ key, value });
      request.onsuccess = () => resolve();
      request.onerror = () => resolve();
    });
  }

  async getConfig(key) {
    const transaction = this.db.transaction(['hostingConfig'], 'readonly');
    const store = transaction.objectStore('hostingConfig');
    
    return new Promise((resolve) => {
      const request = store.get(key);
      request.onsuccess = () => resolve(request.result?.value);
      request.onerror = () => resolve(null);
    });
  }

  // Event handlers (can be overridden)
  onQuotaUpdated = null;
  onHostingStatusChanged = null;
  onFileStored = null;
  onFileServed = null;
  onFileDeleted = null;
}

// Export for ES6 modules
export default LocalHostingManager;

// Also make available globally
window.LocalHostingManager = LocalHostingManager;