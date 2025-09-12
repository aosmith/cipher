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
  validate :spam_prevention_checks, on: :create, unless: :is_synced?

  before_validation :set_timestamp, on: :create
  before_validation :encrypt_content, on: [:create, :update], unless: :is_synced?
  before_validation :sign_content, on: :create, unless: :is_synced?
  before_validation :set_content_hash, on: [:create, :update]
  after_save :clear_plaintext_cache
  
  # Validation for synced posts
  validate :sync_fields_consistency, if: :is_synced?

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

  def encrypt_content
    return if @plaintext_content.blank?
    
    # For now, we'll use a simple encryption scheme
    # In a real implementation, this would be properly encrypted
    self.content_encrypted = @plaintext_content
  end

  def sign_content
    # Create a signature for the entire post including attachments
    content_to_sign = [content_encrypted, attachments.map(&:checksum)].flatten.compact.join
    
    # If there's no content to sign, create a minimal signature with timestamp
    if content_to_sign.empty?
      content_to_sign = timestamp&.to_i&.to_s || Time.current.to_i.to_s
    end
    
    # For now, create a simple signature (in production this would use proper cryptographic signing)
    self.signature = Digest::SHA256.hexdigest("#{user.id}-#{content_to_sign}-#{timestamp}")
  end

  def decrypt_content
    # For public posts, we'll use a simpler encryption scheme
    # For private messages, this would decrypt using the user's private key
    # This is a placeholder - actual implementation depends on post type
    content_encrypted
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
  
  def can_be_synced_by?(user)
    # Can only sync posts from friends
    return false if synced_post? # Don't sync already synced posts
    return false unless self.user # Must have an original user
    
    user.friends_with?(self.user)
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
    content_to_hash = [
      content_encrypted,
      timestamp&.to_i&.to_s,
      signature,
      attachments.map(&:checksum).sort.join
    ].compact.join
    
    Digest::SHA256.hexdigest(content_to_hash)
  end
  
  def sync_fields_consistency
    if is_synced?
      errors.add(:original_user, "must be present for synced posts") unless original_user
      errors.add(:synced_from_user, "must be present for synced posts") unless synced_from_user
      errors.add(:synced_at, "must be present for synced posts") unless synced_at
      
      # Verify syncing user is friends with the original user
      if user && original_user && !user.friends_with?(original_user)
        errors.add(:base, "Can only sync posts from friends")
      end
    end
  end
  
  # Comprehensive spam prevention
  def spam_prevention_checks
    return if is_synced? # Skip spam checks for synced posts
    
    # Rate limiting: max posts per hour
    recent_posts_count = user.posts.original_posts
                            .where('created_at > ?', 1.hour.ago)
                            .count
    
    if recent_posts_count >= 10
      errors.add(:base, "Rate limit exceeded: Maximum 10 posts per hour")
      return
    end
    
    # Rate limiting: max posts per day
    daily_posts_count = user.posts.original_posts
                           .where('created_at > ?', 1.day.ago)
                           .count
    
    if daily_posts_count >= 50
      errors.add(:base, "Daily limit exceeded: Maximum 50 posts per day")
      return
    end
    
    # Content size limits
    if content_encrypted && content_encrypted.bytesize > 10.kilobytes
      errors.add(:content, "Content too large (max 10KB)")
    end
    
    # Attachment limits
    if attachments.size > 5
      errors.add(:attachments, "Too many attachments (max 5)")
    end
    
    total_attachment_size = attachments.sum(&:file_size)
    if total_attachment_size > 10.megabytes
      errors.add(:attachments, "Total attachment size too large (max 10MB)")
    end
    
    # Duplicate content prevention
    if content_encrypted.present?
      content_hash_check = Digest::SHA256.hexdigest(content_encrypted)
      recent_duplicate = user.posts.original_posts
                            .where('created_at > ?', 1.hour.ago)
                            .find { |post| Digest::SHA256.hexdigest(post.content_encrypted || '') == content_hash_check }
      
      if recent_duplicate
        errors.add(:content, "Duplicate content detected")
      end
    end
    
    # Friendship requirement for new users
    if user.created_at > 7.days.ago && user.friends.count < 1
      errors.add(:base, "New users must have at least one friend to post")
    end
  end
  
  # Sync-specific spam prevention
  def self.check_sync_spam(syncing_user, original_user)
    # Limit sync frequency between users
    recent_syncs = syncing_user.posts.synced_posts
                               .where(original_user: original_user)
                               .where('synced_at > ?', 1.hour.ago)
                               .count
    
    if recent_syncs >= 20
      raise "Sync rate limit exceeded: Max 20 syncs per hour from each friend"
    end
    
    # Daily sync limit
    daily_syncs = syncing_user.posts.synced_posts
                              .where('synced_at > ?', 1.day.ago)
                              .count
    
    if daily_syncs >= 100
      raise "Daily sync limit exceeded: Max 100 syncs per day"
    end
    
    # Storage limit for synced content
    synced_content_size = syncing_user.posts.synced_posts.sum do |post|
      (post.content_encrypted&.bytesize || 0) + 
      post.attachments.sum(&:file_size)
    end
    
    if synced_content_size > 100.megabytes
      raise "Synced content storage limit exceeded: Max 100MB"
    end
  end
end
