import { Controller } from "@hotwired/stimulus"

// Connects to data-controller="image-decryption"
export default class extends Controller {
  static values = { 
    attachmentUrl: String,
    filename: String,
    filesize: String,
    contentType: String
  }
  
  connect() {
    this.decryptAndDisplayImage()
  }
  
  async decryptAndDisplayImage() {
    try {
      // Fetch encrypted data from server
      const response = await fetch(this.attachmentUrlValue, {
        credentials: 'include'
      })
      
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`)
      }

      const attachmentData = await response.json()
      
      if (!attachmentData.access_granted) {
        throw new Error('Access denied to attachment')
      }

      // Decrypt the image data
      const decryptedImageData = await this.decryptData(attachmentData.encrypted_data, attachmentData.dev_owner_key)
      
      if (decryptedImageData) {
        this.displayDecryptedImage(decryptedImageData)
      } else {
        throw new Error('Failed to decrypt image data')
      }

    } catch (error) {
      console.error('Error decrypting image:', error)
      this.showDecryptionError()
    }
  }
  
  async decryptData(encryptedDataBase64, devOwnerKey) {
    try {
      // Import TweetNaCl for client-side decryption
      const naclModule = await import('tweetnacl')
      const nacl = naclModule.default || naclModule
      
      if (!nacl || !nacl.secretbox) {
        throw new Error('TweetNaCl secretbox not available')
      }
      
      if (!devOwnerKey) {
        throw new Error('No development key provided')
      }
      
      // Decode base64 to Uint8Array
      const key = this.base64ToUint8Array(devOwnerKey)
      const encryptedData = this.base64ToUint8Array(encryptedDataBase64)
      
      // Split nonce and ciphertext (first 24 bytes are nonce)
      const nonce = encryptedData.slice(0, 24)
      const ciphertext = encryptedData.slice(24)
      
      // Decrypt using TweetNaCl
      const decrypted = nacl.secretbox.open(ciphertext, nonce, key)
      
      if (!decrypted) {
        throw new Error('Decryption failed - invalid key or corrupted data')
      }
      
      return decrypted
      
    } catch (error) {
      console.error('Decryption failed:', error)
      return null
    }
  }
  
  base64ToUint8Array(base64String) {
    const cleanBase64 = base64String.replace(/\s/g, '')
    const padding = cleanBase64.length % 4
    const paddedBase64 = padding ? cleanBase64 + '='.repeat(4 - padding) : cleanBase64
    
    const binaryString = atob(paddedBase64)
    const bytes = new Uint8Array(binaryString.length)
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i)
    }
    return bytes
  }
  
  displayDecryptedImage(imageData) {
    // Create blob and object URL
    const blob = new Blob([imageData], { type: this.contentTypeValue })
    const imageUrl = URL.createObjectURL(blob)

    // Create image element
    const img = document.createElement('img')
    img.src = imageUrl
    img.alt = this.filenameValue
    img.className = 'attachment-image'
    
    // Create clickable link
    const link = document.createElement('a')
    link.href = imageUrl
    link.target = '_blank'
    link.className = 'image-link'
    link.appendChild(img)

    // Replace loading placeholder
    const loadingPlaceholder = this.element.querySelector('.loading-placeholder')
    if (loadingPlaceholder) {
      this.element.removeChild(loadingPlaceholder)
    }

    this.element.appendChild(link)

    // Show overlay if present
    const overlay = this.element.querySelector('.image-overlay')
    if (overlay) {
      overlay.style.display = 'block'
    }

    // Clean up object URL after image loads
    img.onload = () => {
      setTimeout(() => URL.revokeObjectURL(imageUrl), 1000)
    }
  }
  
  showDecryptionError() {
    const loadingPlaceholder = this.element.querySelector('.loading-placeholder')
    if (loadingPlaceholder) {
      loadingPlaceholder.innerHTML = `
        <span class="error-icon">❌</span>
        <span class="error-text">Failed to decrypt image</span>
        <small>Unable to decrypt attachment</small>
      `
    }
  }
}