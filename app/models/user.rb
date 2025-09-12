class User < ApplicationRecord
  require 'rbnacl'
  require 'base64'
  require 'bcrypt'

  has_many :posts, dependent: :destroy
  has_many :comments, dependent: :destroy
  has_many :peers, dependent: :destroy
  
  # Message associations
  has_many :sent_messages, class_name: 'Message', foreign_key: 'sender_id', dependent: :destroy
  has_many :received_messages, class_name: 'Message', foreign_key: 'recipient_id', dependent: :destroy
  
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
  validates :email, uniqueness: { case_sensitive: false, message: "is already registered to another account." }, 
                   format: { with: URI::MailTo::EMAIL_REGEXP, message: "must be a valid email address" },
                   allow_blank: true

  before_validation :check_public_key_presence, on: :create
  before_create :generate_verification_code
  
  # Scopes
  scope :verified, -> { where.not(email_verified_at: nil) }
  scope :unverified, -> { where(email_verified_at: nil) }
  scope :search_by_email, ->(email) { where("email LIKE ?", "%#{email}%") }
  
  def name
    display_name
  end

  # SECURITY: Context-aware private key serialization
  # Users can access their own private key, but never other users' private keys
  def as_json(options = {})
    context_aware_serialization(:as_json, options)
  end
  
  def to_json(options = {})
    context_aware_serialization(:to_json, options)
  end
  
  def serializable_hash(options = {})
    context_aware_serialization(:serializable_hash, options)
  end
  
  private
  
  def context_aware_serialization(method, options = {})
    # Allow private key inclusion only if explicitly requested AND it's for the current user
    current_user_id = Thread.current[:current_user_for_serialization]
    include_private_key = options.delete(:include_private_key) && (current_user_id == self.id)
    
    # By default, exclude private keys from serialization
    unless include_private_key
      excluded = Array(options[:except]) + [:private_key, :private_key_encrypted]
      options = options.merge(except: excluded.uniq)
    end
    
    super(options)
  end

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
  
  public
  
  # Get all friends (both as requester and addressee)
  def friends
    User.where(id: friend_ids)
  end
  
  def friend_ids
    (friends_as_requester.pluck(:id) + friends_as_addressee.pluck(:id)).uniq
  end
  
  public
  
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
  
  # Get posts from friends
  def friends_posts
    Post.where(user_id: friend_ids)
  end
  
  # Get friends of friends (2-degree connections)
  def friends_of_friends
    return User.none if friend_ids.empty?
    
    # Find users who are friends with our friends, but not us directly
    # Users who received friend requests from our friends
    users_from_sent = User.joins(:received_friendships)
        .where(received_friendships: { 
          requester_id: friend_ids, 
          status: 'accepted' 
        })
        .where.not(id: self.id) # Exclude self
        .where.not(id: friend_ids) # Exclude direct friends
        .distinct
    
    # Users who sent friend requests to our friends  
    users_from_received = User.joins(:sent_friendships)
        .where(sent_friendships: { 
          addressee_id: friend_ids, 
          status: 'accepted' 
        })
        .where.not(id: self.id) # Exclude self
        .where.not(id: friend_ids) # Exclude direct friends
        .distinct
        
    # Combine both queries
    User.where(id: (users_from_sent.pluck(:id) + users_from_received.pluck(:id)).uniq)
  end
  
  # Check if user is a friend of a friend (2-degree connection)
  def friends_of_friends_with?(user)
    return false if user == self
    return false if friends_with?(user) # Already direct friends
    
    # Check if we have mutual friends
    mutual_friends = friend_ids & user.friend_ids
    mutual_friends.any?
  end
  
  # Email verification methods
  def email_verified?
    email_verified_at.present?
  end
  
  def generate_verification_code
    return unless email.present?
    self.verification_code = SecureRandom.alphanumeric(6).upcase
    self.verification_code_expires_at = 15.minutes.from_now
  end
  
  def verify_email_with_code(code)
    return false if verification_code.blank?
    return false if verification_code_expires_at < Time.current
    return false unless verification_code == code.upcase
    
    self.email_verified_at = Time.current
    self.verification_code = nil
    self.verification_code_expires_at = nil
    save!
    
    true
  end
  
  def send_verification_email
    # In a real implementation, this would send an email with the verification code
    # For now, this is a placeholder that could integrate with ActionMailer
    Rails.logger.info "Verification code for #{email}: #{verification_code}"
    true
  end
  
  def resend_verification_code
    generate_verification_code
    save!
    send_verification_email
  end
  
  public
  
  # Messaging methods
  def messages_with(user)
    Message.between_users(self, user).recent
  end
  
  def unread_messages_count
    received_messages.unread.count
  end
  
  def conversations
    # Get all unique users this user has messaged or received messages from
    sent_recipients = sent_messages.joins(:recipient).select('users.id, users.username, MAX(messages.created_at) as last_message_at').group('users.id, users.username')
    received_senders = received_messages.joins(:sender).select('users.id, users.username, MAX(messages.created_at) as last_message_at').group('users.id, users.username')
    
    # Combine and get unique conversation partners
    conversation_user_ids = (sent_messages.pluck(:recipient_id) + received_messages.pluck(:sender_id)).uniq
    User.where(id: conversation_user_ids)
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
