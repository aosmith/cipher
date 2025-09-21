require "test_helper"
require "base64"

class FriendshipTest < ActiveSupport::TestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ SyncMessage, P2pConnection, Peer, Friendship, AttachmentShare, Attachment, Comment, Post, User ].each do |model|
        model.delete_all if defined?(model)
      end
    end

    @alice = User.create!(
      username: "alice_friend",
      display_name: "Alice",
      public_key: Base64.strict_encode64("alice_friend_key_1234567890")
    )

    @bob = User.create!(
      username: "bob_friend",
      display_name: "Bob",
      public_key: Base64.strict_encode64("bob_friend_key_0987654321")
    )
  end

  test "accepted friendship triggers mutual sync scheduling" do
    assert_difference -> { P2pConnection.count }, 2 do
      Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")
    end

    assert_equal 0, SyncMessage.count, "sync messages require peer records"
  end

  test "pending friendship schedules sync when accepted later" do
    friendship = Friendship.create!(requester: @alice, addressee: @bob, status: "pending")

    assert_no_difference -> { P2pConnection.count } do
      # Pending status does not trigger sync
      friendship.touch
    end

    assert_difference -> { P2pConnection.count }, 2 do
      friendship.update!(status: "accepted")
    end
  end
end
