class SignalingChannel < ApplicationCable::Channel
  def subscribed
    @user = current_user
    return reject unless @user

    stream_from "signaling_#{@user.id}"
    
    # Announce presence to other peers
    announce_presence
  end

  def unsubscribed
    # Remove from active peers when disconnecting
    @user&.peers&.update_all(last_seen: Time.current)
  end

  # WebRTC offer from one peer to another
  def send_offer(data)
    recipient_id = data['recipient_id']
    recipient = User.find_by(id: recipient_id)

    return unless recipient && allow_friend_connection?(recipient)

    ActionCable.server.broadcast("signaling_#{recipient_id}", {
      type: 'offer',
      sender_id: @user.id,
      sender_public_key: @user.public_key,
      offer: data['offer'],
      timestamp: Time.current.to_i
    })
  end

  # WebRTC answer from recipient back to sender
  def send_answer(data)
    sender_id = data['sender_id']
    sender = User.find_by(id: sender_id)

    return unless sender && allow_friend_connection?(sender)

    ActionCable.server.broadcast("signaling_#{sender_id}", {
      type: 'answer',
      sender_id: @user.id,
      sender_public_key: @user.public_key,
      answer: data['answer'],
      timestamp: Time.current.to_i
    })
  end

  # ICE candidates exchange
  def send_ice_candidate(data)
    recipient_id = data['recipient_id']
    recipient = User.find_by(id: recipient_id)

    return unless recipient && allow_friend_connection?(recipient)

    ActionCable.server.broadcast("signaling_#{recipient_id}", {
      type: 'ice_candidate',
      sender_id: @user.id,
      candidate: data['candidate'],
      timestamp: Time.current.to_i
    })
  end

  # Peer discovery - request list of available peers
  def discover_peers(data)
    friend_contacts = @user.friends.map do |friend|
      next unless allow_friend_connection?(friend)

      peer_record = @user.peers.find_by(public_key: friend.public_key)
      next unless peer_record&.online?

      peer_record.connection_profile_for(friend)
    end.compact

    transmit({
      type: 'peer_list',
      peers: friend_contacts
    })
  end

  # Challenge-response authentication
  def authenticate_peer(data)
    challenge = data['challenge']
    signature = data['signature']
    peer_public_key = data['public_key']
    
    if @user.verify_signature(challenge, signature, peer_public_key)
      # Authentication successful
      transmit({
        type: 'auth_success',
        peer_id: @user.id,
        challenge_response: @user.sign_message(challenge)
      })
    else
      transmit({ type: 'auth_failed' })
    end
  end

  private

  def current_user
    # This would be implemented based on your authentication system
    # For now, we'll use a simple approach - in production you'd want proper session management
    user_id = params[:user_id]
    User.find_by(id: user_id) if user_id
  end

  def allow_friend_connection?(peer_user, connection_type: 'webrtc')
    return false unless friend_authorizer.allow?(peer_user)

    friend_authorizer.ensure_connections!(peer_user, connection_type: connection_type)
    true
  end

  def announce_presence
    # Update last_seen for user's peers
    @user.peers.update_all(last_seen: Time.current)
    
    # Broadcast to other users that this peer is online
    ActionCable.server.broadcast('peer_presence', {
      type: 'peer_online',
      peer_id: @user.id,
      username: @user.username,
      public_key: @user.public_key,
      timestamp: Time.current.to_i
    })
  end

  def friend_authorizer
    @friend_authorizer ||= FriendPeerAuthorizer.new(@user)
  end
end
