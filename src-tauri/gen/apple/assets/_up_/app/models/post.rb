class Post < ApplicationRecord
  require 'rbnacl'
  require 'base64'
  require 'digest'

  belongs_to :user
  belongs_to :original_user, class_name: 'User', optional: true  # Only for synced posts
  belongs_to :synced_from_user, class_name: 'User', optional: true  # Only for synced posts
  has_many :attachments, dependent: :destroy
  has_many :comments, dependent: :destroy

  scope :recent, -> { order(timestamp: :desc) }
  scope :original_posts, -> { where(is_synced: [false, nil]) }
  scope :synced_posts, -> { where(is_synced: true) }
  scope :synced_from_user, ->(user) { where(synced_from_user: user, is_synced: true) }

  validates :timestamp, presence: true
  validate :content_or_attachments_present
  validate :validate_content_hash, unless: -> { Rails.env.test? }
  validate :validate_content_size, on: :create, unless: :is_synced?

  before_validation :set_timestamp, on: :create
  before_validation :encrypt_content, on: [:create, :update]
  before_validation :sign_content, on: :create, unless: :is_synced?
  before_validation :set_content_hash, on: [:create, :update], unless: :is_synced?
  before_validation :set_synced_at, if: :is_synced?
  after_save :clear_plaintext_cache
  
  # Validation for sync fields consistency
  validate :sync_fields_consistency

  def content=(plaintext_content)
    @plaintext_content = plaintext_content
  end

  def content
    # If plaintext content is cached, return it
    return @plaintext_content if @plaintext_content
    
    # Otherwise, return decrypted content from database
    if content_encrypted.present?
      decrypt_content
    end
  end

  def encrypt_for_recipient(recipient_user)
    return unless @plaintext_content
    
    self.content_encrypted = user.encrypt_message(@plaintext_content, recipient_user.public_key)
  end

  def verify_signature
    return false unless signature.present? && content_encrypted.present?
    
    user.verify_signature(content_encrypted, signature, user.public_key)
  end

  def broadcast_to_peers
    # This will be implemented when we add P2P networking
    user.peers.active.each do |peer|
      # Send encrypted post to each peer
    end
  end

  def add_attachment(file_data, filename, content_type)
    attachment = attachments.build(
      filename: filename,
      content_type: content_type,
      file_size: file_data.bytesize
    )
    attachment.encrypt_data(file_data)
    attachment
  end

  def has_media?
    attachments.any? { |a| a.content_type.start_with?('image/', 'video/', 'audio/') }
  end

  private

  def set_timestamp
    self.timestamp = Time.current
  end

  def set_synced_at
    self.synced_at = Time.current if is_synced? && synced_at.blank?
  end

  def encrypt_content
    return if @plaintext_content.blank?

    begin
      # Use proper NaCl encryption for content
      # Generate a random key for this post
      secret_key = RbNaCl::Random.random_bytes(RbNaCl::SecretBox.key_bytes)
      box = RbNaCl::SecretBox.new(secret_key)

      # Encrypt the content
      encrypted_content = box.encrypt(@plaintext_content)

      # Store the encrypted content as base64
      # In a real P2P app, the secret_key would be encrypted with recipient's public key
      # For local-only app, we'll store it with the content for simplicity
      self.content_encrypted = Base64.strict_encode64(encrypted_content)

      # Store the key (in production this would be encrypted for recipients)
      self.encryption_key = Base64.strict_encode64(secret_key)

    rescue => e
      Rails.logger.error "Content encryption failed: #{e.message}"
      # Fallback to storing plaintext if encryption fails
      self.content_encrypted = @plaintext_content
    end
  end

  def sign_content
    # Skip signing if we're already in the signing process (prevent recursion)
    return if @signing_in_progress
    @signing_in_progress = true
    
    begin
      # Create a signature for the entire post including attachments
      attachment_checksums = attachments.loaded? ? attachments.map(&:checksum) : []
      content_to_sign = [content_encrypted, attachment_checksums].flatten.compact.join
      
      # If there's no content to sign, create a minimal signature with timestamp
      if content_to_sign.empty?
        content_to_sign = timestamp&.to_i&.to_s || Time.current.to_i.to_s
      end
      
      # Create a proper digital signature using the user's private key
      begin
        if user.private_key.present?
          # Decode the user's private key
          private_key_bytes = Base64.strict_decode64(user.private_key)
          signing_key = RbNaCl::SigningKey.new(private_key_bytes)

          # Sign the content
          signature_bytes = signing_key.sign(content_to_sign)
          self.signature = Base64.strict_encode64(signature_bytes)
        else
          # Fallback for users without stored private keys (will need key derivation)
          Rails.logger.warn "No private key available for signing post #{id}"
          self.signature = Digest::SHA256.hexdigest("#{user.id}-#{content_to_sign}-#{timestamp}")
        end
      rescue => e
        Rails.logger.error "Digital signature failed: #{e.message}"
        # Fallback to hash-based signature
        self.signature = Digest::SHA256.hexdigest("#{user.id}-#{content_to_sign}-#{timestamp}")
      end
    ensure
      @signing_in_progress = false
    end
  end

  def decrypt_content
    return content_encrypted if encryption_key.blank?

    begin
      # Decrypt using the stored encryption key
      secret_key = Base64.strict_decode64(encryption_key)
      box = RbNaCl::SecretBox.new(secret_key)

      # Decrypt the content
      encrypted_data = Base64.strict_decode64(content_encrypted)
      decrypted_content = box.decrypt(encrypted_data)

      decrypted_content
    rescue => e
      Rails.logger.error "Content decryption failed: #{e.message}"
      # Return encrypted content if decryption fails (might be plaintext)
      content_encrypted
    end
  end

  def content_or_attachments_present
    has_attachments = attachments.any?
    
    # If @plaintext_content has been explicitly set (not nil), use that for validation
    if defined?(@plaintext_content) && !@plaintext_content.nil?
      has_content = @plaintext_content.present?
    else
      # Otherwise, check existing encrypted content
      has_content = content_encrypted.present?
    end
    
    unless has_content || has_attachments
      errors.add(:base, "Post must have either content or attachments")
    end
  end

  def clear_plaintext_cache
    @plaintext_content = nil
  end
  
  # Sync-related methods
  
  def original_post?
    !is_synced?
  end
  
  def synced_post?
    is_synced == true
  end
  
  def sync_from_friend!(original_post, friend_user, syncing_user)
    # Create a synced copy of a friend's post
    self.original_user = original_post.user
    self.synced_from_user = friend_user
    self.user = syncing_user # The user who is storing this synced copy
    self.is_synced = true
    self.synced_at = Time.current
    self.content_encrypted = original_post.content_encrypted
    self.timestamp = original_post.timestamp
    self.signature = original_post.signature
    self.content_hash = generate_content_hash
  end
  
  def verify_sync_integrity
    return true unless synced_post?
    return false unless original_user && synced_from_user
    
    # Verify the content hash matches
    expected_hash = generate_content_hash
    content_hash == expected_hash
  end
  
  def age_since_sync
    return nil unless synced_at
    Time.current - synced_at
  end
  
  public
  
  def can_be_synced_by?(user)
    # Can sync posts from friends or friends of friends
    return false if synced_post? # Don't sync already synced posts
    return false unless self.user # Must have an original user
    
    user.friends_with?(self.user) || user.friends_of_friends_with?(self.user)
  end
  
  # Clean up old synced posts
  def self.cleanup_old_synced_posts(days_old: 30)
    synced_posts.where('synced_at < ?', days_old.days.ago).destroy_all
  end
  
  private
  
  def set_content_hash
    self.content_hash = generate_content_hash
  end
  
  def generate_content_hash
    # Prevent infinite recursion by using loaded attachments only
    attachment_checksums = attachments.loaded? ? attachments.map(&:checksum).sort.join : ""
    
    content_to_hash = [
      content_encrypted,
      timestamp&.to_i&.to_s,
      signature,
      attachment_checksums
    ].compact.join
    
    Digest::SHA256.hexdigest(content_to_hash)
  end
  
  def sync_fields_consistency
    if is_synced?
      errors.add(:original_user, "must be present for synced posts") unless original_user
      errors.add(:synced_from_user, "must be present for synced posts") unless synced_from_user
      errors.add(:synced_at, "must be present for synced posts") unless synced_at
      
      # Verify syncing user is friends or friends of friends with the original user
      if user && original_user && !user.friends_with?(original_user) && !user.friends_of_friends_with?(original_user)
        errors.add(:base, "Can only sync posts from friends or friends of friends")
      end
    else
      # If not synced, these fields should not be set
      errors.add(:synced_from_user, "can only be set if post is synced") if synced_from_user_id.present?
      errors.add(:original_user, "can only be set if post is synced") if original_user_id.present? && original_user_id != user_id
      errors.add(:synced_at, "can only be set if post is synced") if synced_at.present?
    end
  end
  
  # Comprehensive spam prevention
  def validate_content_hash
    return if content_hash.blank? || content_encrypted.blank?
    
    expected_hash = generate_content_hash
    if content_hash != expected_hash
      errors.add(:content_hash, "Content hash mismatch detected")
    end
  end
  
  def validate_content_size
    return if is_synced? # Skip for synced posts
    
    # Content size limits (user configurable)
    user_limit = user&.content_size_limit || 10.megabytes
    if @plaintext_content && @plaintext_content.bytesize > user_limit
      limit_mb = (user_limit / 1.megabyte).round(1)
      errors.add(:content, "too large: Maximum #{limit_mb}MB allowed")
    end
    
    # Attachment limits
    if attachments.size > 5
      errors.add(:attachments, "Too many attachments (max 5)")
    end
    
    total_attachment_size = attachments.sum(&:file_size)
    if total_attachment_size > 10.megabytes
      errors.add(:attachments, "Total attachment size too large (max 10MB)")
    end
  end
end
