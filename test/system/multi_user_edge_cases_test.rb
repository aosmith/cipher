require "application_system_test_case"

class MultiUserEdgeCasesTest < ApplicationSystemTestCase
  driven_by :selenium, using: :headless_chrome, screen_size: [1400, 1400]

  setup do
    @alice = users(:alice)
    @bob = users(:bob)
    @charlie = users(:charlie)
    @david = users(:david)
    
    # Set up friendship network
    unless Friendship.exists?(requester: @alice, addressee: @bob, status: 'accepted')
      @alice.sent_friendships.create!(addressee: @bob, status: 'accepted')
    end
    unless Friendship.exists?(requester: @bob, addressee: @alice, status: 'accepted')
      @bob.sent_friendships.create!(addressee: @alice, status: 'accepted')
    end
    
    unless Friendship.exists?(requester: @alice, addressee: @charlie, status: 'accepted')
      @alice.sent_friendships.create!(addressee: @charlie, status: 'accepted')
    end
    unless Friendship.exists?(requester: @charlie, addressee: @alice, status: 'accepted')
      @charlie.sent_friendships.create!(addressee: @alice, status: 'accepted')
    end
    
    unless Friendship.exists?(requester: @bob, addressee: @david, status: 'accepted')
      @bob.sent_friendships.create!(addressee: @david, status: 'accepted')
    end
    unless Friendship.exists?(requester: @david, addressee: @bob, status: 'accepted')
      @david.sent_friendships.create!(addressee: @bob, status: 'accepted')
    end
  end

  test "handles concurrent sync requests from multiple friends" do
    # Create initial content for Alice
    alice_post = @alice.posts.create!(
      content: "Alice's shared content",
      is_synced: false,
      original_user_id: @alice.id,
      content_hash: Digest::SHA256.hexdigest("Alice's shared content")
    )

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

    # Bob and Charlie simultaneously try to sync with Alice
    using_session "bob" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
      end
    end

    using_session "charlie" do
      login_as @charlie
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
      end
    end

    # Both should complete successfully despite concurrent requests
    using_session "bob" do
      within "[data-friend-sync]" do
        assert_text "✅ Sync completed with Alice", wait: 20
      end
      
      within "[data-posts-feed]" do
        assert_text "Alice's shared content"
        assert_text "Synced from Alice"
      end
    end

    using_session "charlie" do
      within "[data-friend-sync]" do
        assert_text "✅ Sync completed with Alice", wait: 20
      end
      
      within "[data-posts-feed]" do
        assert_text "Alice's shared content"
        assert_text "Synced from Alice"
      end
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        # Alice should show successful syncs with both friends
        assert_text "✅ Last sync with Bob:", wait: 10
        assert_text "✅ Last sync with Charlie:", wait: 10
        assert_text "Active connections: 2", wait: 5
      end
    end
  end

  test "gracefully handles friend disconnection during sync" do
    # Alice creates content to share
    @alice.posts.create!(
      content: "Content to share with Bob",
      is_synced: false,
      original_user_id: @alice.id,
      content_hash: Digest::SHA256.hexdigest("Content to share with Bob")
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

    using_session "bob" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
        
        # Simulate sudden disconnection by closing the tab/browser
        page.execute_script("window.close()")
      end
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        # Alice should detect Bob's disconnection and handle gracefully
        assert_text "⚠️ Bob disconnected during sync", wait: 15
        assert_text "🔄 Cleaning up incomplete sync...", wait: 5
        assert_text "✅ Sync cleanup completed", wait: 10
        
        # Connection count should decrease
        assert_text "Active connections: 0", wait: 5
      end
    end

    # Bob reconnects and should be able to sync successfully
    using_session "bob_reconnected" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
        assert_text "✅ Sync completed with Alice", wait: 15
      end
      
      within "[data-posts-feed]" do
        assert_text "Content to share with Bob"
        assert_text "Synced from Alice"
      end
    end
  end

  test "handles partial sync completion and resume" do
    # Create many posts for a large sync operation
    20.times do |i|
      @alice.posts.create!(
        content: "Alice's post #{i + 1}",
        is_synced: false,
        original_user_id: @alice.id,
        content_hash: Digest::SHA256.hexdigest("Alice's post #{i + 1}")
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
        assert_text "🔄 Syncing with Alice...", wait: 5
        assert_text "📊 Syncing batch 1/2...", wait: 10
        
        # Simulate network interruption during first batch
        page.execute_script("""
          if (window.webrtc && window.webrtc.connections) {
            Object.values(window.webrtc.connections).forEach(conn => {
              if (conn.dataChannel) {
                conn.dataChannel.close();
              }
            });
          }
        """)
        
        assert_text "⚠️ Connection lost during sync", wait: 10
        assert_text "💾 Partial sync saved (10/20 posts)", wait: 5
        assert_text "🔄 Attempting to resume sync...", wait: 5
        
        # Should successfully resume from where it left off
        assert_text "🔄 Resuming from batch 2/2...", wait: 15
        assert_text "✅ Sync completed with Alice", wait: 20
      end
      
      # Verify all posts were synced despite interruption
      within "[data-posts-feed]" do
        assert_text "Alice's post 1"
        assert_text "Alice's post 10" # From first batch
        assert_text "Alice's post 20" # From resumed batch
        assert_text "Showing 20 posts" # All Alice's posts
      end
    end
  end

  test "prevents sync loops and circular dependencies" do
    # Create a scenario where Alice has content from Bob, and Bob tries to sync it back
    original_post = @alice.posts.create!(
      content: "Original Alice content",
      is_synced: false,
      original_user_id: @alice.id,
      content_hash: Digest::SHA256.hexdigest("Original Alice content")
    )

    # Simulate that Bob already has this content synced from Alice
    synced_post = @bob.posts.create!(
      content: "Original Alice content",
      is_synced: true,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
      content_hash: Digest::SHA256.hexdigest("Original Alice content")
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

    using_session "bob" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
        
        # Should detect and prevent sync loop
        assert_text "⚠️ Sync loop detected", wait: 10
        assert_text "🚫 Skipping already synced content", wait: 5
        assert_text "✅ Sync completed (0 new posts)", wait: 10
      end
    end

    using_session "alice" do
      # Now Alice tries to sync with Bob - should also prevent the loop
      within "[data-friend-sync]" do
        assert_text "👥 Friends Available:", wait: 10
        assert_text "Bob (bob@test.com)", wait: 5
        click_on "Sync with Bob"
        assert_text "🔄 Syncing with Bob...", wait: 5
        assert_text "⚠️ Circular sync prevented", wait: 10
        assert_text "✅ Sync completed (0 new posts)", wait: 10
      end
    end
  end

  test "handles network partition and friend discovery conflicts" do
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
      end
      
      # Simulate network partition - Bob loses connection to discovery service
      page.execute_script("""
        if (window.signalingChannel) {
          window.signalingChannel.disconnect();
        }
      """)
      
      within "[data-friend-sync]" do
        assert_text "⚠️ Discovery service disconnected", wait: 10
        assert_text "🔄 Attempting reconnection...", wait: 5
        assert_text "📡 Using fallback peer discovery", wait: 10
        
        # Should still be able to connect via direct WebRTC
        assert_text "🔗 Direct connection available", wait: 15
        click_on "Connect Directly to Alice"
        assert_text "🔄 Establishing direct connection...", wait: 5
        assert_text "✅ Direct connection established", wait: 15
      end
    end

    using_session "charlie" do
      login_as @charlie
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      # Charlie should also handle the network partition gracefully
      within "[data-friend-sync]" do
        # Should detect multiple friends and handle discovery conflicts
        assert_text "👥 Discovering friends...", wait: 10
        assert_text "Alice (alice@test.com)", wait: 15
        assert_text "⚠️ Discovery conflict detected", wait: 5
        assert_text "✅ Resolved via peer priority", wait: 10
      end
    end
  end

  test "manages memory and resource usage with many concurrent friends" do
    # Create additional test users to simulate high load
    many_friends = 5.times.map do |i|
      friend = User.create!(
        username: "friend#{i}",
        display_name: "Friend #{i}",
        public_key: "friend_key_#{i}"
      )
      
      # Make Alice friends with everyone
      @alice.friendships.create!(friend: friend)
      friend.friendships.create!(friend: @alice)
      
      friend
    end

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

    # All friends come online simultaneously
    friend_sessions = many_friends.map.with_index do |friend, index|
      session_name = "friend_#{index}"
      
      using_session session_name do
        login_as friend
        visit root_path
        
        click_on "💾 Local Hosting"
        assert_text "🟢 Hosting Active", wait: 5
        
        within "[data-friend-sync]" do
          assert_text "Alice (alice@test.com)", wait: 15
          # Stagger the sync requests slightly
          sleep(index * 0.5)
          click_on "Sync with Alice"
          assert_text "🔄 Syncing with Alice...", wait: 5
        end
      end
      
      session_name
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        # Alice should manage multiple concurrent connections efficiently
        assert_text "Active connections: 5", wait: 20
        assert_text "🔧 Connection pool: 5/10", wait: 5
        assert_text "💾 Memory usage: Normal", wait: 5
        
        # Should complete all syncs successfully
        friend_sessions.each_with_index do |session, index|
          assert_text "✅ Last sync with Friend #{index}:", wait: 30
        end
        
        assert_text "📊 Total syncs completed: 5", wait: 5
        assert_text "⚡ All connections stable", wait: 5
      end
    end

    # Verify resource cleanup when friends disconnect
    friend_sessions.each do |session|
      using_session session do
        # Friends disconnect by closing their apps
        page.execute_script("window.close()")
      end
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        assert_text "Active connections: 0", wait: 20
        assert_text "💾 Memory usage: Optimized", wait: 5
        assert_text "🧹 Connection cleanup completed", wait: 5
      end
    end
  end

  test "handles conflicting sync operations and data consistency" do
    # Alice and Bob both modify the same type of content simultaneously
    using_session "alice" do
      login_as @alice
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      # Alice creates a post
      click_on "New Post"
      fill_in "Content", with: "Alice's version of shared idea"
      click_on "Create Post"
      assert_text "Post created successfully"
      
      within "[data-friend-sync]" do
        click_on "Announce Availability"
      end
    end

    using_session "bob" do
      login_as @bob
      visit root_path
      
      click_on "💾 Local Hosting"
      assert_text "🟢 Hosting Active", wait: 5
      
      # Bob creates similar content at the same time
      click_on "New Post"
      fill_in "Content", with: "Bob's version of shared idea"
      click_on "Create Post"
      assert_text "Post created successfully"
      
      within "[data-friend-sync]" do
        assert_text "Alice (alice@test.com)", wait: 10
        click_on "Sync with Alice"
        assert_text "🔄 Syncing with Alice...", wait: 5
      end
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        assert_text "Bob (bob@test.com)", wait: 10
        click_on "Sync with Bob"
        assert_text "🔄 Syncing with Bob...", wait: 5
      end
    end

    # Both should complete sync and have both versions
    using_session "bob" do
      within "[data-friend-sync]" do
        assert_text "✅ Sync completed with Alice", wait: 15
      end
      
      within "[data-posts-feed]" do
        assert_text "Alice's version of shared idea"
        assert_text "Bob's version of shared idea"
        assert_text "Synced from Alice"
        # Should maintain both versions, not overwrite
      end
    end

    using_session "alice" do
      within "[data-friend-sync]" do
        assert_text "✅ Sync completed with Bob", wait: 15
      end
      
      within "[data-posts-feed]" do
        assert_text "Alice's version of shared idea"
        assert_text "Bob's version of shared idea" 
        assert_text "Synced from Bob"
        # Both versions preserved with proper attribution
      end
    end
  end

  private

  def login_as(user)
    # For system tests, use API-based login
    visit root_path
    
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
    
    sleep 0.5
  end
end