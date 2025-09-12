/**
 * Group Encryption for One-to-Many File Sharing
 * Handles client-side encryption for sharing files with multiple users
 */

import { encryptMessage, decryptMessage, generateKeyPair } from 'crypto_utils';

class GroupEncryption {
  constructor() {
    this.currentUserKeys = null;
  }

  // Set current user's key pair (should be called when user logs in)
  setCurrentUserKeys(publicKey, privateKey) {
    this.currentUserKeys = { publicKey, privateKey };
  }

  /**
   * Encrypt file data for sharing with multiple users
   * @param {ArrayBuffer} fileData - The raw file data
   * @param {Array} recipientPublicKeys - Array of recipient public keys including sender
   * @returns {Object} - Contains encrypted data and per-user encrypted keys
   */
  async encryptForMultipleUsers(fileData, recipientPublicKeys = []) {
    if (!this.currentUserKeys) {
      throw new Error('Current user keys not set');
    }

    // Always include the current user as a recipient
    const allRecipients = [this.currentUserKeys.publicKey, ...recipientPublicKeys];
    const uniqueRecipients = [...new Set(allRecipients)];

    // Generate a random symmetric key for the file
    const symmetricKey = this.generateSymmetricKey();
    
    // Encrypt the file data with the symmetric key
    const encryptedFileData = await this.encryptWithSymmetricKey(fileData, symmetricKey);
    
    // Encrypt the symmetric key for each recipient
    const encryptedKeys = {};
    
    for (const recipientPublicKey of uniqueRecipients) {
      try {
        const encryptedKey = await encryptMessage(
          this.arrayBufferToBase64(symmetricKey),
          recipientPublicKey,
          this.currentUserKeys.privateKey
        );
        encryptedKeys[recipientPublicKey] = encryptedKey;
      } catch (error) {
        console.error(`Failed to encrypt key for recipient ${recipientPublicKey}:`, error);
      }
    }

    return {
      encryptedData: this.arrayBufferToBase64(encryptedFileData),
      encryptedKeys: encryptedKeys,
      recipients: uniqueRecipients
    };
  }

  /**
   * Decrypt file data using the current user's private key
   * @param {string} encryptedData - Base64 encoded encrypted file data
   * @param {string} encryptedKey - The encrypted symmetric key for current user
   * @param {string} senderPublicKey - Public key of the user who encrypted the file
   * @returns {ArrayBuffer} - Decrypted file data
   */
  async decryptFile(encryptedData, encryptedKey, senderPublicKey) {
    if (!this.currentUserKeys) {
      throw new Error('Current user keys not set');
    }

    try {
      // Decrypt the symmetric key using our private key
      const symmetricKeyBase64 = await decryptMessage(
        encryptedKey,
        senderPublicKey,
        this.currentUserKeys.privateKey
      );
      
      const symmetricKey = this.base64ToArrayBuffer(symmetricKeyBase64);
      
      // Decrypt the file data using the symmetric key
      const encryptedFileBuffer = this.base64ToArrayBuffer(encryptedData);
      const decryptedData = await this.decryptWithSymmetricKey(encryptedFileBuffer, symmetricKey);
      
      return decryptedData;
    } catch (error) {
      console.error('Failed to decrypt file:', error);
      throw new Error('Failed to decrypt file: Invalid key or corrupted data');
    }
  }

  /**
   * Share an existing file with additional users
   * @param {string} encryptedData - The encrypted file data
   * @param {string} currentUserEncryptedKey - Current user's encrypted key
   * @param {Array} newRecipientPublicKeys - New recipients to share with
   * @param {string} originalSenderPublicKey - Original sender's public key
   * @returns {Object} - New encrypted keys for the additional recipients
   */
  async shareWithAdditionalUsers(encryptedData, currentUserEncryptedKey, newRecipientPublicKeys, originalSenderPublicKey) {
    if (!this.currentUserKeys) {
      throw new Error('Current user keys not set');
    }

    try {
      // First decrypt the symmetric key using current user's key
      const symmetricKeyBase64 = await decryptMessage(
        currentUserEncryptedKey,
        originalSenderPublicKey,
        this.currentUserKeys.privateKey
      );
      
      // Now encrypt the same symmetric key for new recipients
      const newEncryptedKeys = {};
      
      for (const recipientPublicKey of newRecipientPublicKeys) {
        try {
          const encryptedKey = await encryptMessage(
            symmetricKeyBase64,
            recipientPublicKey,
            this.currentUserKeys.privateKey
          );
          newEncryptedKeys[recipientPublicKey] = encryptedKey;
        } catch (error) {
          console.error(`Failed to encrypt key for new recipient ${recipientPublicKey}:`, error);
        }
      }

      return newEncryptedKeys;
    } catch (error) {
      console.error('Failed to share with additional users:', error);
      throw error;
    }
  }

  // Utility methods for symmetric encryption using Web Crypto API
  generateSymmetricKey() {
    // Generate a 256-bit AES key
    const key = new Uint8Array(32);
    crypto.getRandomValues(key);
    return key.buffer;
  }

  async encryptWithSymmetricKey(data, key) {
    const cryptoKey = await crypto.subtle.importKey(
      'raw',
      key,
      { name: 'AES-GCM' },
      false,
      ['encrypt']
    );

    const iv = crypto.getRandomValues(new Uint8Array(12)); // 96-bit IV for GCM
    const encrypted = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: iv },
      cryptoKey,
      data
    );

    // Prepend IV to encrypted data
    const result = new Uint8Array(iv.length + encrypted.byteLength);
    result.set(iv, 0);
    result.set(new Uint8Array(encrypted), iv.length);
    
    return result.buffer;
  }

  async decryptWithSymmetricKey(encryptedData, key) {
    const cryptoKey = await crypto.subtle.importKey(
      'raw',
      key,
      { name: 'AES-GCM' },
      false,
      ['decrypt']
    );

    const data = new Uint8Array(encryptedData);
    const iv = data.slice(0, 12);
    const encrypted = data.slice(12);

    const decrypted = await crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: iv },
      cryptoKey,
      encrypted
    );

    return decrypted;
  }

  // Utility methods for data conversion
  arrayBufferToBase64(buffer) {
    const bytes = new Uint8Array(buffer);
    let binary = '';
    for (let i = 0; i < bytes.byteLength; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
  }

  base64ToArrayBuffer(base64) {
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return bytes.buffer;
  }

  // Get user info by public key (helper for UI)
  async getUserByPublicKey(publicKey) {
    try {
      const response = await fetch('/api/v1/users/by_public_key', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Requested-With': 'XMLHttpRequest'
        },
        body: JSON.stringify({ public_key: publicKey })
      });

      if (response.ok) {
        return await response.json();
      }
      return null;
    } catch (error) {
      console.error('Failed to get user by public key:', error);
      return null;
    }
  }
}

// Export for ES6 modules
export default GroupEncryption;

// Also make available globally
window.GroupEncryption = GroupEncryption;