/**
 * Blockchain integration for file uploads and downloads
 * Integrates with existing file handling to add payment functionality
 */

import CipherToken from 'cipher_token';

class BlockchainFileManager {
  constructor() {
    this.token = null;
    this.isInitialized = false;
    
    // Initialize when wallet is connected
    this.setupWalletIntegration();
  }
  
  async setupWalletIntegration() {
    // Wait for wallet manager to be available
    if (typeof window.cipherWallet !== 'undefined') {
      this.token = window.cipherWallet.token;
      this.isInitialized = window.cipherWallet.isInitialized;
      
      // Listen for initialization
      const checkInit = setInterval(() => {
        if (window.cipherWallet.isInitialized) {
          this.token = window.cipherWallet.token;
          this.isInitialized = true;
          this.setupFileUploadIntegration();
          clearInterval(checkInit);
        }
      }, 1000);
    }
  }
  
  setupFileUploadIntegration() {
    // Integrate with existing file upload forms
    const fileInputs = document.querySelectorAll('input[type="file"]');
    fileInputs.forEach(input => {
      input.addEventListener('change', (event) => {
        this.handleFileSelection(event);
      });
    });
    
    // Integrate with drag-and-drop areas
    const dropZones = document.querySelectorAll('.file-drop-zone, .attachment-drop-zone');
    dropZones.forEach(zone => {
      zone.addEventListener('drop', (event) => {
        event.preventDefault();
        const files = Array.from(event.dataTransfer.files);
        this.handleMultipleFiles(files);
      });
      
      zone.addEventListener('dragover', (event) => {
        event.preventDefault();
        zone.classList.add('drag-over');
      });
      
      zone.addEventListener('dragleave', () => {
        zone.classList.remove('drag-over');
      });
    });
  }
  
  async handleFileSelection(event) {
    const files = Array.from(event.target.files);
    await this.handleMultipleFiles(files);
  }
  
  async handleMultipleFiles(files) {
    for (const file of files) {
      await this.processFileWithBlockchain(file);
    }
  }
  
  async processFileWithBlockchain(file) {
    try {
      // Calculate cost before upload
      const cost = await this.calculateUploadCost(file.size);
      
      // Show cost confirmation dialog
      const userConfirmed = await this.showCostConfirmation(file, cost);
      if (!userConfirmed) {
        return;
      }
      
      // Check if user has sufficient balance
      const canAfford = await this.checkSufficientBalance(cost);
      if (!canAfford) {
        this.showInsufficientBalanceDialog(cost);
        return;
      }
      
      // Generate file hash for blockchain
      const fileHash = await this.generateFileHash(file);
      
      // Show upload progress
      this.showUploadProgress(file, fileHash, cost);
      
      // Upload to blockchain first
      try {
        const txReceipt = await this.uploadToBlockchain(fileHash, file.size);
        
        // If blockchain upload succeeds, proceed with regular file upload
        await this.uploadFileWithBlockchainData(file, fileHash, cost, txReceipt);
        
        this.showUploadSuccess(file, cost, txReceipt.transactionHash);
        
      } catch (blockchainError) {
        console.error('Blockchain upload failed:', blockchainError);
        this.showUploadError(file, blockchainError.message);
      }
      
    } catch (error) {
      console.error('File processing failed:', error);
      this.showUploadError(file, error.message);
    }
  }
  
  async calculateUploadCost(fileSize) {
    if (this.isInitialized && this.token) {
      return this.token.calculateUploadCost(fileSize);
    }
    
    // Fallback calculation
    return Math.ceil(fileSize / 1024); // 1 CPH per KB
  }
  
  async checkSufficientBalance(requiredCost) {
    if (!this.isInitialized || !this.token) {
      return false;
    }
    
    try {
      const balance = parseFloat(await this.token.getBalance());
      const userStats = await this.token.getUserStats();
      const depositBalance = parseFloat(userStats?.depositedAmount || '0');
      
      const totalAvailable = balance + depositBalance;
      return totalAvailable >= requiredCost;
    } catch (error) {
      console.error('Failed to check balance:', error);
      return false;
    }
  }
  
