class Message < ApplicationRecord
  belongs_to :sender, class_name: 'User'
  belongs_to :recipient, class_name: 'User'
  
  validates :content, presence: true, length: { maximum: 2000 }
  validates :sender_id, :recipient_id, presence: true
  validate :sender_cannot_be_recipient
  
  # Encrypt content before saving (local encryption for privacy)
  before_save :encrypt_content, if: :will_save_change_to_content?
  # Decrypt content after loading (for display)
  after_find :decrypt_content
  
  scope :unread, -> { where(read_at: nil) }
  scope :read, -> { where.not(read_at: nil) }
  scope :recent, -> { order(created_at: :desc) }
  scope :between_users, ->(user1, user2) { 
    where(
      "(sender_id = ? AND recipient_id = ?) OR (sender_id = ? AND recipient_id = ?)",
      user1.id, user2.id, user2.id, user1.id
    )
  }
  
  def mark_as_read!
    update!(read_at: Time.current) unless read?
  end
  
  def read?
    read_at.present?
  end
  
  def unread?
    !read?
  end
  
  def conversation_partner_for(user)
    user == sender ? recipient : sender
  end
  
  private
  
  def sender_cannot_be_recipient
    errors.add(:recipient, "can't be the same as sender") if sender_id == recipient_id
  end
  
  def encrypt_content
    if content.present? && content_changed?
      begin
        # Use NaCl for encryption (hybrid approach: NaCl box for message, signed by sender)
        
        # Get sender's private key and recipient's public key
        sender_private_key_bytes = Base64.decode64(sender.private_key) if sender.private_key
        recipient_public_key_bytes = Base64.decode64(recipient.public_key)
        
        return unless sender_private_key_bytes && recipient_public_key_bytes
        
        # Create NaCl box for encryption (sender private key + recipient public key)
        box = RbNaCl::Box.new(recipient_public_key_bytes, sender_private_key_bytes)
        
        # Encrypt the message
        nonce = RbNaCl::Random.random_bytes(box.nonce_bytes)
        ciphertext = box.encrypt(content, nonce)
        
        # Sign the original content for authenticity
        signing_key = RbNaCl::SigningKey.new(sender_private_key_bytes)
        signature = signing_key.sign(content)
        
        # Combine everything into JSON
        encrypted_message = {
          ciphertext: Base64.strict_encode64(ciphertext),
          nonce: Base64.strict_encode64(nonce),
          signature: Base64.strict_encode64(signature),
          algorithm: 'NaCl-Box'
        }
        
        self.content = encrypted_message.to_json
        
      rescue RbNaCl::CryptoError, ArgumentError => e
        Rails.logger.error "Message encryption failed: #{e.message}"
        # Leave content unencrypted if encryption fails
      end
    end
  end
  
  def decrypt_content
    return unless content.present?
    
    # Check if content is encrypted (JSON format)
    begin
      encrypted_data = JSON.parse(content)
      return unless encrypted_data.is_a?(Hash) && encrypted_data['algorithm'] == 'NaCl-Box'
    rescue JSON::ParserError
      return # Not encrypted or legacy format
    end
    
    begin
      # Get the current user for decryption (should be recipient)
      current_user = get_current_user_for_decryption
      return unless current_user&.private_key
      
      # Determine if current user is sender or recipient
      if current_user == sender
        # Current user is sender - use sender private key + recipient public key
        private_key_bytes = Base64.decode64(current_user.private_key)
        public_key_bytes = Base64.decode64(recipient.public_key)
      elsif current_user == recipient
        # Current user is recipient - use recipient private key + sender public key  
        private_key_bytes = Base64.decode64(current_user.private_key)
        public_key_bytes = Base64.decode64(sender.public_key)
      else
        return # Current user is neither sender nor recipient
      end
      
      # Create NaCl box for decryption
      box = RbNaCl::Box.new(public_key_bytes, private_key_bytes)
      
      # Decrypt the message
      ciphertext = Base64.strict_decode64(encrypted_data['ciphertext'])
      nonce = Base64.strict_decode64(encrypted_data['nonce'])
      decrypted_content = box.decrypt(ciphertext, nonce)
      
      # Verify signature
      if encrypted_data['signature']
        sender_public_key_bytes = Base64.decode64(sender.public_key)
        verify_key = RbNaCl::VerifyKey.new(sender_public_key_bytes)
        signature = Base64.strict_decode64(encrypted_data['signature'])
        
        begin
          verify_key.verify(signature, decrypted_content)
        rescue RbNaCl::BadSignatureError
          Rails.logger.warn "Invalid message signature for message #{id}"
          # Still return content but log the warning
        end
      end
      
      self.content = decrypted_content
      
    rescue RbNaCl::CryptoError, ArgumentError, Base64::DecodeError => e
      Rails.logger.error "Message decryption failed for message #{id}: #{e.message}"
      # Leave content as encrypted JSON if decryption fails
    end
  end
  
  def get_current_user_for_decryption
    # Use thread-local storage set by controller
    Thread.current[:current_user_for_decryption]
  end
end
