class MessageSyncService
  def initialize(user)
    @user = user
  end

  def sync_with_peers
    # Get all posts that haven't been synchronized with peers yet
    unsync_posts = @user.posts.where(synced_at: nil)
    
    sync_results = {}
    
    @user.peers.active.find_each do |peer|
      result = sync_with_peer(peer, unsync_posts)
      sync_results[peer.id] = result
    end
    
    # Mark posts as synced if successfully sent to at least one peer
    if sync_results.values.any? { |result| result[:success] }
      unsync_posts.update_all(synced_at: Time.current)
    end
    
    sync_results
  end

  def sync_with_peer(peer, posts = nil)
    posts ||= @user.posts.recent.limit(100)
    
    sync_payload = {
      type: 'message_sync',
      user_id: @user.id,
      timestamp: Time.current.to_i,
      messages: posts.map { |post| serialize_post_for_sync(post) }
    }
    
    # This would be sent over WebRTC data channel
    # For now, we'll store it and let the WebRTC layer handle delivery
    
    SyncMessage.create!(
      user: @user,
      peer: peer,
      payload: sync_payload.to_json,
      message_type: 'outbound_sync',
      status: 'pending'
    )
    
    { success: true, message_count: posts.count }
  rescue => error
    { success: false, error: error.message }
  end

  def process_incoming_sync(peer, sync_data)
    return unless verify_sync_authenticity(peer, sync_data)
    
    processed_count = 0
    error_count = 0
    
    sync_data['messages'].each do |message_data|
      begin
        process_incoming_message(peer, message_data)
        processed_count += 1
      rescue => error
        Rails.logger.error "Failed to process message: #{error.message}"
        error_count += 1
      end
    end
    
    SyncMessage.create!(
      user: @user,
      peer: peer,
      payload: sync_data.to_json,
      message_type: 'inbound_sync',
      status: 'processed',
      processed_count: processed_count,
      error_count: error_count
    )
    
    { processed: processed_count, errors: error_count }
  end

  private

  def serialize_post_for_sync(post)
    {
      id: post.id,
      content_encrypted: post.content_encrypted,
      signature: post.signature,
      timestamp: post.timestamp.to_i,
      attachments: post.attachments.map do |attachment|
        {
          filename: attachment.filename,
          content_type: attachment.content_type,
          file_size: attachment.file_size,
          data_encrypted: attachment.data_encrypted,
          checksum: attachment.checksum
        }
      end
    }
  end

  def process_incoming_message(peer, message_data)
    # Verify the message signature
    peer_user = peer.user
    unless peer_user.verify_signature(message_data['content_encrypted'], 
                                      message_data['signature'], 
                                      peer_user.public_key)
      raise "Invalid message signature from peer #{peer.id}"
    end
    
    # Check if we already have this message
    existing_post = Post.find_by(
      user: peer_user,
      signature: message_data['signature']
    )
    
    return if existing_post
    
    # Create the post
    post = Post.new(
      user: peer_user,
      content_encrypted: message_data['content_encrypted'],
      signature: message_data['signature'],
      timestamp: Time.at(message_data['timestamp'])
    )
    
    # Add attachments if any
    message_data['attachments']&.each do |attachment_data|
      attachment = post.attachments.build(
        filename: attachment_data['filename'],
        content_type: attachment_data['content_type'],
        file_size: attachment_data['file_size'],
        data_encrypted: attachment_data['data_encrypted'],
        checksum: attachment_data['checksum']
      )
    end
    
    post.save!
  end

  def verify_sync_authenticity(peer, sync_data)
    # Add timestamp check to prevent replay attacks
    sync_timestamp = sync_data['timestamp']
    current_time = Time.current.to_i
    
    # Allow 5 minute window for clock differences
    if (current_time - sync_timestamp).abs > 300
      Rails.logger.warn "Sync message timestamp out of range from peer #{peer.id}"
      return false
    end
    
    true
  end
end