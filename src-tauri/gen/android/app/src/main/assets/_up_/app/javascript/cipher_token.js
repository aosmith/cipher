/**
 * CIPHER Token integration for Web3 payments
 * Handles token operations, file costs, and smart contract interactions
 */

import Web3Utils from 'web3_utils';
import { ethers } from 'ethers';

class CipherToken {
  constructor() {
    this.web3 = new Web3Utils();
    this.tokenContract = null;
    this.storageContract = null;
    this.isInitialized = false;
    
    // Contract addresses (will be set after deployment)
    this.addresses = {
      token: null,
      storage: null
    };
    
    // Contract ABIs (simplified for demo)
    this.tokenABI = [
      'function name() view returns (string)',
      'function symbol() view returns (string)',
      'function decimals() view returns (uint8)',
      'function totalSupply() view returns (uint256)',
      'function balanceOf(address owner) view returns (uint256)',
      'function transfer(address to, uint256 amount) returns (bool)',
      'function approve(address spender, uint256 amount) returns (bool)',
      'function allowance(address owner, address spender) view returns (uint256)',
      'function calculateCost(uint256 fileSizeBytes) view returns (uint256)',
      'event Transfer(address indexed from, address indexed to, uint256 value)',
      'event Approval(address indexed owner, address indexed spender, uint256 value)'
    ];
    
    this.storageABI = [
      'function uploadFile(bytes32 fileHash, uint256 sizeKB) external',
      'function downloadFile(bytes32 fileHash) external',
      'function deposit(uint256 amount) external',
      'function withdraw(uint256 amount) external',
      'function registerHost(uint256 storageCapacityKB) external',
      'function unregisterHost() external',
      'function claimRewards() external',
      'function userBalances(address user) view returns (uint256, uint256, uint256, uint256)',
      'function hosts(address host) view returns (uint256, uint256, uint256, uint256, uint256, uint256, bool)',
      'function files(bytes32 fileHash) view returns (address, uint256, uint256, uint256, uint256, bool)',
      'function getUserFiles(address user) view returns (bytes32[])',
      'function getActiveHostsCount() view returns (uint256)',
      'event FileUploaded(bytes32 indexed fileHash, address indexed owner, uint256 sizeKB, uint256 cost, address[] hosts)',
      'event FileDownloaded(bytes32 indexed fileHash, address indexed downloader, uint256 cost)',
      'event HostRegistered(address indexed host, uint256 stakedAmount, uint256 storageCapacityKB)',
      'event UserDeposit(address indexed user, uint256 amount)'
    ];
  }

  // Initialize contracts with deployed addresses
  async initialize(tokenAddress, storageAddress) {
    if (!tokenAddress || !storageAddress) {
      throw new Error('Contract addresses required');
    }

    this.addresses.token = tokenAddress;
    this.addresses.storage = storageAddress;

    // Connect wallet if not already connected
    if (!this.web3.isConnected) {
      await this.web3.connectWallet();
    }

    // Initialize contract instances
    this.tokenContract = this.web3.getContract(tokenAddress, this.tokenABI, false);
    this.storageContract = this.web3.getContract(storageAddress, this.storageABI, false);
    
    this.isInitialized = true;
  }

  // Check if system is ready
  checkInitialized() {
    if (!this.isInitialized) {
      throw new Error('CipherToken not initialized. Call initialize() first.');
    }
  }

  // Get user's CPH token balance
  async getBalance(address = null) {
    this.checkInitialized();
    const userAddress = address || this.web3.getAccount();
    if (!userAddress) return '0';

    try {
      const balance = await this.tokenContract.balanceOf(userAddress);
      return this.web3.formatTokenAmount(balance, 18, 2);
    } catch (error) {
      console.error('Failed to get CPH balance:', error);
      return '0';
    }
  }

  // Get user's deposit balance and stats from storage contract
  async getUserStats(address = null) {
    this.checkInitialized();
    const userAddress = address || this.web3.getAccount();
    if (!userAddress) return null;

    try {
      const stats = await this.storageContract.userBalances(userAddress);
      return {
        depositedAmount: this.web3.formatTokenAmount(stats[0], 18, 2),
        totalSpent: this.web3.formatTokenAmount(stats[1], 18, 2),
        totalUploaded: stats[2].toString(), // KB
        totalDownloaded: stats[3].toString() // KB
      };
    } catch (error) {
      console.error('Failed to get user stats:', error);
      return null;
    }
  }

  // Calculate upload cost for file
  calculateUploadCost(fileSizeBytes) {
    return this.web3.calculateUploadCost(fileSizeBytes);
  }

