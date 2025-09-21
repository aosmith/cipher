require "test_helper"
require "minitest/mock"

class PeerTest < ActiveSupport::TestCase
  setup do
    @user = User.create!(
      username: "peer_test_user",
      display_name: "Peer Test User",
      public_key: "peer_test_public_key"
    )
  end

  test "initiate_webrtc_connection returns client connection details" do
    peer = @user.peers.create!(
      address: "127.0.0.1",
      port: 9000,
      public_key: "peer_public_key"
    )

    custom_stun_servers = [ { urls: "stun:test" } ]
    custom_turn_servers = [ { urls: "turn:test", username: "user", credential: "secret" } ]

    WebRtcConfig.stub :stun_servers, custom_stun_servers do
      WebRtcConfig.stub :turn_servers, custom_turn_servers do
        connection_info = peer.initiate_webrtc_connection

        assert_equal peer.id, connection_info[:peer_id]
        assert_equal custom_stun_servers, connection_info[:stun_servers]
        assert_equal custom_turn_servers, connection_info[:turn_servers]
      end
    end
  end
end
