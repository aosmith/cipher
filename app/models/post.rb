class Post < ApplicationRecord
  require 'rbnacl'
  require 'base64'

  belongs_to :user
  has_many :attachments, dependent: :destroy

  validates :signature, presence: true
  validates :timestamp, presence: true
  validate :content_or_attachments_present

  before_create :set_timestamp
  before_save :sign_content

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

  def sign_content
    # Create a signature for the entire post including attachments
    content_to_sign = [content_encrypted, attachments.map(&:checksum)].flatten.compact.join
    return if content_to_sign.empty?
    
    self.signature = user.sign_message(content_to_sign)
  end

  def decrypt_content
    # For public posts, we'll use a simpler encryption scheme
    # For private messages, this would decrypt using the user's private key
    # This is a placeholder - actual implementation depends on post type
    content_encrypted
  end

  def content_or_attachments_present
    if content_encrypted.blank? && attachments.empty?
      errors.add(:base, "Post must have either content or attachments")
    end
  end
end
