// User Registration with Key Generation
import CipherCrypto from 'crypto_utils';

class UserRegistration {
  constructor() {
    this.form = document.querySelector('.user-form');
    this.submitButton = document.getElementById('create-identity-btn');
    this.keyGenSection = document.getElementById('key-generation');
    this.keyProgress = document.getElementById('key-progress');
    this.keyStatus = document.getElementById('key-status');
    this.publicKeyField = document.getElementById('public-key-field');
    this.usernameField = document.getElementById('user_username');
    this.passwordField = document.getElementById('master-password');
    this.confirmPasswordField = document.getElementById('confirm-password');
    
    this.crypto = new CipherCrypto();
    this.keysGenerated = false;
    
    this.init();
  }

  init() {
    if (!this.form) return; // Exit if not on registration page
    
    this.setupEventListeners();
  }

  setupEventListeners() {
    this.passwordField.addEventListener('input', () => this.validatePasswords());
    this.confirmPasswordField.addEventListener('input', () => this.validatePasswords());
    
    this.form.addEventListener('submit', async (event) => {
      await this.handleFormSubmission(event);
    });
  }

  validatePasswords() {
    const password = this.passwordField.value;
    const confirmPassword = this.confirmPasswordField.value;
    
    if (password !== confirmPassword && confirmPassword.length > 0) {
      this.confirmPasswordField.setCustomValidity('Passwords do not match');
    } else {
      this.confirmPasswordField.setCustomValidity('');
    }
  }

  async generateKeys(username, password) {
    try {
      this.keyGenSection.style.display = 'block';
      this.submitButton.disabled = true;
      
      // Step 1: Derive private key
      this.keyStatus.textContent = 'Deriving private key from password...';
      this.keyProgress.style.width = '25%';
      
      await new Promise(resolve => setTimeout(resolve, 500)); // Visual delay
      
      const keyPair = await this.crypto.createKeyPair(username, password);
      
      // Step 2: Generate public key
      this.keyStatus.textContent = 'Generating public key...';
      this.keyProgress.style.width = '50%';
      
      await new Promise(resolve => setTimeout(resolve, 300));
      
      // Step 3: Store keys locally
      this.keyStatus.textContent = 'Storing keys securely in browser...';
      this.keyProgress.style.width = '75%';
      
      await this.crypto.storePrivateKey(username, keyPair.privateKey, password);
      
      await new Promise(resolve => setTimeout(resolve, 300));
      
      // Step 4: Complete
      this.keyStatus.textContent = 'Keys generated successfully!';
      this.keyProgress.style.width = '100%';
      
      // Set the public key in the hidden field
      this.publicKeyField.value = keyPair.publicKeyBase64;
      this.keysGenerated = true;
      
      console.log('Public key generated:', keyPair.publicKeyBase64);
      console.log('Public key field value:', this.publicKeyField.value);
      
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Show the private key to the user before proceeding
      this.showPrivateKey(keyPair.privateKeyBase64);
      
      // Clear password fields to ensure they're never sent to server
      this.passwordField.value = '';
      this.confirmPasswordField.value = '';
      
    } catch (error) {
      console.error('Key generation failed:', error);
      this.keyStatus.textContent = 'Key generation failed. Please try again.';
      this.submitButton.disabled = false;
      
      setTimeout(() => {
        this.keyGenSection.style.display = 'none';
      }, 3000);
    }
  }

  showPrivateKey(privateKeyBase64) {
    // Hide the key generation section
    this.keyGenSection.style.display = 'none';
    
    // Create and show private key display modal
    const modal = document.createElement('div');
    modal.className = 'private-key-modal';
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
              <textarea readonly id="private-key-textarea" class="private-key-textarea">${privateKeyBase64}</textarea>
              <button onclick="copyPrivateKey()" class="btn-copy" title="Copy to clipboard">📋</button>
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
    `;
    
    // Add modal to page
    document.body.appendChild(modal);
    
    // Setup modal interactions
    this.setupPrivateKeyModal(modal);
  }

  setupPrivateKeyModal(modal) {
    const checkbox = modal.querySelector('#private-key-saved');
    const continueButton = modal.querySelector('#continue-signup');
    const privateKeyTextarea = modal.querySelector('#private-key-textarea');
    
    // Enable continue button when checkbox is checked
    checkbox.addEventListener('change', () => {
      continueButton.disabled = !checkbox.checked;
    });
    
    // Handle continue button
    continueButton.addEventListener('click', () => {
      // Remove modal
      modal.remove();
      
      // Submit the form
      this.submitButton.disabled = false;
      setTimeout(() => {
        this.form.submit();
      }, 100);
    });
    
    // Global function to copy private key
    window.copyPrivateKey = () => {
      privateKeyTextarea.select();
      navigator.clipboard.writeText(privateKeyTextarea.value).then(() => {
        const copyButton = modal.querySelector('.btn-copy');
        const original = copyButton.textContent;
        copyButton.textContent = '✓';
        setTimeout(() => {
          copyButton.textContent = original;
        }, 1000);
      });
    };
  }

  async handleFormSubmission(event) {
    // Allow normal submission if keys are already generated
    if (this.keysGenerated) {
      return; // Let form submit normally
    }
    
    // Otherwise, prevent submission and generate keys first
    event.preventDefault();
    
    const username = this.usernameField.value.trim();
    const password = this.passwordField.value;
    const confirmPassword = this.confirmPasswordField.value;
    
    // Validation
    if (!username || !password || !confirmPassword) {
      alert('Please fill in all required fields');
      return;
    }
    
    if (password !== confirmPassword) {
      alert('Passwords do not match');
      return;
    }
    
    if (password.length < 8) {
      alert('Password must be at least 8 characters long');
      return;
    }
    
    // Generate keys and then submit
    await this.generateKeys(username, password);
  }
}

// Initialize when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  new UserRegistration();
});

export default UserRegistration;