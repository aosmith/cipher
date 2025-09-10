class User < ApplicationRecord
  require 'rbnacl'
  require 'base64'
  require 'bcrypt'

  has_many :posts, dependent: :destroy
  has_many :peers, dependent: :destroy
  
  # Friendship associations
  has_many :sent_friendships, class_name: 'Friendship', foreign_key: 'requester_id', dependent: :destroy
  has_many :received_friendships, class_name: 'Friendship', foreign_key: 'addressee_id', dependent: :destroy
  
  # Friends through accepted friendships
  has_many :friends_as_requester, -> { where(friendships: { status: 'accepted' }) }, 
           through: :sent_friendships, source: :addressee
  has_many :friends_as_addressee, -> { where(friendships: { status: 'accepted' }) }, 
           through: :received_friendships, source: :requester
  
  # Attachment shares
  has_many :attachment_shares, dependent: :destroy
  has_many :accessible_attachments, through: :attachment_shares, source: :attachment

  validates :username, presence: true, 
                      uniqueness: { message: "is already taken. Please choose a different username." }
  validates :public_key, presence: true, 
                        uniqueness: { message: "is already registered to another account. Please regenerate your keys." },
                        allow_blank: false

  before_validation :check_public_key_presence, on: :create

  # Note: These methods are now handled client-side in JavaScript
  # The server never sees or handles private keys

  def verify_signature(message, signature, sender_public_key)
    verify_key = RbNaCl::VerifyKey.new(Base64.decode64(sender_public_key))
    begin
      verify_key.verify(Base64.decode64(signature), message)
      true
    rescue RbNaCl::BadSignatureError
      false
    end
  end

  # Validate that a username matches a given public key
  # This is used for client-side key derivation verification
  def self.validate_identity(username, public_key_base64)
    user = find_by(username: username)
    return false unless user
    user.public_key == public_key_base64
  end
  
  # Get all friends (both as requester and addressee)
  def friends
    User.where(id: friend_ids)
  end
  
  def friend_ids
    (friends_as_requester.pluck(:id) + friends_as_addressee.pluck(:id)).uniq
  end
  
  # Check if two users are friends
  def friends_with?(user)
    return false if user == self
    Friendship.where(
      "(requester_id = ? AND addressee_id = ? AND status = ?) OR (requester_id = ? AND addressee_id = ? AND status = ?)",
      self.id, user.id, 'accepted', user.id, self.id, 'accepted'
    ).exists?
  end
  
  # Send friend request
  def send_friend_request_to(user)
    return false if user == self
    return false if friends_with?(user)
    
    existing_friendship = Friendship.where(
      "(requester_id = ? AND addressee_id = ?) OR (requester_id = ? AND addressee_id = ?)",
      self.id, user.id, user.id, self.id
    ).first
    
    return false if existing_friendship # Already exists
    
    sent_friendships.create(addressee: user, status: 'pending')
  end
  
  # Get pending friend requests received by this user
  def pending_friend_requests
    received_friendships.pending.includes(:requester)
  end
  
  # Get pending friend requests sent by this user
  def sent_friend_requests
    sent_friendships.pending.includes(:addressee)
  end

  private

  # Validate that public key was generated client-side and provided
  def check_public_key_presence
    return if public_key.present?
    
    # Check if this is likely a JavaScript failure vs missing form data
    if username.present?
      errors.add(:base, "⚠️ Key generation failed. Please ensure JavaScript is enabled and try refreshing the page.")
    else
      errors.add(:base, "🔐 Cryptographic keys must be generated on your device for security. Please use the account creation form.")
    end
    
    throw :abort
  end
end
