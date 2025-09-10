import { Controller } from "@hotwired/stimulus"
import CipherCrypto from "crypto_utils"

// Connects to data-controller="user-registration"
export default class extends Controller {
  static targets = ["username", "password", "confirmPassword", "publicKey", "submitButton", "entropyProgress", "entropyStatus", "entropyProgressBar"]
  static values = { entropyRequired: { type: Number, default: 100 } }
  
  connect() {
    this.crypto = new CipherCrypto()
    this.keysGenerated = false
    this.entropyPool = []
    this.entropyCollected = 0
    
    this.setupEntropyCollection()
    this.showEntropySection()
    this.updateSubmitState()
  }
  
  showEntropySection() {
    if (this.hasEntropyProgressTarget) {
      this.entropyProgressTarget.style.display = 'block'
    }
  }
  
  setupEntropyCollection() {
    // Collect entropy from mouse movements and key presses
    document.addEventListener('mousemove', this.addEntropy.bind(this))
    document.addEventListener('keydown', this.addEntropy.bind(this))
    document.addEventListener('touchmove', this.addEntropy.bind(this))
  }
  
  addEntropy(event) {
    if (this.entropyCollected >= this.entropyRequiredValue) return
    
    // Add event data to entropy pool
    this.entropyPool.push(
      event.clientX || 0, 
      event.clientY || 0, 
      event.timeStamp || Date.now(),
      Date.now(), 
      performance.now()
    )
    
    // Simple entropy estimation
    this.entropyCollected = Math.min(this.entropyPool.length / 2, this.entropyRequiredValue)
    this.updateEntropyDisplay()
  }
  
  updateEntropyDisplay() {
    const progress = (this.entropyCollected / this.entropyRequiredValue) * 100
    
    if (this.hasEntropyProgressBarTarget) {
      this.entropyProgressBarTarget.style.width = `${progress}%`
    }
    
    if (this.hasEntropyStatusTarget) {
      if (progress >= 100) {
        this.entropyStatusTarget.textContent = '🔐 Ready for secure key generation!'
        this.entropyStatusTarget.className = 'entropy-status entropy-ready'
      } else {
        this.entropyStatusTarget.textContent = `🎲 Collecting randomness... ${Math.round(progress)}%`
        this.entropyStatusTarget.className = 'entropy-status entropy-collecting'
      }
    }
    
    this.updateSubmitState()
  }
  
  updateSubmitState() {
    const hasEntropy = this.entropyCollected >= this.entropyRequiredValue
    const hasValidInputs = this.usernameTarget.value.trim() && 
                          this.passwordTarget.value.length >= 8 && 
                          this.passwordTarget.value === this.confirmPasswordTarget.value
    
    this.submitButtonTarget.disabled = !hasEntropy || !hasValidInputs || this.keysGenerated
  }
  
  // Stimulus action methods
  validateInput() {
    this.updateSubmitState()
  }
  
  async generateKeys(event) {
    if (this.keysGenerated) return
    
    event.preventDefault()
    
    // Validate inputs first
    const username = this.usernameTarget.value.trim()
    const password = this.passwordTarget.value
    const confirmPassword = this.confirmPasswordTarget.value
    
    if (!username || !password || !confirmPassword) {
      this.showError('Please fill in all required fields')
      return
    }
    
    if (password !== confirmPassword) {
      this.showError('Passwords do not match')
      return
    }
    
    if (password.length < 8) {
      this.showError('Password must be at least 8 characters long')
      return
    }
    
    if (!/^[a-zA-Z0-9_]{3,20}$/.test(username)) {
      this.showError('Username must be 3-20 characters using only letters, numbers, and underscores')
      return
    }
    
    if (this.entropyCollected < this.entropyRequiredValue) {
      this.showError('Please move your mouse or type to collect sufficient randomness for secure key generation')
      return
    }
    
    try {
      this.submitButtonTarget.disabled = true
      this.submitButtonTarget.textContent = 'Generating Keys...'
      
      // Generate the key pair
      const keyPair = await this.crypto.createKeyPair(username, password)
      
      // Set the public key in the hidden field
      this.publicKeyTarget.value = keyPair.publicKeyBase64
      this.keysGenerated = true
      
      // Show private key modal and proceed
      this.showPrivateKeyModal(keyPair.privateKeyBase64)
      
    } catch (error) {
      console.error('Key generation failed:', error)
      this.showError('Key generation failed. Please try again.')
      this.submitButtonTarget.disabled = false
      this.submitButtonTarget.textContent = '🚀 Create Zero-Knowledge Identity'
    }
  }
  
