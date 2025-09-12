require "test_helper"

class Api::V1::SyncControllerTest < ActionDispatch::IntegrationTest
  setup do
    @alice = users(:alice)
    @bob = users(:bob)
    @charlie = users(:charlie)
    
    # Ensure friendship between Alice and Bob exists (may already be in fixtures)
    unless Friendship.exists?(requester: @alice, addressee: @bob, status: 'accepted')
      @alice.sent_friendships.create!(addressee: @bob, status: 'accepted')
    end
    unless Friendship.exists?(requester: @bob, addressee: @alice, status: 'accepted')
      @bob.sent_friendships.create!(addressee: @alice, status: 'accepted')
    end
    
    # Create Alice's posts on her server
    @alice_post = @alice.posts.create!(
      content: "Hello from Alice's server!",
      is_synced: false,
      original_user_id: @alice.id,
      content_hash: Digest::SHA256.hexdigest("Hello from Alice's server!")
    )
    
    # Create Bob's posts on his server
    @bob_post = @bob.posts.create!(
      content: "Hello from Bob's server!",
      is_synced: false,
      original_user_id: @bob.id,
      content_hash: Digest::SHA256.hexdigest("Hello from Bob's server!")
    )
    
    # Setup two separate server sessions
    @alice_session = ActionDispatch::Integration::Session.new(Rails.application)
    @bob_session = ActionDispatch::Integration::Session.new(Rails.application)
  end

  test "should sync data between Alice's server and Bob's server" do
    # Step 1: Bob's server requests sync data from Alice's server
    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.get "/api/v1/sync_data", params: { friend_id: @bob.id }
    
    assert_equal 200, @alice_session.response.status
    alice_sync_data = JSON.parse(@alice_session.response.body)
    
    assert alice_sync_data.key?('posts')
    assert alice_sync_data.key?('user_id')
    assert_equal @alice.id, alice_sync_data['user_id']
    
    # Should include Alice's posts that Bob can sync
    post_contents = alice_sync_data['posts'].map { |p| p['content'] }
    assert_includes post_contents, "Hello from Alice's server!"
    
    # Step 2: Bob's server accepts the sync data from Alice
    @bob_session.post "/api/v1/login", params: { user_id: @bob.id }
    @bob_session.post "/api/v1/accept_sync", params: { 
      friend_id: @alice.id, 
      sync_data: alice_sync_data 
    }
    
    assert_equal 200, @bob_session.response.status
    bob_response = JSON.parse(@bob_session.response.body)
    assert bob_response['success']
    assert_equal 1, bob_response['synced_posts_count']
    
    # Step 3: Verify Alice's post now exists on Bob's server as synced
    synced_post = Post.find_by(
      user: @bob,
      content_encrypted: "Hello from Alice's server!",
      is_synced: true,
      synced_from_user_id: @alice.id,
      original_user_id: @alice.id
    )
    assert synced_post, "Alice's post should be synced to Bob's server"
  end

  test "should reject sync data request from non-friend" do
    # Charlie's server tries to sync with Alice, but they're not friends
    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.get api_v1_sync_data_path, params: { friend_id: @charlie.id }
    
    assert_equal 403, @alice_session.response.status
    response_data = JSON.parse(@alice_session.response.body)
    assert_equal "Access denied: Can only sync with friends or friends of friends", response_data['error']
  end

  test "should reject sync data request without authentication" do
    # Unauthenticated request to Alice's server
    @alice_session.get api_v1_sync_data_path, params: { friend_id: @bob.id }
    
    assert_equal 401, @alice_session.response.status
  end

  test "bidirectional sync between Alice and Bob servers" do
    # Step 1: Alice's server shares data with Bob
    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.get api_v1_sync_data_path, params: { friend_id: @bob.id }
    
    alice_sync_data = JSON.parse(@alice_session.response.body)
    
    @bob_session.post "/api/v1/login", params: { user_id: @bob.id }
    @bob_session.post api_v1_accept_sync_path, params: { 
      friend_id: @alice.id, 
      sync_data: alice_sync_data 
    }
    
    assert_equal 200, @bob_session.response.status
    
    # Step 2: Bob's server shares data with Alice
    @bob_session.get api_v1_sync_data_path, params: { friend_id: @alice.id }
    
    bob_sync_data = JSON.parse(@bob_session.response.body)
    
    @alice_session.post api_v1_accept_sync_path, params: { 
      friend_id: @bob.id, 
      sync_data: bob_sync_data 
    }
    
    assert_equal 200, @alice_session.response.status
    alice_response = JSON.parse(@alice_session.response.body)
    assert alice_response['success']
    
    # Step 3: Verify both servers have each other's content
    # Bob's post should exist on Alice's server as synced
    bob_post_on_alice = Post.find_by(
      user: @alice,
      content_encrypted: "Hello from Bob's server!",
      is_synced: true,
      synced_from_user_id: @bob.id,
      original_user_id: @bob.id
    )
    assert bob_post_on_alice, "Bob's post should be synced to Alice's server"
    
    # Alice's post should exist on Bob's server as synced
    alice_post_on_bob = Post.find_by(
      user: @bob,
      content_encrypted: "Hello from Alice's server!",
      is_synced: true,
      synced_from_user_id: @alice.id,
      original_user_id: @alice.id
    )
    assert alice_post_on_bob, "Alice's post should be synced to Bob's server"
  end

  test "should reject sync data from non-friend server" do
    # Charlie's server tries to send data to Bob's server
    charlie_sync_data = {
      posts: [
        {
          content: "Malicious post from Charlie",
          original_user_id: @charlie.id,
          content_hash: Digest::SHA256.hexdigest("Malicious post from Charlie"),
          created_at: 1.hour.ago.iso8601
        }
      ],
      user_id: @charlie.id
    }

    @bob_session.post "/api/v1/login", params: { user_id: @bob.id }
    @bob_session.post api_v1_accept_sync_path, params: { 
      friend_id: @charlie.id, 
      sync_data: charlie_sync_data 
    }
    
    assert_equal 403, @bob_session.response.status
    response_data = JSON.parse(@bob_session.response.body)
    assert_equal "Access denied: Can only sync with friends or friends of friends", response_data['error']
    
    # Verify no malicious data was synced
    malicious_post = Post.find_by(content_encrypted: "Malicious post from Charlie")
    assert_nil malicious_post, "Malicious post should not be synced"
  end

  test "should prevent private key leakage in sync data" do
    # Bob's server attempts to send malicious data with private keys to Alice
    malicious_sync_data = {
      posts: [
        {
          content: "Normal content",
          original_user_id: @bob.id,
          content_hash: Digest::SHA256.hexdigest("Normal content"),
          created_at: 1.hour.ago.iso8601,
          private_key: "malicious_private_key_data",
          secret_key: "another_secret"
        }
      ],
      user_id: @bob.id
    }

    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.post api_v1_accept_sync_path, params: { 
      friend_id: @bob.id, 
      sync_data: malicious_sync_data 
    }
    
    assert_equal 400, @alice_session.response.status
    response_data = JSON.parse(@alice_session.response.body)
    assert_match(/SECURITY VIOLATION.*Private key data detected/, response_data['error'])
  end


  # Content size limits removed - dealing with spam later

  test "should prevent duplicate content sync between servers" do
    # First, sync Alice's existing post to Bob's server
    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.get api_v1_sync_data_path, params: { friend_id: @bob.id }
    
    alice_sync_data = JSON.parse(@alice_session.response.body)
    
    @bob_session.post "/api/v1/login", params: { user_id: @bob.id }
    @bob_session.post api_v1_accept_sync_path, params: { 
      friend_id: @alice.id, 
      sync_data: alice_sync_data 
    }
    
    # Now try to sync the same content again
    @bob_session.post api_v1_accept_sync_path, params: { 
      friend_id: @alice.id, 
      sync_data: alice_sync_data 
    }
    
    assert_equal 200, @bob_session.response.status
    response_data = JSON.parse(@bob_session.response.body)
    assert_equal 0, response_data['synced_posts_count']
    assert_includes response_data['skipped_reasons'], 'Duplicate content detected'
  end

  test "should validate sync data structure" do
    # Bob's server sends invalid sync data structure to Alice
    invalid_sync_data = {
      invalid_field: "test",
      user_id: @bob.id
    }

    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.post api_v1_accept_sync_path, params: { 
      friend_id: @bob.id, 
      sync_data: invalid_sync_data 
    }
    
    assert_equal 400, @alice_session.response.status
    response_data = JSON.parse(@alice_session.response.body)
    assert_match(/Invalid sync data format/, response_data['error'])
  end

  test "should enforce bulk sync limits" do
    # Bob's server tries to sync too many posts at once to Alice
    large_posts_array = 101.times.map do |i|
      {
        content: "Bulk post #{i}",
        original_user_id: @bob.id,
        content_hash: Digest::SHA256.hexdigest("Bulk post #{i}"),
        created_at: 1.hour.ago.iso8601
      }
    end
    
    sync_data = {
      posts: large_posts_array,
      user_id: @bob.id
    }

    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.post api_v1_accept_sync_path, params: { 
      friend_id: @bob.id, 
      sync_data: sync_data 
    }
    
    assert_equal 400, @alice_session.response.status
    response_data = JSON.parse(@alice_session.response.body)
    assert_match(/Too many posts in sync batch/, response_data['error'])
  end

  test "should return appropriate sync metadata" do
    @alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    @alice_session.get api_v1_sync_data_path, params: { friend_id: @bob.id }
    
    assert_equal 200, @alice_session.response.status
    response_data = JSON.parse(@alice_session.response.body)
    
    assert response_data.key?('sync_metadata')
    metadata = response_data['sync_metadata']
    
    assert metadata.key?('last_sync_time')
    assert metadata.key?('total_posts')
    assert metadata.key?('user_public_key')
    assert_equal @alice.id, metadata['user_id']
  end
end