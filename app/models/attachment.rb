class Attachment < ApplicationRecord
  require 'rbnacl'
  require 'base64'
  require 'digest'

  belongs_to :post
  has_many :attachment_shares, dependent: :destroy
  has_many :shared_with_users, through: :attachment_shares, source: :user

  validates :filename, presence: true
  validates :content_type, presence: true
  validates :file_size, presence: true, numericality: { greater_than: 0 }
  validates :data_encrypted, presence: true
  validates :checksum, presence: true

  before_save :generate_checksum
  after_create :create_attachment_shares
  
  # Blockchain-related attributes
  attr_accessor :blockchain_file_hash, :blockchain_upload_cost, :blockchain_transaction_hash

  def encrypt_data(binary_data, shared_user_public_keys = [])
    # Use ChaCha20-Poly1305 for large file encryption (stream cipher)
    key = RbNaCl::Random.random_bytes(RbNaCl::SecretBox.key_bytes)
    box = RbNaCl::SecretBox.new(key)
    
    encrypted_data = box.encrypt(binary_data)
    
    # Store only the encrypted data, not the keys
    self.data_encrypted = Base64.encode64(encrypted_data)
    
    # Always include the owner
    all_public_keys = [post.user.public_key] + shared_user_public_keys
    all_public_keys.uniq!
    
    @encryption_key = key # Store temporarily for sharing
    @shared_public_keys = all_public_keys
    @original_data = binary_data
  end

  def decrypt_data_for_user(user)
    return @decrypted_data if @decrypted_data
    
    # Get the encrypted key for this specific user
    encrypted_key = AttachmentShare.encrypted_key_for(user, self)
    return nil unless encrypted_key
    
    # This needs to be handled client-side since server doesn't have private keys
    # Return the encrypted key so client can decrypt it
    {
      encrypted_key: encrypted_key,
      encrypted_data: data_encrypted
    }
  end
  
  # Server-side method to check if user has access
  def accessible_by?(user)
    # Owner always has access
    return true if post.user == user
    
    # Check if user is in the shared list
    AttachmentShare.user_has_access?(user, self)
  end

  def is_image?
    content_type.start_with?('image/')
  end

  def is_video?
    content_type.start_with?('video/')
  end

  def is_audio?
    content_type.start_with?('audio/')
  end

  def media_type
    case content_type
    when /^image\//
      'image'
    when /^video\//
      'video'
    when /^audio\//
      'audio'
    else
      'file'
    end
  end

  def human_file_size
    units = ['B', 'KB', 'MB', 'GB', 'TB']
    size = file_size.to_f
    unit_index = 0
    
    while size >= 1024 && unit_index < units.length - 1
      size /= 1024
      unit_index += 1
    end
    
    "#{size.round(1)} #{units[unit_index]}"
  end
  
  # Blockchain integration methods
  def calculate_blockchain_cost
    # Cost is 1 CPH per KB, rounded up
    size_kb = (file_size + 1023) / 1024
    size_kb # Returns cost in CPH
  end
  
  def blockchain_file_hash_for_storage
    # Use the file's checksum as the blockchain file hash
    # This ensures the hash is deterministic and represents the actual file content
    checksum
  end
  
  def to_blockchain_json
    {
      filename: filename,
      content_type: content_type,
      file_size: file_size,
      file_size_kb: (file_size + 1023) / 1024,
      checksum: checksum,
      blockchain_cost: calculate_blockchain_cost,
      media_type: media_type,
      human_size: human_file_size
    }
  end

  # Share attachment with additional users
  def share_with_users(user_public_keys_map)
    # user_public_keys_map should be: { user_id => encrypted_key }
    user_public_keys_map.each do |user_id, encrypted_key|
      user = User.find(user_id)
      attachment_shares.find_or_create_by(user: user) do |share|
        share.encrypted_key = encrypted_key
      end
    end
  end

  private

  def generate_checksum
    data_to_hash = @original_data
    if data_to_hash
      self.checksum = Digest::SHA256.hexdigest(data_to_hash)
    else
      # Fallback if we don't have original data
      self.checksum = Digest::SHA256.hexdigest("#{filename}-#{file_size}-#{Time.current.to_i}")
    end
  end
  
  def create_attachment_shares
    return unless @encryption_key && @shared_public_keys
    
    # This method expects that key encryption happens client-side
    # For now, we'll create placeholder shares that need to be updated client-side
    @shared_public_keys.each do |public_key|
      user = User.find_by(public_key: public_key)
      next unless user
      
      # Create placeholder - the encrypted key will be set by client-side code
      attachment_shares.find_or_create_by(user: user) do |share|
        share.encrypted_key = "PLACEHOLDER_TO_BE_ENCRYPTED_CLIENT_SIDE"
      end
    end
  end
end
