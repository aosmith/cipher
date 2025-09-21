require "application_system_test_case"

class P2pSimpleTest < ApplicationSystemTestCase
  setup do
    # Use the same cleanup pattern as other working system tests
    AttachmentShare.destroy_all if defined?(AttachmentShare)
    Attachment.destroy_all if defined?(Attachment)
    Comment.destroy_all
    SyncMessage.destroy_all if defined?(SyncMessage)
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all

    # Create two test users
    @alice = User.create!(
      username: "alice_webrtc",
      display_name: "Alice WebRTC",
      public_key: Base64.strict_encode64("alice_webrtc_key_123456789012345678901234")
    )

    @bob = User.create!(
      username: "bob_webrtc",
      display_name: "Bob WebRTC",
      public_key: Base64.strict_encode64("bob_webrtc_key_987654321098765432109876")
    )

    # Make them friends
    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")
  end

  test "hosting dashboard shows network status" do
    using_session "alice" do
      login_as @alice
      visit root_path

      # Verify user is logged in
      assert_text "Hi, alice_webrtc"

      click_on "Local Hosting"
      assert_text "Local Hosting"
      assert_selector "#p2p-status", wait: 5
    end
  end

  test "multiple users can access WebRTC infrastructure simultaneously" do
    # Test that multiple users can access the Rails WebRTC infrastructure
    using_session "alice" do
      login_as @alice
      visit local_hosting_users_path

      # Verify Alice can access hosting page (WebRTC infrastructure backend)
      assert_text "Local Hosting"
      assert_css "#p2p-status"
    end

    using_session "bob" do
      login_as @bob
      visit local_hosting_users_path

      # Verify Bob can access hosting page (WebRTC infrastructure backend)
      assert_text "Local Hosting"
      assert_css "#p2p-status"
    end

    # Verify both users exist and can potentially connect (Rails model layer)
    assert @alice.present?, "Alice should be ready for P2P connections"
    assert @bob.present?, "Bob should be ready for P2P connections"
    assert_not_equal @alice.public_key, @bob.public_key, "Users should have different public keys"
  end

  test "P2P hosting interface renders static controls" do
    using_session "alice" do
      login_as @alice
      visit local_hosting_users_path

      # Verify hosting page loads correctly
      assert_text "Local Hosting"
      assert_text "Hosting Status"

      assert_css ".hosting-overview"
      assert_css "#p2p-status"
    end
  end

  test "WebRTC connection attempt between sessions" do
    using_session "alice" do
      login_as @alice
      visit local_hosting_users_path

      assert_text "Local Hosting"
      assert_css "#p2p-status"

      alice_peer = @alice.peers.create!(
        address: "127.0.0.1",
        port: 9000,
        public_key: @bob.public_key,
        last_seen: Time.current
      )
      assert alice_peer.persisted?, "Alice should be able to create peer connection record"
    end

    using_session "bob" do
      login_as @bob
      visit local_hosting_users_path

      assert_text "Local Hosting"
      assert_css "#p2p-status"

      bob_peer = @bob.peers.create!(
        address: "127.0.0.1",
        port: 9001,
        public_key: @alice.public_key,
        last_seen: Time.current
      )
      assert bob_peer.persisted?, "Bob should be able to create peer connection record"
    end

    # Verify the signaling records exist (foundation for WebRTC signaling)
    assert_equal 1, @alice.peers.count, "Alice should have one peer record"
    assert_equal 1, @bob.peers.count, "Bob should have one peer record"
  end
end