  // Calculate download cost for file
  calculateDownloadCost(fileSizeBytes) {
    return this.web3.calculateDownloadCost(fileSizeBytes);
  }

  // Format cost for display
  formatCost(costInCPH) {
    return this.web3.formatCost(costInCPH);
  }

  // Approve token spending for storage contract
  async approveSpending(amount) {
    this.checkInitialized();
    const signer = this.web3.signer;
    const tokenWithSigner = this.tokenContract.connect(signer);

    try {
      const amountWei = this.web3.parseTokenAmount(amount, 18);
      const tx = await tokenWithSigner.approve(this.addresses.storage, amountWei);
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to approve spending:', error);
      throw error;
    }
  }

  // Check if user has approved enough tokens
  async checkAllowance(amount) {
    this.checkInitialized();
    const userAddress = this.web3.getAccount();
    if (!userAddress) return false;

    try {
      const allowance = await this.tokenContract.allowance(userAddress, this.addresses.storage);
      const amountWei = this.web3.parseTokenAmount(amount, 18);
      return allowance.gte(amountWei);
    } catch (error) {
      console.error('Failed to check allowance:', error);
      return false;
    }
  }

  // Deposit CPH tokens for future use
  async deposit(amount) {
    this.checkInitialized();
    const signer = this.web3.signer;
    const storageWithSigner = this.storageContract.connect(signer);

    try {
      // First approve if needed
      const hasAllowance = await this.checkAllowance(amount);
      if (!hasAllowance) {
        await this.approveSpending(amount);
      }

      // Then deposit
      const amountWei = this.web3.parseTokenAmount(amount, 18);
      const tx = await storageWithSigner.deposit(amountWei);
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to deposit:', error);
      throw error;
    }
  }

  // Withdraw unused CPH tokens
  async withdraw(amount) {
    this.checkInitialized();
    const signer = this.web3.signer;
    const storageWithSigner = this.storageContract.connect(signer);

    try {
      const amountWei = this.web3.parseTokenAmount(amount, 18);
      const tx = await storageWithSigner.withdraw(amountWei);
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to withdraw:', error);
      throw error;
    }
  }

  // Upload file with payment
  async uploadFile(fileHash, fileSizeBytes) {
    this.checkInitialized();
    const signer = this.web3.signer;
    const storageWithSigner = this.storageContract.connect(signer);

    try {
      const sizeKB = this.web3.calculateFileSizeKB(fileSizeBytes);
      const cost = this.calculateUploadCost(fileSizeBytes);

      // Check if user has enough balance (deposit + wallet)
      const stats = await this.getUserStats();
      const balance = await this.getBalance();
      const totalAvailable = parseFloat(stats?.depositedAmount || '0') + parseFloat(balance);

      if (totalAvailable < cost) {
        throw new Error(`Insufficient balance. Need ${cost} CPH, have ${totalAvailable.toFixed(2)} CPH`);
      }

      // If paying directly (not from deposit), need to approve
      const depositedAmount = parseFloat(stats?.depositedAmount || '0');
      if (depositedAmount < cost) {
        const directPayment = cost - depositedAmount;
        const hasAllowance = await this.checkAllowance(directPayment);
        if (!hasAllowance) {
          await this.approveSpending(directPayment);
        }
      }

      // Upload file
      const fileHashBytes32 = ethers.utils.formatBytes32String(fileHash);
      const tx = await storageWithSigner.uploadFile(fileHashBytes32, sizeKB);
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to upload file:', error);
      throw error;
    }
  }

  // Download file with payment
  async downloadFile(fileHash) {
    this.checkInitialized();
    const signer = this.web3.signer;
    const storageWithSigner = this.storageContract.connect(signer);

    try {
      // Get file info to calculate cost
      const fileHashBytes32 = ethers.utils.formatBytes32String(fileHash);
      const fileInfo = await this.storageContract.files(fileHashBytes32);
      
      if (!fileInfo[5]) { // exists field
        throw new Error('File does not exist');
      }

      const sizeKB = fileInfo[1].toNumber();
      const cost = sizeKB; // 1 CPH per KB

      // Check balance
      const stats = await this.getUserStats();
      const balance = await this.getBalance();
      const totalAvailable = parseFloat(stats?.depositedAmount || '0') + parseFloat(balance);

      if (totalAvailable < cost) {
        throw new Error(`Insufficient balance. Need ${cost} CPH, have ${totalAvailable.toFixed(2)} CPH`);
      }

      // Handle payment approval if needed
      const depositedAmount = parseFloat(stats?.depositedAmount || '0');
      if (depositedAmount < cost) {
        const directPayment = cost - depositedAmount;
        const hasAllowance = await this.checkAllowance(directPayment);
        if (!hasAllowance) {
          await this.approveSpending(directPayment);
        }
      }

      // Download file
      const tx = await storageWithSigner.downloadFile(fileHashBytes32);
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to download file:', error);
      throw error;
    }
  }