  async generateFileHash(file) {
    return new Promise((resolve) => {
      const reader = new FileReader();
      reader.onload = async (event) => {
        const arrayBuffer = event.target.result;
        const hashBuffer = await crypto.subtle.digest('SHA-256', arrayBuffer);
        const hashArray = Array.from(new Uint8Array(hashBuffer));
        const hashHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
        resolve(hashHex);
      };
      reader.readAsArrayBuffer(file);
    });
  }
  
  async uploadToBlockchain(fileHash, fileSize) {
    if (!this.isInitialized || !this.token) {
      throw new Error('Blockchain not initialized');
    }
    
    return await this.token.uploadFile(fileHash, fileSize);
  }
  
  async uploadFileWithBlockchainData(file, fileHash, cost, txReceipt) {
    // Create form data for traditional file upload
    const formData = new FormData();
    formData.append('file', file);
    formData.append('blockchain_file_hash', fileHash);
    formData.append('blockchain_cost', cost.toString());
    formData.append('blockchain_tx_hash', txReceipt.transactionHash);
    
    // Submit to Rails backend
    const response = await fetch('/api/v1/attachments', {
      method: 'POST',
      body: formData,
      headers: {
        'X-Requested-With': 'XMLHttpRequest'
      }
    });
    
    if (!response.ok) {
      throw new Error(`Upload failed: ${response.statusText}`);
    }
    
    return await response.json();
  }
  
  async downloadFileWithPayment(fileHash) {
    if (!this.isInitialized || !this.token) {
      throw new Error('Blockchain not initialized');
    }
    
    try {
      // Pay for download on blockchain
      const txReceipt = await this.token.downloadFile(fileHash);
      
      // Record the transaction
      await this.recordTransaction({
        hash: txReceipt.transactionHash,
        type: 'download',
        file_hash: fileHash,
        wallet_address: window.cipherWallet.web3.getAccount(),
        block_number: txReceipt.blockNumber,
        gas_used: txReceipt.gasUsed
      });
      
      return txReceipt;
    } catch (error) {
      console.error('Download payment failed:', error);
      throw error;
    }
  }
  
