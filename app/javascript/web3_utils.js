/**
 * Web3 utilities for Cipher blockchain integration
 * Handles wallet connections, network setup, and contract interactions
 */

import { ethers } from 'ethers';

class Web3Utils {
  constructor() {
    this.provider = null;
    this.signer = null;
    this.address = null;
    this.isConnected = false;
    this.networkConfig = this.getNetworkConfig();
  }

  // Network configurations
  getNetworkConfig() {
    return {
      polygon: {
        chainId: '0x89', // 137 in decimal
        chainName: 'Polygon Mainnet',
        rpcUrls: ['https://polygon-rpc.com/'],
        nativeCurrency: {
          name: 'MATIC',
          symbol: 'MATIC',
          decimals: 18
        },
        blockExplorerUrls: ['https://polygonscan.com/']
      },
      polygonTestnet: {
        chainId: '0x13881', // 80001 in decimal
        chainName: 'Polygon Mumbai',
        rpcUrls: ['https://rpc-mumbai.maticvigil.com/'],
        nativeCurrency: {
          name: 'MATIC',
          symbol: 'MATIC',
          decimals: 18
        },
        blockExplorerUrls: ['https://mumbai.polygonscan.com/']
      }
    };
  }

  // Check if MetaMask is available
  isMetaMaskAvailable() {
    return typeof window.ethereum !== 'undefined';
  }

  // Connect to wallet
  async connectWallet() {
    if (!this.isMetaMaskAvailable()) {
      throw new Error('MetaMask not found. Please install MetaMask to use Cipher tokens.');
    }

    try {
      // Request account access
      const accounts = await window.ethereum.request({
        method: 'eth_requestAccounts'
      });

      if (accounts.length === 0) {
        throw new Error('No accounts found');
      }

      // Set up provider and signer
      this.provider = new ethers.providers.Web3Provider(window.ethereum);
      this.signer = this.provider.getSigner();
      this.address = accounts[0];
      this.isConnected = true;

      // Ensure we're on the right network
      await this.switchToPolygon();

      // Listen for account changes
      window.ethereum.on('accountsChanged', (accounts) => {
        if (accounts.length === 0) {
          this.disconnect();
        } else {
          this.address = accounts[0];
          this.onAccountChanged?.(accounts[0]);
        }
      });

      // Listen for network changes
      window.ethereum.on('chainChanged', (chainId) => {
        window.location.reload(); // Reload on network change
      });

      return {
        address: this.address,
        network: await this.provider.getNetwork()
      };

    } catch (error) {
      console.error('Failed to connect wallet:', error);
      throw error;
    }
  }

  // Switch to Polygon network
  async switchToPolygon() {
    const targetNetwork = this.networkConfig.polygon;
    
    try {
      // Try to switch to Polygon
      await window.ethereum.request({
        method: 'wallet_switchEthereumChain',
        params: [{ chainId: targetNetwork.chainId }],
      });
    } catch (switchError) {
      // If network doesn't exist, add it
      if (switchError.code === 4902) {
        try {
          await window.ethereum.request({
            method: 'wallet_addEthereumChain',
            params: [targetNetwork],
          });
        } catch (addError) {
          throw new Error('Failed to add Polygon network');
        }
      } else {
        throw switchError;
      }
    }
  }

  // Disconnect wallet
  disconnect() {
    this.provider = null;
    this.signer = null;
    this.address = null;
    this.isConnected = false;
    this.onDisconnected?.();
  }

  // Get current account
  getAccount() {
    return this.address;
  }

  // Get balance in ETH/MATIC
  async getNativeBalance() {
    if (!this.provider || !this.address) return '0';
    
    const balance = await this.provider.getBalance(this.address);
    return ethers.utils.formatEther(balance);
  }

  // Get ERC-20 token balance
  async getTokenBalance(tokenAddress) {
    if (!this.provider || !this.address) return '0';

    const tokenContract = new ethers.Contract(
      tokenAddress,
      [
        'function balanceOf(address owner) view returns (uint256)',
        'function decimals() view returns (uint8)',
        'function symbol() view returns (string)'
      ],
      this.provider
    );

    try {
      const balance = await tokenContract.balanceOf(this.address);
      const decimals = await tokenContract.decimals();
      return ethers.utils.formatUnits(balance, decimals);
    } catch (error) {
      console.error('Failed to get token balance:', error);
      return '0';
    }
  }

  // Create contract instance
  getContract(address, abi, needsSigner = false) {
    if (!this.provider) {
      throw new Error('No provider available');
    }

    return new ethers.Contract(
      address,
      abi,
      needsSigner ? this.signer : this.provider
    );
  }

  // Format token amounts
  parseTokenAmount(amount, decimals = 18) {
    return ethers.utils.parseUnits(amount.toString(), decimals);
  }

  formatTokenAmount(amount, decimals = 18, precision = 4) {
    const formatted = ethers.utils.formatUnits(amount, decimals);
    return parseFloat(formatted).toFixed(precision);
  }

  // Transaction helpers
  async waitForTransaction(txHash) {
    if (!this.provider) {
      throw new Error('No provider available');
    }
    
    return await this.provider.waitForTransaction(txHash);
  }

  async getTransactionReceipt(txHash) {
    if (!this.provider) {
      throw new Error('No provider available');
    }
    
    return await this.provider.getTransactionReceipt(txHash);
  }

  // Calculate file size in KB
  calculateFileSizeKB(bytes) {
    return Math.ceil(bytes / 1024);
  }

  // Calculate costs (1 CPH per KB)
  calculateUploadCost(fileSizeBytes) {
    return this.calculateFileSizeKB(fileSizeBytes);
  }

  calculateDownloadCost(fileSizeBytes) {
    return this.calculateFileSizeKB(fileSizeBytes);
  }

  // Format costs for display
  formatCost(costInCPH) {
    if (costInCPH >= 1000000) {
      return (costInCPH / 1000000).toFixed(2) + 'M CPH';
    } else if (costInCPH >= 1000) {
      return (costInCPH / 1000).toFixed(2) + 'K CPH';
    } else {
      return costInCPH.toString() + ' CPH';
    }
  }

  // Event handlers (can be overridden)
  onAccountChanged = null;
  onDisconnected = null;
  onNetworkChanged = null;
}

// Export for ES6 modules
export default Web3Utils;

// Also make available globally
window.Web3Utils = Web3Utils;