  showError(message) {
    // Create or update error message display
    let errorDiv = document.getElementById('registration-error')
    if (!errorDiv) {
      errorDiv = document.createElement('div')
      errorDiv.id = 'registration-error'
      errorDiv.className = 'error-message'
      this.element.insertBefore(errorDiv, this.element.firstChild)
    }
    
    errorDiv.innerHTML = `
      <div class="alert alert-error">
        <strong>⚠️ Error:</strong> ${message}
      </div>
    `
    
    // Scroll to error
    errorDiv.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }
  
  clearError() {
    const errorDiv = document.getElementById('registration-error')
    if (errorDiv) {
      errorDiv.remove()
    }
  }
  
  showPrivateKeyModal(privateKey) {
    // Simple modal for showing private key
    const modal = document.createElement('div')
    modal.className = 'private-key-modal'
    modal.innerHTML = `
      <div class="private-key-content">
        <div class="private-key-header">
          <h2>🔐 Your Private Key</h2>
          <p><strong>CRITICAL:</strong> Save this private key securely! This is your only chance to see it.</p>
        </div>
        
        <div class="private-key-display">
          <div class="key-section">
            <label>Private Key (Base64):</label>
            <div class="key-value">
              <textarea readonly id="private-key-textarea" class="private-key-textarea">${privateKey}</textarea>
              <button onclick="this.copyPrivateKey()" class="btn-copy" title="Copy to clipboard">📋</button>
            </div>
          </div>
        </div>
        
        <div class="private-key-warning">
          <h3>⚠️ IMPORTANT SECURITY NOTES:</h3>
          <ul>
            <li><strong>Keep this private key safe</strong> - it's the only way to access your account</li>
            <li><strong>Never share it</strong> with anyone - it gives full access to your encrypted data</li>
            <li><strong>Store it securely</strong> - consider using a password manager</li>
            <li><strong>We cannot recover it</strong> - if lost, your account is permanently inaccessible</li>
          </ul>
        </div>
        
        <div class="private-key-actions">
          <div class="checkbox-container">
            <input type="checkbox" id="private-key-saved" class="private-key-checkbox">
            <label for="private-key-saved">I have securely saved my private key</label>
          </div>
          <button id="continue-signup" class="btn btn-primary btn-large" disabled>Continue to Account</button>
        </div>
      </div>
    `
    
    document.body.appendChild(modal)
    
    // Setup modal interactions
    this.setupPrivateKeyModal(modal, privateKey)
  }
  
  setupPrivateKeyModal(modal, privateKey) {
    const checkbox = modal.querySelector('#private-key-saved')
    const continueButton = modal.querySelector('#continue-signup')
    const privateKeyTextarea = modal.querySelector('#private-key-textarea')
    const copyButton = modal.querySelector('.btn-copy')
    
    // Enable continue button when checkbox is checked
    checkbox.addEventListener('change', () => {
      continueButton.disabled = !checkbox.checked
    })
    
    // Handle continue button
    continueButton.addEventListener('click', () => {
      // Remove modal
      modal.remove()
      
      // Submit the form
      this.element.submit()
    })
    
    // Handle copy button
    copyButton.addEventListener('click', () => {
      privateKeyTextarea.select()
      navigator.clipboard.writeText(privateKey).then(() => {
        const original = copyButton.textContent
        copyButton.textContent = '✓'
        setTimeout(() => {
          copyButton.textContent = original
        }, 1000)
      }).catch(() => {
        // Fallback for older browsers
        privateKeyTextarea.select()
        document.execCommand('copy')
      })
    })
  }
}