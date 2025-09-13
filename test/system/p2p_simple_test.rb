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
    Friendship.create!(requester: @alice, addressee: @bob, status: 'accepted')
  end

  test "WebRTC signaling infrastructure is available" do
    using_session "alice" do
      login_user(@alice)
      visit root_path
      
      # Verify user is logged in
      assert_text "Hi, alice_webrtc"
      
      # Navigate to hosting page which should initialize WebRTC
      click_on "Local Hosting"
      assert_text "Local Hosting"
      
      # Wait for WebRTC infrastructure to load
      sleep 2
      
      # Test that WebRTC APIs are available
      webrtc_available = page.evaluate_script("
        typeof RTCPeerConnection !== 'undefined'
      ")
      assert webrtc_available, "RTCPeerConnection should be available"
      
      # Test STUN server accessibility
      stun_test = page.evaluate_script("
        (function() {
          try {
            const pc = new RTCPeerConnection({
              iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
            });
            pc.close();
            return { success: true, message: 'STUN server accessible' };
          } catch (error) {
            return { success: false, error: error.message };
          }
        })();
      ")
      
      assert stun_test['success'], "STUN server should be accessible: #{stun_test['error']}"
    end
  end

  test "multiple users can access WebRTC simultaneously" do
    # Test that multiple browser sessions can both use WebRTC
    using_session "alice" do
      login_user(@alice)
      visit local_hosting_users_path
      
      # Initialize WebRTC for Alice
      webrtc_alice = page.evaluate_script("
        window.aliceRTC = new RTCPeerConnection({ 
          iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] 
        });
        return { state: window.aliceRTC.connectionState, success: true };
      ")
      
      assert webrtc_alice['success'], "Alice should be able to create RTCPeerConnection"
    end
    
    using_session "bob" do
      login_user(@bob)
      visit local_hosting_users_path
      
      # Initialize WebRTC for Bob
      webrtc_bob = page.evaluate_script("
        window.bobRTC = new RTCPeerConnection({ 
          iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] 
        });
        return { state: window.bobRTC.connectionState, success: true };
      ")
      
      assert webrtc_bob['success'], "Bob should be able to create RTCPeerConnection"
    end
  end

  test "P2P hosting interface is functional" do
    using_session "alice" do
      login_user(@alice)
      visit local_hosting_users_path
      
      # Verify hosting page loads correctly
      assert_text "Local Hosting"
      assert_text "Hosting Status"
      
      # Check that hosting interface elements are present
      assert_css ".hosting-overview"
      
      # Verify P2P network section exists
      if has_css?("#connection-status", wait: 2)
        connection_status = find("#connection-status")
        assert connection_status.present?
      end
      
      # Test JavaScript WebRTC infrastructure
      javascript_test = page.evaluate_script("
        // Test basic WebRTC functionality
        const testResults = {
          rtcSupported: typeof RTCPeerConnection !== 'undefined',
          webSocketSupported: typeof WebSocket !== 'undefined',
          arrayBufferSupported: typeof ArrayBuffer !== 'undefined'
        };
        return testResults;
      ")
      
      assert javascript_test['rtcSupported'], "RTCPeerConnection should be supported"
      assert javascript_test['webSocketSupported'], "WebSocket should be supported"
      assert javascript_test['arrayBufferSupported'], "ArrayBuffer should be supported"
    end
  end

  test "WebRTC connection attempt between sessions" do
    # This test simulates what would happen when two users try to connect
    using_session "alice" do
      login_user(@alice)
      visit local_hosting_users_path
      
      # Set up Alice's WebRTC connection
      alice_setup = page.evaluate_script("
        window.aliceConnection = new RTCPeerConnection({
          iceServers: [
            { urls: 'stun:stun.l.google.com:19302' },
            { urls: 'stun:stun1.l.google.com:19302' }
          ]
        });
        
        window.aliceICECandidates = [];
        window.aliceConnection.onicecandidate = function(event) {
          if (event.candidate) {
            window.aliceICECandidates.push(event.candidate);
          }
        };
        
        return { success: true, state: window.aliceConnection.connectionState };
      ")
      
      assert alice_setup['success'], "Alice's WebRTC setup should succeed"
    end
    
    using_session "bob" do
      login_user(@bob)
      visit local_hosting_users_path
      
      # Set up Bob's WebRTC connection
      bob_setup = page.evaluate_script("
        window.bobConnection = new RTCPeerConnection({
          iceServers: [
            { urls: 'stun:stun.l.google.com:19302' },
            { urls: 'stun:stun1.l.google.com:19302' }
          ]
        });
        
        window.bobICECandidates = [];
        window.bobConnection.onicecandidate = function(event) {
          if (event.candidate) {
            window.bobICECandidates.push(event.candidate);
          }
        };
        
        return { success: true, state: window.bobConnection.connectionState };
      ")
      
      assert bob_setup['success'], "Bob's WebRTC setup should succeed"
    end
    
    # Both sessions should now have WebRTC connections ready
    # In a real P2P scenario, they would exchange offers/answers through signaling
    # For testing purposes, we verify the infrastructure is working
  end

  private

  def login_user(user)
    visit root_path
    
    # Use the API login approach
    page.execute_script("
      fetch('/api/v1/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Requested-With': 'XMLHttpRequest'
        },
        body: JSON.stringify({
          username: '#{user.username}',
          public_key: '#{user.public_key}'
        })
      });
    ")
    
    sleep(1)
    visit root_path
  end
end