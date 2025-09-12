require "application_system_test_case"

class FriendBasedSyncTest < ApplicationSystemTestCase
  driven_by :selenium, using: :headless_chrome, screen_size: [1400, 1400]

  setup do
    @alice = users(:alice)
    @bob = users(:bob)
    @charlie = users(:charlie)
    
    # Set up friendship between Alice and Bob
    unless Friendship.exists?(requester: @alice, addressee: @bob, status: 'accepted')
      @alice.sent_friendships.create!(addressee: @bob, status: 'accepted')
    end
    unless Friendship.exists?(requester: @bob, addressee: @alice, status: 'accepted')
      @bob.sent_friendships.create!(addressee: @alice, status: 'accepted')
    end
    
    # Create some initial content for each user
    @alice.posts.create!(
      content: "Alice's original post",
      is_synced: false,
      original_user_id: @alice.id,
      content_hash: Digest::SHA256.hexdigest("Alice's original post")
    )
    
    @bob.posts.create!(
      content: "Bob's original post", 
      is_synced: false,
      original_user_id: @bob.id,
      content_hash: Digest::SHA256.hexdigest("Bob's original post")
    )
  end

  test "friends can discover and sync with each other via WebRTC" do
    using_session "alice" do
      login_as @alice
      visit root_path
      
      # Alice starts local hosting to make her content available
      click_on "🌐 Local Hosting"
      
      # Wait for hosting to be enabled
      assert_text "🟢 Hosting Active", wait: 5
      assert_text "Alice's original post"
      
      # Alice should see the friend-based sync controls
      assert_selector "[data-friend-sync]", visible: true
      
      # Alice announces she's available for sync
      within "[data-friend-sync]" do
        click_on "Announce Availability"
        assert_text "📡 Broadcasting to friends...", wait: 3
      end
    end

    using_session "bob" do
      login_as @bob
      visit root_path
      
      # Bob also starts local hosting
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      assert_text "Bob's original post"
      
      # Bob should detect Alice's availability
      within "[data-friend-sync]" do
        assert_text "👥 Friends Available:", wait: 10
        assert_text "Alice (alice@test.com)", wait: 5
        
        # Bob initiates sync with Alice
        click_on "Sync with Alice"
        
        # Wait for sync process to complete
        assert_text "🔄 Syncing with Alice...", wait: 5
        assert_text "✅ Sync completed with Alice", wait: 15
      end
      
      # Verify Bob now has Alice's content as synced posts
      within "[data-posts-feed]" do
        assert_text "Alice's original post"
        assert_text "Synced from Alice"
        assert_text "Bob's original post"
      end
    end

    using_session "alice" do
      # Alice should also have Bob's content after sync
      within "[data-posts-feed]" do
        assert_text "Bob's original post" 
        assert_text "Synced from Bob"
        assert_text "Alice's original post"
      end
      
      # Verify sync status indicators
      within "[data-friend-sync]" do
        assert_text "✅ Last sync with Bob:"
        assert_text "1 post synced"
      end
    end
  end

  test "non-friends cannot sync content" do
    using_session "alice" do
      login_as @alice
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        click_on "Announce Availability"
        assert_text "📡 Broadcasting to friends...", wait: 3
      end
    end

    using_session "charlie" do
      login_as @charlie
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      # Charlie should NOT see Alice in available friends
      within "[data-friend-sync]" do
        assert_no_text "Alice (alice@test.com)", wait: 10
        assert_text "👥 No friends currently available"
      end
      
      # Even if Charlie tries to manually connect, it should fail
      page.evaluate_script("""
        if (window.friendBasedSync) {
          window.friendBasedSync.attemptManualSync('alice@test.com');
        }
      """)
      
      # Should show security rejection
      assert_text "❌ Sync failed: Not authorized", wait: 5
      assert_no_text "Alice's original post"
    end
  end

  test "sync process handles network disconnections gracefully" do
    using_session "alice" do
      login_as @alice
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        click_on "Announce Availability"
        assert_text "📡 Broadcasting to friends...", wait: 3
      end
    end

    using_session "bob" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting" 
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
        
        # Simulate network interruption during sync
        page.evaluate_script("""
          if (window.webrtc && window.webrtc.connections) {
            // Force close all WebRTC connections
            Object.values(window.webrtc.connections).forEach(conn => {
              if (conn.close) conn.close();
            });
          }
        """)
        
        # Should show retry mechanism
        assert_text "⚠️ Connection lost, retrying...", wait: 10
        assert_text "🔄 Attempting reconnection (1/3)", wait: 5
      end
    end
  end

  test "sync content validates security and prevents malicious data" do
    # Create a malicious user who somehow gets Alice's friend data
    malicious_user = User.create!(
      email: "malicious@evil.com",
      name: "Malicious User",
      public_key: "fake_public_key_123"
    )

    using_session "alice" do
      login_as @alice
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        click_on "Announce Availability"
      end
    end

    using_session "malicious" do
      login_as malicious_user
      visit root_path
      
      # Malicious user tries to inject harmful sync data
      malicious_script = <<~JAVASCRIPT
        if (window.friendBasedSync) {
          const maliciousData = {
            posts: [{
              content: 'Malicious post with <script>alert("XSS")</script>',
              original_user_id: #{malicious_user.id},
              content_hash: 'fake_hash',
              private_key: 'stolen_private_key_data',
              created_at: new Date().toISOString()
            }],
            user_id: #{malicious_user.id}
          };
          
          // Try to force sync malicious data
          window.friendBasedSync.processSyncData('alice@test.com', maliciousData);
        }
      JAVASCRIPT
      
      page.evaluate_script(malicious_script)
      
      # Should be rejected by security validation
      assert_text "❌ Security violation detected", wait: 5
      assert_text "Private key data blocked", wait: 3
    end

    using_session "alice" do
      # Alice's content should remain clean
      within "[data-posts-feed]" do
        assert_no_text "Malicious post"
        assert_no_text "<script>"
        assert_text "Alice's original post"
      end
      
      # Security logs should show the attempt
      within "[data-security-log]" do
        assert_text "🛡️ Blocked malicious sync attempt"
        assert_text "Source: malicious@evil.com"
      end
    end
  end

  test "sync handles large content batches with rate limiting" do
    # Create many posts for Bob
    50.times do |i|
      @bob.posts.create!(
        content: "Bob's post number #{i + 1}",
        is_synced: false,
        original_user_id: @bob.id,
        content_hash: Digest::SHA256.hexdigest("Bob's post number #{i + 1}")
      )
    end

    using_session "alice" do
      login_as @alice
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        click_on "Announce Availability"
      end
    end

    using_session "bob" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        
        # Should show batched sync progress
        assert_text "🔄 Syncing batch 1/2...", wait: 5
        assert_text "📊 25/50 posts synced", wait: 10
        assert_text "🔄 Syncing batch 2/2...", wait: 5
        assert_text "📊 50/50 posts synced", wait: 10
        assert_text "✅ Sync completed with Alice", wait: 5
      end
    end

    using_session "alice" do
      # Alice should have received all of Bob's posts in batches
      within "[data-posts-feed]" do
        assert_text "Bob's post number 1"
        assert_text "Bob's post number 25" 
        assert_text "Bob's post number 50"
        assert_text "Showing 51 posts" # Alice's 1 + Bob's 50
      end
      
      within "[data-friend-sync]" do
        assert_text "✅ Last sync with Bob:"
        assert_text "50 posts synced"
      end
    end
  end

  test "real-time sync status updates between friends" do
    using_session "alice" do
      login_as @alice
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
    end

    using_session "bob" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        click_on "Announce Availability"
        assert_text "📡 Broadcasting to friends...", wait: 3
        
        # Alice should see Bob come online
        assert_text "👥 Friends online: 1", wait: 10
        assert_text "🟢 Bob (bob@test.com) - Active", wait: 5
      end
    end

    using_session "bob" do
      within "[data-friend-sync]" do
        # Bob should see Alice's status
        assert_text "👥 Friends Available:", wait: 10
        assert_text "🟢 Alice (alice@test.com) - Ready", wait: 5
        
        # When Bob starts sync, Alice should see the status change
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
      end
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        # Alice should see that Bob is syncing
        assert_text "🔄 Bob - Syncing...", wait: 10
        assert_text "📊 Receiving sync data from Bob", wait: 5
        assert_text "✅ Sync with Bob completed", wait: 15
        assert_text "🟢 Bob (bob@test.com) - Active", wait: 5
      end
    end
  end

  private

  def login_as(user)
    # For system tests, we'll use a direct session approach
    # This simulates the user being logged in without going through the full key derivation
    visit root_path
    
    # Use JavaScript to set the session for testing purposes
    page.execute_script("
      fetch('/api/v1/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-CSRF-Token': document.querySelector('meta[name=\"csrf-token\"]')?.getAttribute('content')
        },
        body: JSON.stringify({
          username: '#{user.username}',
          public_key: '#{user.public_key}'
        })
      });
    ")
    
    # Wait for login to complete
    sleep 0.5
  end
end