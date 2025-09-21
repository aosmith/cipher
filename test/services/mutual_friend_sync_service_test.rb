require "test_helper"
require "base64"

class MutualFriendSyncServiceTest < ActiveSupport::TestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [SyncMessage, P2pConnection, Peer, Friendship, AttachmentShare, Attachment, Comment, Post, User].each do |model|
        model.delete_all if defined?(model)
      end
    end

    @alice = User.create!(
      username: "alice_sync",
      display_name: "Alice",
      public_key: Base64.strict_encode64("alice_sync_key_1234567890123456")
    )

    @bob = User.create!(
      username: "bob_sync",
      display_name: "Bob",
      public_key: Base64.strict_encode64("bob_sync_key_1234567890123456")
    )

    @alice_peer = @alice.peers.create!(
      address: "127.0.0.1",
      port: 9000,
      public_key: @bob.public_key,
      last_seen: Time.current
    )

    @friendship = Friendship.new(requester: @alice, addressee: @bob, status: "accepted")
  end

  test "schedules connections and pending sync for known peer" do
    service = MutualFriendSyncService.new(@friendship)
    service.schedule_initial_sync

    assert P2pConnection.exists?(user: @alice, friend_user: @bob), "connection record for requester missing"
    assert P2pConnection.exists?(user: @bob, friend_user: @alice), "connection record for addressee missing"

    pending_message = SyncMessage.find_by(user: @alice, peer: @alice_peer)
    assert pending_message, "expected a pending sync message for alice"
    assert_equal "pending", pending_message.status
    assert_equal 0, SyncMessage.where(user: @bob).count, "bob should not queue sync without peer record"
  end

  test "does not duplicate pending sync messages" do
    service = MutualFriendSyncService.new(@friendship)
    service.schedule_initial_sync

    assert_no_difference -> { SyncMessage.count } do
      service.schedule_initial_sync
    end
  end
end
