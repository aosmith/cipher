/**
 * Client-side cryptographic utilities for Cipher
 * Uses Web Crypto API for key derivation and TweetNaCl for encryption
 */

import nacl from 'tweetnacl';
import naclUtil from 'tweetnacl-util';

class CipherCrypto {
  constructor() {
    // Check if we're in a secure context (required for Web Crypto API)
    if (!window.isSecureContext) {
      throw new Error('Cipher requires a secure context (HTTPS) for cryptographic operations');
    }
  }

  /**
   * Derive a private key from username and password using PBKDF2
   * @param {string} username - User's username (used as additional salt)
   * @param {string} password - User's password
   * @returns {Promise<Uint8Array>} - 32-byte private key
   */
  async derivePrivateKey(username, password) {
    const encoder = new TextEncoder();
    
    // Create a deterministic salt from username
    const usernameSalt = encoder.encode(username);
    
    // Add a fixed application salt to prevent rainbow tables across different apps
    const appSalt = encoder.encode('Cipher-P2P-Social-Network-v1');
    
    // Combine salts
    const combinedSalt = new Uint8Array(usernameSalt.length + appSalt.length);
    combinedSalt.set(usernameSalt);
    combinedSalt.set(appSalt, usernameSalt.length);
    
    // Import the password as a key
    const passwordKey = await crypto.subtle.importKey(
      'raw',
      encoder.encode(password),
      'PBKDF2',
      false,
      ['deriveBits']
    );
    
    // Derive 32 bytes using PBKDF2 with 100,000 iterations
    const derivedBits = await crypto.subtle.deriveBits(
      {
        name: 'PBKDF2',
        salt: combinedSalt,
        iterations: 100000,
        hash: 'SHA-256'
      },
      passwordKey,
      256 // 32 bytes * 8 bits
    );
    
    return new Uint8Array(derivedBits);
  }

  /**
   * Generate public key from private key using NaCl
   * @param {Uint8Array} privateKeyBytes - 32-byte private key
   * @returns {Uint8Array} - 32-byte public key
   */
  getPublicKey(privateKeyBytes) {
    // Use TweetNaCl to generate public key from private key
    const keyPair = nacl.box.keyPair.fromSecretKey(privateKeyBytes);
    return keyPair.publicKey;
  }

  /**
   * Create a complete key pair from username and password
   * @param {string} username - User's username
   * @param {string} password - User's password
   * @returns {Promise<Object>} - {privateKey, publicKey, publicKeyBase64}
   */
  async createKeyPair(username, password) {
    const privateKey = await this.derivePrivateKey(username, password);
    const publicKey = this.getPublicKey(privateKey);
    
    return {
      privateKey: privateKey,
      publicKey: publicKey,
      publicKeyBase64: this.arrayToBase64(publicKey)
    };
  }

  /**
   * Encrypt a message for a recipient
   * @param {string} message - Message to encrypt
   * @param {Uint8Array} senderPrivateKey - Sender's private key
   * @param {Uint8Array} recipientPublicKey - Recipient's public key
   * @returns {string} - Base64 encoded encrypted message
   */
  encryptMessage(message, senderPrivateKey, recipientPublicKey) {
    const messageBytes = naclUtil.decodeUTF8(message);
    const nonce = this.generateNonce();
    const encrypted = nacl.box(messageBytes, nonce, recipientPublicKey, senderPrivateKey);
    
    // Prepend nonce to encrypted message
    const combined = new Uint8Array(nonce.length + encrypted.length);
    combined.set(nonce);
    combined.set(encrypted, nonce.length);
    
    return this.arrayToBase64(combined);
  }

  /**
   * Decrypt a message from a sender
   * @param {string} encryptedMessage - Base64 encoded encrypted message with nonce
   * @param {Uint8Array} recipientPrivateKey - Recipient's private key
   * @param {Uint8Array} senderPublicKey - Sender's public key
   * @returns {string} - Decrypted message
   */
  decryptMessage(encryptedMessage, recipientPrivateKey, senderPublicKey) {
    const combined = this.base64ToArray(encryptedMessage);
    
    // Extract nonce (first 24 bytes) and encrypted content
    const nonce = combined.slice(0, 24);
    const encrypted = combined.slice(24);
    
    const decrypted = nacl.box.open(encrypted, nonce, senderPublicKey, recipientPrivateKey);
    
    if (!decrypted) {
      throw new Error('Failed to decrypt message');
    }
    
    return naclUtil.encodeUTF8(decrypted);
  }

  /**
   * Sign a message
   * @param {string} message - Message to sign
   * @param {Uint8Array} privateKey - Signer's private key
   * @returns {string} - Base64 encoded signature
   */
  signMessage(message, privateKey) {
    const messageBytes = naclUtil.decodeUTF8(message);
    const signingKey = nacl.sign.keyPair.fromSeed(privateKey.slice(0, 32));
    const signature = nacl.sign.detached(messageBytes, signingKey.secretKey);
    return this.arrayToBase64(signature);
  }

