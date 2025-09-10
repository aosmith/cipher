class ImageDecryption {
  constructor() {
    this.init();
  }

  async init() {
    // Wait for DOM and crypto libraries to load
    if (document.readyState === 'loading') {
      document.addEventListener('DOMContentLoaded', () => this.setupDecryption());
    } else {
      this.setupDecryption();
    }
  }

  async setupDecryption() {
    const encryptedImages = document.querySelectorAll('.encrypted-image');
    
    for (const imageContainer of encryptedImages) {
      try {
        await this.decryptAndDisplayImage(imageContainer);
      } catch (error) {
        console.error('Failed to decrypt image:', error);
        this.showDecryptionError(imageContainer);
      }
    }
  }

  async decryptAndDisplayImage(container) {
    const attachmentUrl = container.dataset.attachmentUrl;
    const filename = container.dataset.filename;
    const filesize = container.dataset.filesize;

    try {
      // Fetch encrypted data from server
      const response = await fetch(attachmentUrl, {
        credentials: 'include'
      });
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const attachmentData = await response.json();
      
      if (!attachmentData.access_granted) {
        throw new Error('Access denied to attachment');
      }

      // For development: use the dev_owner_key for decryption
      // In production, this would use the user's private key to decrypt the attachment key
      const decryptedImageData = await this.decryptData(attachmentData.encrypted_data, attachmentData.dev_owner_key);
      
      if (decryptedImageData) {
        // Create image element and display
        this.displayDecryptedImage(container, decryptedImageData, filename, filesize, attachmentData.content_type);
      } else {
        throw new Error('Failed to decrypt image data');
      }

    } catch (error) {
      console.error('Error decrypting image:', error);
      this.showDecryptionError(container);
    }
  }

  async decryptData(encryptedDataBase64, devOwnerKey) {
    try {
      // Import TweetNaCl for client-side decryption (using skypack CDN)
      const naclModule = await import('tweetnacl');
      console.log('TweetNaCl import result:', naclModule);
      console.log('Available properties:', Object.keys(naclModule));
      
      // Skypack should provide proper default export
      const nacl = naclModule.default || naclModule;
      console.log('Final nacl object:', nacl);
      console.log('nacl.secretbox:', nacl.secretbox);
      
      if (!nacl || !nacl.secretbox) {
        throw new Error('TweetNaCl secretbox not found in import. Available: ' + Object.keys(nacl || {}));
      }
      
      const secretbox = nacl.secretbox;
      
      if (!devOwnerKey) {
        console.error('No development key provided');
        return null;
      }
      
      // Helper function to safely decode base64 to Uint8Array
      function base64ToUint8Array(base64String) {
        // Remove any whitespace and ensure proper padding
        const cleanBase64 = base64String.replace(/\s/g, '');
        const padding = cleanBase64.length % 4;
        const paddedBase64 = padding ? cleanBase64 + '='.repeat(4 - padding) : cleanBase64;
        
        const binaryString = atob(paddedBase64);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
          bytes[i] = binaryString.charCodeAt(i);
        }
        return bytes;
      }
      
      // Decode the key and encrypted data from base64
      const key = base64ToUint8Array(devOwnerKey);
      const encryptedData = base64ToUint8Array(encryptedDataBase64);
      
      // RbNaCl::SimpleBox concatenates nonce + ciphertext automatically
      // We need to manually split them for TweetNaCl
      // First 24 bytes are the nonce, rest is the encrypted content
      const nonce = encryptedData.slice(0, 24);
      const ciphertext = encryptedData.slice(24);
      
      // Decrypt the data using TweetNaCl
      const decrypted = secretbox.open(ciphertext, nonce, key);
      
      if (!decrypted) {
        console.error('Decryption failed - invalid key or corrupted data');
        return null;
      }
      
      console.log('Successfully decrypted image data');
      return decrypted;
      
    } catch (error) {
      console.error('Decryption failed:', error);
      return null;
    }
  }

  displayDecryptedImage(container, imageData, filename, filesize, contentType) {
    // Create blob and object URL
    const blob = new Blob([imageData], { type: contentType });
    const imageUrl = URL.createObjectURL(blob);

    // Create image element
    const img = document.createElement('img');
    img.src = imageUrl;
    img.alt = filename;
    img.className = 'attachment-image';
    
    // Create clickable link
    const link = document.createElement('a');
    link.href = imageUrl;
    link.target = '_blank';
    link.className = 'image-link';
    link.appendChild(img);

    // Replace loading placeholder
    const loadingPlaceholder = container.querySelector('.loading-placeholder');
    if (loadingPlaceholder) {
      container.removeChild(loadingPlaceholder);
    }

    container.appendChild(link);

    // Show overlay on hover
    const overlay = container.querySelector('.image-overlay');
    if (overlay) {
      overlay.style.display = 'block';
    }

    // Clean up object URL after image loads
    img.onload = () => {
      setTimeout(() => URL.revokeObjectURL(imageUrl), 1000);
    };
  }

  showDecryptionError(container) {
    const loadingPlaceholder = container.querySelector('.loading-placeholder');
    if (loadingPlaceholder) {
      loadingPlaceholder.innerHTML = `
        <span class="error-icon">❌</span>
        <span class="error-text">Failed to decrypt image</span>
        <small>Client-side decryption not fully implemented</small>
      `;
    }
  }
}

// Initialize when page loads
new ImageDecryption();

export default ImageDecryption;