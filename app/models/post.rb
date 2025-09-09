class Post < ApplicationRecord
  require 'rbnacl'
  require 'base64'
  require 'digest'

  belongs_to :user
  has_many :attachments, dependent: :destroy

  scope :recent, -> { order(timestamp: :desc) }

  validates :timestamp, presence: true
  validate :content_or_attachments_present

  before_validation :set_timestamp, on: :create
  before_validation :encrypt_content, on: :create
  before_validation :sign_content, on: :create

  def content=(plaintext_content)
    @plaintext_content = plaintext_content
  end

  def content
    return @plaintext_content if @plaintext_content
    
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
    has_content = @plaintext_content.present? || content_encrypted.present?
    has_attachments = attachments.any?
    
    unless has_content || has_attachments
      errors.add(:base, "Post must have either content or attachments")
    end
  end
end
