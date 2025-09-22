class MutualFriendSyncService
  def initialize(friendship)
    @friendship = friendship
  end

  def schedule_initial_sync
    return unless @friendship.status == "accepted"

    requester = @friendship.requester
    addressee = @friendship.addressee

    ensure_connection_record(requester, addressee)
    ensure_connection_record(addressee, requester)

    queue_sync_if_supported(requester, addressee)
    queue_sync_if_supported(addressee, requester)
  end

  private

  def ensure_connection_record(user, friend)
    authorizer = FriendPeerAuthorizer.new(user)

    if authorizer.allow?(friend)
      authorizer.ensure_connections!(friend)
    else
      P2pConnection.establish_connection(user, friend, "webrtc")
    end
  rescue => error
    Rails.logger.warn("Failed to establish P2P record for #{user.id} -> #{friend.id}: #{error.message}")
  end

  def queue_sync_if_supported(user, friend)
    peer = user.peers.find_by(public_key: friend.public_key)
    return unless peer

    existing_pending = SyncMessage.where(user:, peer:, status: "pending").exists?
    return if existing_pending

    MessageSyncService.new(user).sync_with_peer(peer)
  rescue => error
    Rails.logger.warn("Failed to queue sync for #{user.id} with #{friend.id}: #{error.message}")
  end
end
