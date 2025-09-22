class FriendPeerAuthorizer
  def initialize(user)
    @user = user
  end

  def allow?(peer_user)
    return false unless @user && peer_user
    return false if peer_user == @user

    @user.friends_with?(peer_user)
  end

  def ensure_connections!(peer_user, connection_type: "webrtc")
    return unless allow?(peer_user)

    establish_connection(@user, peer_user, connection_type)
    establish_connection(peer_user, @user, connection_type)
  end

  private

  def establish_connection(user, friend, connection_type)
    P2pConnection.establish_connection(user, friend, connection_type)
  rescue => error
    Rails.logger.debug { "FriendPeerAuthorizer failed to record connection for #{user.id} -> #{friend.id}: #{error.message}" }
  end
end
