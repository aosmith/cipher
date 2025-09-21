require "test_helper"
require "json"

class SignalingChannelTest < ActionCable::Channel::TestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ P2pConnection, Peer, Friendship, User ].each(&:delete_all)
    end

    @alice = User.create!(username: "alice_signal", display_name: "Alice", public_key: "alice_pub")
    @bob   = User.create!(username: "bob_signal", display_name: "Bob", public_key: "bob_pub")

    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")
  end

  test "subscribes only with valid user" do
    subscribe(user_id: @alice.id)

    assert subscription.confirmed?
    assert_has_stream "signaling_#{@alice.id}"
  end

  test "rejects when user missing" do
    subscribe

    refute subscription.confirmed?
  end

  test "broadcasts offer only to friends" do
    subscribe(user_id: @alice.id)

    perform :send_offer, recipient_id: @bob.id, offer: { sdp: "demo" }

    raw_payload = broadcasts("signaling_#{@bob.id}").last
    refute_nil raw_payload, "expected broadcast to friend"

    payload = JSON.parse(raw_payload)
    assert_equal "offer", payload["type"]
    assert_equal @alice.id, payload["sender_id"]
  end

  test "ignores offer to non-friend" do
    subscribe(user_id: @alice.id)

    eve = User.create!(username: "eve_signal", display_name: "Eve", public_key: "eve_pub")

    assert_no_broadcasts "signaling_#{eve.id}" do
      perform :send_offer, recipient_id: eve.id, offer: { sdp: "block" }
    end
  end

  test "discover_peers returns only active friends" do
    Peer.create!(user: @alice, public_key: @bob.public_key, address: "127.0.0.1", port: 9000, last_seen: Time.current)

    subscribe(user_id: @alice.id)

    perform :discover_peers, {}
    payload = transmissions.last

    refute_nil payload, "expected peer list payload"
    assert_equal "peer_list", payload[:type]
    assert_equal 1, payload[:peers].length
    assert_equal @bob.id, payload[:peers].first[:id]
  end
end
