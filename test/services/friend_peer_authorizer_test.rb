require "test_helper"
require "base64"

class FriendPeerAuthorizerTest < ActiveSupport::TestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ P2pConnection, Friendship, User ].each(&:delete_all)
    end

    @alice = User.create!(
      username: "alice_peer",
      display_name: "Alice",
      public_key: Base64.strict_encode64("alice_peer_public_key_123456")
    )

    @bob = User.create!(
      username: "bob_peer",
      display_name: "Bob",
      public_key: Base64.strict_encode64("bob_peer_public_key_654321")
    )

    @carol = User.create!(
      username: "carol_peer",
      display_name: "Carol",
      public_key: Base64.strict_encode64("carol_peer_public_key_222222")
    )

    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")
  end

  test "allows only accepted friends" do
    authorizer = FriendPeerAuthorizer.new(@alice)

    assert authorizer.allow?(@bob), "expected Bob to be treated as a friend"
    refute authorizer.allow?(@carol), "non-friends must not be authorized"
    refute authorizer.allow?(@alice), "users should not authorize themselves"
  end

  test "records mutual connection when ensuring access" do
    authorizer = FriendPeerAuthorizer.new(@alice)

    authorizer.ensure_connections!(@bob)

    assert P2pConnection.exists?(user: @alice, friend_user: @bob), "expected alice to record connection to bob"
    assert P2pConnection.exists?(user: @bob, friend_user: @alice), "expected bob to record connection to alice"
  end

  test "ignores connection recording for non-friends" do
    authorizer = FriendPeerAuthorizer.new(@alice)

    assert_no_difference -> { P2pConnection.count } do
      authorizer.ensure_connections!(@carol)
    end
  end
end