  async recordTransaction(transactionData) {
    try {
      await fetch('/api/v1/blockchain/record_transaction', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Requested-With': 'XMLHttpRequest'
        },
        body: JSON.stringify({ transaction: transactionData })
      });
    } catch (error) {
      console.error('Failed to record transaction:', error);
    }
  }
  
  // UI Methods
  async showCostConfirmation(file, cost) {
    return new Promise((resolve) => {
      const modal = this.createModal(`
        <div class="blockchain-cost-confirmation">
          <h3>Upload Cost Confirmation</h3>
          <div class="file-info">
            <strong>File:</strong> ${file.name}<br>
            <strong>Size:</strong> ${this.formatFileSize(file.size)}<br>
            <strong>Cost:</strong> ${this.formatCost(cost)}
          </div>
          <p>This file will be stored on the decentralized network. Do you want to proceed?</p>
          <div class="modal-actions">
            <button class="btn btn-secondary cancel-btn">Cancel</button>
            <button class="btn btn-primary confirm-btn">Pay ${this.formatCost(cost)} & Upload</button>
          </div>
        </div>
      `);
      
      modal.querySelector('.confirm-btn').onclick = () => {
        modal.remove();
        resolve(true);
      };
      
      modal.querySelector('.cancel-btn').onclick = () => {
        modal.remove();
        resolve(false);
      };
    });
  }
  
  showInsufficientBalanceDialog(requiredCost) {
    const modal = this.createModal(`
      <div class="blockchain-insufficient-balance">
        <h3>Insufficient Balance</h3>
        <p>You need ${this.formatCost(requiredCost)} to upload this file.</p>
        <p>Please deposit more CPH tokens or connect a wallet with sufficient balance.</p>
        <div class="modal-actions">
          <button class="btn btn-secondary close-btn">Close</button>
          <button class="btn btn-primary deposit-btn">Deposit Tokens</button>
        </div>
      </div>
    `);
    
    modal.querySelector('.close-btn').onclick = () => modal.remove();
    modal.querySelector('.deposit-btn').onclick = () => {
      modal.remove();
      window.cipherWallet?.showDepositModal();
    };
  }
  
  showUploadProgress(file, fileHash, cost) {
    const progressId = `upload-progress-${Date.now()}`;
    const progressHtml = `
      <div id="${progressId}" class="upload-progress">
        <div class="progress-header">
          <strong>Uploading: ${file.name}</strong>
          <span class="progress-cost">${this.formatCost(cost)}</span>
        </div>
        <div class="progress-bar">
          <div class="progress-fill" style="width: 10%"></div>
        </div>
        <div class="progress-status">Paying on blockchain...</div>
      </div>
    `;
    
    this.showNotification(progressHtml, 'progress', 0); // 0 = don't auto-remove
    return progressId;
  }
  
  showUploadSuccess(file, cost, txHash) {
    const successHtml = `
      <div class="upload-success">
        <h4>✅ Upload Successful</h4>
        <p><strong>${file.name}</strong> uploaded for ${this.formatCost(cost)}</p>
        <p class="tx-hash">Transaction: ${txHash.slice(0, 10)}...${txHash.slice(-8)}</p>
      </div>
    `;
    
    this.showNotification(successHtml, 'success');
  }
  
  showUploadError(file, errorMessage) {
    const errorHtml = `
      <div class="upload-error">
        <h4>❌ Upload Failed</h4>
        <p><strong>${file.name}</strong></p>
        <p class="error-message">${errorMessage}</p>
      </div>
    `;
    
    this.showNotification(errorHtml, 'error');
  }
  
  // Utility methods
  createModal(content) {
    const modal = document.createElement('div');
    modal.className = 'blockchain-modal-overlay';
    modal.innerHTML = `
      <div class="blockchain-modal">
        ${content}
      </div>
    `;
    
    modal.style.cssText = `
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: rgba(0, 0, 0, 0.7);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 10000;
    `;
    
    document.body.appendChild(modal);
    return modal;
  }
  
  showNotification(content, type = 'info', autoRemove = 5000) {
    const notification = document.createElement('div');
    notification.className = `blockchain-notification blockchain-notification-${type}`;
    notification.innerHTML = content;
    notification.style.cssText = `
      position: fixed;
      top: 20px;
      right: 20px;
      background: ${type === 'success' ? '#48bb78' : type === 'error' ? '#f56565' : type === 'progress' ? '#4299e1' : '#4299e1'};
      color: white;
      padding: 16px;
      border-radius: 8px;
      z-index: 9999;
      max-width: 350px;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    `;
    
    document.body.appendChild(notification);
    
    if (autoRemove > 0) {
      setTimeout(() => {
        notification.remove();
      }, autoRemove);
    }
    
    return notification;
  }
  
  formatFileSize(bytes) {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;
    
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }
    
    return `${size.toFixed(1)} ${units[unitIndex]}`;
  }
  
  formatCost(cost) {
    if (cost >= 1000000) {
      return `${(cost / 1000000).toFixed(2)}M CPH`;
    } else if (cost >= 1000) {
      return `${(cost / 1000).toFixed(2)}K CPH`;
    } else {
      return `${cost} CPH`;
    }
  }
}

// Initialize the blockchain file manager
document.addEventListener('DOMContentLoaded', () => {
  window.blockchainFileManager = new BlockchainFileManager();
});

// Export for modules
export default BlockchainFileManager;