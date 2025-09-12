class Api::V1::SyncController < ApplicationController
  before_action :require_current_user_session
  before_action :check_rate_limits, only: [:sync_data, :accept_sync]
  
  # GET /api/v1/sync_data - Get data to share with a friend
  def sync_data
    friend_id = params[:friend_id]
    friend = User.find(friend_id)
    
    # Security: Allow sync data sharing with friends and friends of friends
    unless current_user_session.friends_with?(friend) || current_user_session.friends_of_friends_with?(friend)
      render json: { error: 'Access denied: Can only sync with friends or friends of friends' }, status: :forbidden
      return
    end
    
    # Get original posts from current user (exclude synced posts)
    posts = current_user_session.posts.original_posts.includes(:attachments).recent.limit(50)
    
    posts_data = posts.map do |post|
      {
        content: post.content_encrypted,
        original_user_id: post.original_user_id,
        content_hash: post.content_hash,
        created_at: post.created_at.iso8601,
        timestamp: post.timestamp.iso8601
      }
    end
    
    sync_response = {
      posts: posts_data,
      user_id: current_user_session.id,
      sync_metadata: {
        last_sync_time: Time.current.iso8601,
        total_posts: posts.count,
        user_public_key: current_user_session.public_key,
        user_id: current_user_session.id
      }
    }
    
    render json: sync_response
  end
  
  # POST /api/v1/accept_sync - Accept sync data from a friend
  def accept_sync
    friend_id = params[:friend_id]
    sync_data = params[:sync_data]
    friend = User.find(friend_id)
    
    # Security: Allow accepting sync data from friends and friends of friends
    unless current_user_session.friends_with?(friend) || current_user_session.friends_of_friends_with?(friend)
      render json: { error: 'Access denied: Can only sync with friends or friends of friends' }, status: :forbidden
      return
    end
    
    # Validate sync data structure
    unless sync_data && sync_data[:posts] && sync_data[:user_id]
      render json: { error: 'Invalid sync data format' }, status: :bad_request
      return
    end
    
    # Security validation
    begin
      validate_sync_data_security(sync_data)
    rescue => e
      render json: { error: e.message }, status: :bad_request
      return
    end
    
    # Validate bulk limits
    if sync_data[:posts].size > 100
      render json: { error: 'Too many posts in sync batch (max 100)' }, status: :bad_request
      return
    end
    
    synced_count = 0
    skipped_reasons = []
    
    sync_data[:posts].each do |post_data|
      begin
        # Check for oversized content
        if post_data[:content] && post_data[:content].bytesize > 10.kilobytes
          skipped_reasons << 'Content too large'
          next
        end
        
        # Check for duplicates
        if Post.where(content_hash: post_data[:content_hash], user: current_user_session).exists?
          skipped_reasons << 'Duplicate content detected'
          next
        end
        
        # Create synced post
        synced_post = current_user_session.posts.build(
          content_encrypted: post_data[:content],
          is_synced: true,
          original_user_id: post_data[:original_user_id],
          synced_from_user_id: friend.id,
          synced_at: Time.current,
          content_hash: post_data[:content_hash],
          timestamp: Time.parse(post_data[:created_at])
        )
        
        if synced_post.save
          synced_count += 1
        else
          skipped_reasons << synced_post.errors.full_messages.join(', ')
        end
        
      rescue => e
        skipped_reasons << e.message
      end
    end
    
    render json: {
      success: true,
      synced_posts_count: synced_count,
      skipped_reasons: skipped_reasons.uniq
    }
  end
  
  # GET /api/v1/users/:user_id/friends
  def friends
    user = User.find(params[:user_id])
    
    # Security: Only allow users to get their own friends list
    unless user == current_user_session
      render json: { error: 'Unauthorized' }, status: :unauthorized
      return
    end
    
    # SECURITY: Explicitly exclude private keys from friend data
    friends = user.friends.select(:id, :username, :public_key, :display_name)
                         .map { |friend| friend.attributes.except('private_key', 'private_key_encrypted') }
    
    render json: friends
  end
  
  # GET /api/v1/posts/my_posts_for_friends
  def my_posts_for_friends
    posts = current_user_session.posts.includes(:attachments)
    
    posts_data = posts.map do |post|
      {
        id: post.id,
        content_hash: generate_content_hash(post),
        timestamp: post.timestamp,
        attachments_count: post.attachments.count,
        attachment_hashes: post.attachments.map { |att| att.checksum }
      }
    end
    
    render json: posts_data
  end
  
  # GET /api/v1/users/:user_id/posts/for_sync
  def posts_for_sync
    friend = User.find(params[:user_id])
    
    # Security: Only return posts if requester is friends with the user
    unless current_user_session.friends_with?(friend)
      render json: { error: 'Not authorized to sync with this user' }, status: :unauthorized
      return
    end
    
    posts = friend.posts.includes(:attachments).recent.limit(100)
    
    posts_data = posts.map do |post|
      {
        id: post.id,
        content_hash: generate_content_hash(post),
        timestamp: post.timestamp,
        has_attachments: post.attachments.any?
      }
    end
    
    render json: posts_data
  end
  
  # GET /api/v1/posts/:id/sync_data
  def post_sync_data
    post = Post.find(params[:id])
    
    # Security: Only allow if requester is friends with post owner
    unless current_user_session.friends_with?(post.user)
      render json: { error: 'Not authorized to sync this post' }, status: :unauthorized
      return
    end
    
    post_data = {
      id: post.id,
      user_id: post.user_id,
      content_encrypted: post.content_encrypted,
      timestamp: post.timestamp,
      signature: post.signature,
      created_at: post.created_at,
      updated_at: post.updated_at
    }
    
    # Include attachments if requested
    if post.attachments.any?
      post_data[:attachments] = post.attachments.map do |attachment|
        {
          id: attachment.id,
          filename: attachment.filename,
          content_type: attachment.content_type,
          file_size: attachment.file_size,
          checksum: attachment.checksum,
          data_encrypted: attachment.data_encrypted
          # SECURITY: Never include any user private key data in attachment sync
        }
      end
    end
    
    render json: post_data
  end
  
  # POST /api/v1/posts/sync_store
  def sync_store
    post_params = params.require(:post)
    original_user_id = params.require(:original_user_id)
    
    original_user = User.find(original_user_id)
    
    # Security: Only allow storing posts from friends
    unless current_user_session.friends_with?(original_user)
      render json: { error: 'Not authorized to store posts from this user' }, status: :unauthorized
      return
    end
    
    begin
      # SECURITY: Validate that sync data contains no private keys
      validate_sync_data_security(post_params)
      
      # Check sync-specific spam prevention
      Post.check_sync_spam(current_user_session, original_user)
      
      # Create a synced post record using the unified Post model
      synced_post = Post.new
      synced_post.user = current_user_session
      synced_post.original_user = original_user
      synced_post.synced_from_user = original_user
      synced_post.is_synced = true
      synced_post.synced_at = Time.current
      synced_post.content_encrypted = post_params[:content_encrypted]
      synced_post.timestamp = Time.parse(post_params[:timestamp]) if post_params[:timestamp]
      synced_post.signature = post_params[:signature]
      
      synced_post.save!
      
      render json: { success: true, synced_post_id: synced_post.id }
      
    rescue => e
      # Handle both spam prevention and validation errors
      render json: { error: e.message }, status: :unprocessable_entity
    end
  end
  
  # GET /api/v1/content/:content_hash/exists
  def content_exists
    content_hash = params[:content_hash]
    
    # Check if we have this content (either as our own post or synced)
    exists = current_user_session.posts.joins("LEFT JOIN synced_posts ON posts.id = synced_posts.original_post_id")
                                      .where(
                                        "SHA256(CONCAT(posts.content_encrypted, posts.timestamp, posts.signature)) = ? OR 
                                         synced_posts.content_hash = ?",
                                        content_hash, content_hash
                                      ).exists?
    
    render json: { exists: exists }
  end
  
  # GET /api/v1/sync/stats
  def sync_stats
    stats = {
      friends_count: current_user_session.friends.count,
      my_original_posts_count: current_user_session.posts.original_posts.count,
      synced_posts_count: current_user_session.posts.synced_posts.count,
      total_posts_count: current_user_session.posts.count,
      storage_used_mb: calculate_storage_used,
      last_sync_activity: last_sync_activity,
      sync_limits: {
        hourly_sync_limit: 20,
        daily_sync_limit: 100,
        storage_limit_mb: 100
      }
    }
    
    render json: stats
  end
  
  private
  
  def generate_content_hash(post)
    content_to_hash = [
      post.content_encrypted,
      post.timestamp&.to_i&.to_s,
      post.signature,
      post.attachments.map(&:checksum).sort.join
    ].join
    
    Digest::SHA256.hexdigest(content_to_hash)
  end
  
  def generate_content_hash_from_data(post_data)
    content_to_hash = [
      post_data[:content_encrypted],
      post_data[:timestamp],
      post_data[:signature]
    ].compact.join
    
    Digest::SHA256.hexdigest(content_to_hash)
  end
  
  def calculate_storage_used
    # Calculate total storage used for synced content
    synced_posts = current_user_session.posts.synced_posts
    
    total_size = synced_posts.sum do |post|
      (post.content_encrypted&.bytesize || 0) + 
      post.attachments.sum(&:file_size)
    end
    
    # Convert to MB
    (total_size / (1024.0 * 1024.0)).round(2)
  end
  
  def last_sync_activity
    last_synced_post = current_user_session.posts.synced_posts
                                          .order(synced_at: :desc)
                                          .first
    
    last_synced_post&.synced_at
  end
  
  def require_current_user_session
    unless current_user_session
      render json: { error: 'Authentication required' }, status: :unauthorized
    end
  end
  
  def check_rate_limits
    # Simple rate limiting: max 10 requests per hour per user
    cache_key = "sync_rate_limit:#{current_user_session.id}"
    current_count = Rails.cache.read(cache_key) || 0
    
    if current_count >= 10
      render json: { error: 'Rate limit exceeded: Maximum 10 sync requests per hour' }, status: :too_many_requests
      return false
    end
    
    Rails.cache.write(cache_key, current_count + 1, expires_in: 1.hour)
    true
  end
  
  # SECURITY: Ensure no private key data ever gets synced
  def sanitize_user_data(user_data)
    if user_data.is_a?(Hash)
      user_data.except('private_key', 'private_key_encrypted', :private_key, :private_key_encrypted)
    elsif user_data.respond_to?(:attributes)
      user_data.attributes.except('private_key', 'private_key_encrypted')
    else
      user_data
    end
  end
  
  # SECURITY: Validate that sync data contains no private key information
  def validate_sync_data_security(data)
    data_str = data.to_json.downcase
    
    # Check for any private key patterns
    forbidden_patterns = [
      'private_key', 'privatekey', 'private-key',
      'secret_key', 'secretkey', 'secret-key',
      'encryption_key', 'encryptionkey', 'encryption-key'
    ]
    
    forbidden_patterns.each do |pattern|
      if data_str.include?(pattern)
        raise "SECURITY VIOLATION: Private key data detected in sync payload"
      end
    end
    
    true
  end
end