  // Register as storage host
  async registerAsHost(storageCapacityKB) {
    this.checkInitialized();
    const signer = this.web3.signer;
    const storageWithSigner = this.storageContract.connect(signer);

    try {
      // Check if user has minimum stake (1000 CPH)
      const balance = await this.getBalance();
      const minimumStake = 1000;

      if (parseFloat(balance) < minimumStake) {
        throw new Error(`Insufficient balance for staking. Need ${minimumStake} CPH, have ${balance} CPH`);
      }

      // Approve staking amount
      const hasAllowance = await this.checkAllowance(minimumStake);
      if (!hasAllowance) {
        await this.approveSpending(minimumStake);
      }

      // Register as host
      const tx = await storageWithSigner.registerHost(storageCapacityKB);
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to register as host:', error);
      throw error;
    }
  }

  // Unregister as host
  async unregisterAsHost() {
    this.checkInitialized();
    const signer = this.web3.signer;
    const storageWithSigner = this.storageContract.connect(signer);

    try {
      const tx = await storageWithSigner.unregisterHost();
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to unregister as host:', error);
      throw error;
    }
  }

  // Claim host rewards
  async claimHostRewards() {
    this.checkInitialized();
    const signer = this.web3.signer;
    const storageWithSigner = this.storageContract.connect(signer);

    try {
      const tx = await storageWithSigner.claimRewards();
      return await this.web3.waitForTransaction(tx.hash);
    } catch (error) {
      console.error('Failed to claim rewards:', error);
      throw error;
    }
  }

  // Get host information
  async getHostInfo(address = null) {
    this.checkInitialized();
    const hostAddress = address || this.web3.getAccount();
    if (!hostAddress) return null;

    try {
      const hostInfo = await this.storageContract.hosts(hostAddress);
      return {
        stakedAmount: this.web3.formatTokenAmount(hostInfo[0], 18, 2),
        storageCapacityKB: hostInfo[1].toString(),
        usedStorageKB: hostInfo[2].toString(),
        totalEarnings: this.web3.formatTokenAmount(hostInfo[3], 18, 2),
        reliabilityScore: hostInfo[4].toString(),
        registrationTime: new Date(hostInfo[5].toNumber() * 1000),
        isActive: hostInfo[6]
      };
    } catch (error) {
      console.error('Failed to get host info:', error);
      return null;
    }
  }

  // Get network statistics
  async getNetworkStats() {
    this.checkInitialized();

    try {
      const activeHostsCount = await this.storageContract.getActiveHostsCount();
      const tokenInfo = await this.tokenContract.name();
      
      return {
        activeHosts: activeHostsCount.toString(),
        tokenName: tokenInfo,
        costPerKB: '1 CPH'
      };
    } catch (error) {
      console.error('Failed to get network stats:', error);
      return null;
    }
  }

  // Event listeners for real-time updates
  setupEventListeners() {
    if (!this.storageContract) return;

    // Listen for file uploads
    this.storageContract.on('FileUploaded', (fileHash, owner, sizeKB, cost, hosts) => {
      this.onFileUploaded?.(fileHash, owner, sizeKB.toString(), cost.toString(), hosts);
    });

    // Listen for file downloads
    this.storageContract.on('FileDownloaded', (fileHash, downloader, cost) => {
      this.onFileDownloaded?.(fileHash, downloader, cost.toString());
    });

    // Listen for deposits
    this.storageContract.on('UserDeposit', (user, amount) => {
      this.onUserDeposit?.(user, this.web3.formatTokenAmount(amount, 18, 2));
    });

    // Listen for host registrations
    this.storageContract.on('HostRegistered', (host, stakedAmount, storageCapacity) => {
      this.onHostRegistered?.(host, stakedAmount.toString(), storageCapacity.toString());
    });
  }

  // Event handler placeholders (can be overridden)
  onFileUploaded = null;
  onFileDownloaded = null;
  onUserDeposit = null;
  onHostRegistered = null;
}

// Export for ES6 modules
export default CipherToken;

// Also make available globally
window.CipherToken = CipherToken;