  /**
   * Verify a message signature
   * @param {string} message - Original message
   * @param {string} signature - Base64 encoded signature
   * @param {Uint8Array} publicKey - Signer's public key
   * @returns {boolean} - True if signature is valid
   */
  verifySignature(message, signature, publicKey) {
    try {
      const messageBytes = naclUtil.decodeUTF8(message);
      const signatureBytes = this.base64ToArray(signature);
      const signingKey = nacl.sign.keyPair.fromSeed(publicKey.slice(0, 32));
      
      return nacl.sign.detached.verify(messageBytes, signatureBytes, signingKey.publicKey);
    } catch (error) {
      return false;
    }
  }

  /**
   * Store encrypted private key in browser localStorage
   * @param {string} username - User's username
   * @param {Uint8Array} privateKey - User's private key
   * @param {string} password - User's password for encryption
   */
  async storePrivateKey(username, privateKey, password) {
    // Create a strong key for local storage encryption
    const storageKey = await this.deriveStorageKey(password);
    
    // Encrypt the private key for local storage
    const nonce = nacl.randomBytes(24);
    const encrypted = nacl.secretbox(privateKey, nonce, storageKey);
    
    // Store encrypted key with nonce
    const storageData = {
      encrypted: this.arrayToBase64(encrypted),
      nonce: this.arrayToBase64(nonce),
      username: username,
      timestamp: Date.now()
    };
    
    localStorage.setItem(`cipher_key_${username}`, JSON.stringify(storageData));
  }

  /**
   * Retrieve and decrypt private key from localStorage
   * @param {string} username - User's username
   * @param {string} password - User's password
   * @returns {Promise<Uint8Array|null>} - Private key or null if not found/invalid
   */
  async retrievePrivateKey(username, password) {
    try {
      const storageData = JSON.parse(localStorage.getItem(`cipher_key_${username}`));
      if (!storageData) return null;
      
      const storageKey = await this.deriveStorageKey(password);
      const encrypted = this.base64ToArray(storageData.encrypted);
      const nonce = this.base64ToArray(storageData.nonce);
      
      const decrypted = nacl.secretbox.open(encrypted, nonce, storageKey);
      return decrypted;
    } catch (error) {
      console.warn('Failed to retrieve private key from storage:', error);
      return null;
    }
  }

  /**
   * Export user identity as a backup phrase (deterministic from password)
   * @param {string} username - User's username
   * @param {string} password - User's password
   * @returns {Promise<string>} - Backup phrase (username + password hash)
   */
  async createBackupPhrase(username, password) {
    // Create a verifiable backup phrase that doesn't expose the password
    const encoder = new TextEncoder();
    const data = encoder.encode(`${username}:${password}`);
    
    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = new Uint8Array(hashBuffer);
    const hashBase64 = this.arrayToBase64(hashArray);
    
    return `cipher:${username}:${hashBase64}`;
  }

  /**
   * Verify a backup phrase without revealing the password
   * @param {string} backupPhrase - The backup phrase
   * @param {string} username - Username to verify
   * @param {string} password - Password to verify
   * @returns {Promise<boolean>} - True if phrase matches
   */
  async verifyBackupPhrase(backupPhrase, username, password) {
    const expectedPhrase = await this.createBackupPhrase(username, password);
    return backupPhrase === expectedPhrase;
  }

  // Utility methods
  generateNonce() {
    return nacl.randomBytes(24);
  }

  async deriveStorageKey(password) {
    const encoder = new TextEncoder();
    const passwordKey = await crypto.subtle.importKey(
      'raw',
      encoder.encode(password),
      'PBKDF2',
      false,
      ['deriveBits']
    );
    
    const salt = encoder.encode('cipher-storage-salt-v1');
    const derivedBits = await crypto.subtle.deriveBits(
      {
        name: 'PBKDF2',
        salt: salt,
        iterations: 50000,
        hash: 'SHA-256'
      },
      passwordKey,
      256
    );
    
    return new Uint8Array(derivedBits);
  }

  arrayToBase64(array) {
    return btoa(String.fromCharCode.apply(null, array));
  }

  base64ToArray(base64) {
    return new Uint8Array(atob(base64).split('').map(c => c.charCodeAt(0)));
  }
}

// Create a global instance for convenience
const cipherCrypto = new CipherCrypto();

// Export for ES6 modules
export default CipherCrypto;

// Named exports for individual functions (used by other modules)
export const encryptMessage = (message, senderPrivateKey, recipientPublicKey) => 
  cipherCrypto.encryptMessage(message, senderPrivateKey, recipientPublicKey);

export const decryptMessage = (encryptedMessage, recipientPrivateKey, senderPublicKey) => 
  cipherCrypto.decryptMessage(encryptedMessage, recipientPrivateKey, senderPublicKey);

export const generateKeyPair = (username, password) => 
  cipherCrypto.createKeyPair(username, password);

export const signMessage = (message, privateKey) => 
  cipherCrypto.signMessage(message, privateKey);

export const verifySignature = (message, signature, publicKey) => 
  cipherCrypto.verifySignature(message, signature, publicKey);

// Also make available globally for inline scripts
window.CipherCrypto = CipherCrypto;