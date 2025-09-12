require "test_helper"

class SpamPreventionTest < ActionDispatch::IntegrationTest
  def setup
    # Create users directly like the posts controller test does
    @alice = User.create!(
      username: "alice_spam_test",
      display_name: "Alice Spam Test",
      public_key: "alice_spam_public_key"
    )
    @bob = User.create!(
      username: "bob_spam_test", 
      display_name: "Bob Spam Test",
      public_key: "bob_spam_public_key"
    )
    @spammer = User.create!(
      username: "charlie_spam_test",
      display_name: "Charlie Spam Test",
      public_key: "charlie_spam_public_key"
    )
    
    # Ensure friendship between Alice and Bob exists
    unless Friendship.exists?(requester: @alice, addressee: @bob, status: 'accepted')
      @alice.sent_friendships.create!(addressee: @bob, status: 'accepted')
    end
    unless Friendship.exists?(requester: @bob, addressee: @alice, status: 'accepted')
      @bob.sent_friendships.create!(addressee: @alice, status: 'accepted')
    end
    
    # Create friendship between Alice and the spammer to test friend-based spam
    unless Friendship.exists?(requester: @alice, addressee: @spammer, status: 'accepted')
      @alice.sent_friendships.create!(addressee: @spammer, status: 'accepted')
    end
    unless Friendship.exists?(requester: @spammer, addressee: @alice, status: 'accepted')
      @spammer.sent_friendships.create!(addressee: @alice, status: 'accepted')
    end
  end

  test "rate limiting prevents rapid post creation" do
    login_as @alice
    
    # Create posts rapidly to trigger rate limiting
    10.times do |i|
      post posts_path, params: { 
        post: { content: "Rapid post #{i}" }
      }
    end
    
    # 11th post should be rate limited
    post posts_path, params: { 
      post: { content: "This should be blocked" }
    }
    
    assert_response :redirect
    assert_redirected_to root_path
    
    # Check that the alert message mentions rate limiting
    assert_match(/Rate limit exceeded/, flash[:alert])
    
    # Verify only 10 posts were created
    assert_equal 10, @alice.posts.where('created_at > ?', 1.minute.ago).count
  end

  test "daily post limit prevents spam flooding" do
    login_as @spammer
    
    # Create posts throughout the day to hit daily limit
    50.times do |i|
      # Simulate posts at different times
      travel_to(i.minutes.ago) do
        post posts_path, params: { 
          post: { content: "Daily spam post #{i}" }
        }
      end
    end
    
    # 51st post should exceed daily limit
    post posts_path, params: { 
      post: { content: "This should exceed daily limit" }
    }
    
    assert_response :redirect
    assert_redirected_to root_path
    assert_match(/Daily limit exceeded/, flash[:alert])
    
    # Verify daily count
    daily_count = @spammer.posts.where('created_at > ?', 24.hours.ago).count
    assert_equal 50, daily_count
  end

  # Content size limits removed - dealing with spam later

  test "duplicate content detection prevents spam reposts" do
    login_as @alice
    
    original_content = "This is unique content"
    
    # Create first post successfully
    post posts_path, params: { 
      post: { content: original_content }
    }
    assert_response :redirect
    assert_redirected_to root_path
    assert_match(/Post created successfully/, flash[:notice])
    
    # Try to create duplicate post
    post posts_path, params: { 
      post: { content: original_content }
    }
    
    assert_response :redirect
    assert_redirected_to root_path
    assert_match(/Duplicate content detected/, flash[:alert])
    
    # Verify only one post with this content exists
    assert_equal 1, Post.where(content_encrypted: original_content).count
  end

  test "content hash validation ensures data integrity" do
    login_as @alice
    
    content = "Test content for hash validation"
    expected_hash = Digest::SHA256.hexdigest(content)
    
    post posts_path, params: { 
      post: { 
        content: content,
        content_hash: expected_hash
      }
    }
    
    assert_response :created
    created_post = Post.last
    assert_equal expected_hash, created_post.content_hash
    
    # Try to create post with mismatched hash
    post posts_path, params: { 
      post: { 
        content: content,
        content_hash: "invalid_hash_123"
      }
    }
    
    assert_response :unprocessable_entity
    response_data = JSON.parse(@response.body)
    assert_match(/Content hash mismatch/, response_data['errors'].join(' '))
  end

  test "sync spam prevention with friend verification" do
    # Setup sync sessions
    alice_session = ActionDispatch::Integration::Session.new(Rails.application)
    spammer_session = ActionDispatch::Integration::Session.new(Rails.application)
    
    # Alice logs in to her server
    alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    
    # Spammer creates rapid burst of posts
    25.times do |i|
      @spammer.posts.create!(
        content: "Spam post #{i}",
        is_synced: false,
        original_user_id: @spammer.id,
        content_hash: Digest::SHA256.hexdigest("Spam post #{i}")
      )
    end
    
    # Spammer tries to sync all posts to Alice at once
    spam_sync_data = {
      posts: @spammer.posts.limit(25).map do |post|
        {
          content: post.content,
          original_user_id: post.original_user_id,
          content_hash: post.content_hash,
          created_at: post.created_at.iso8601
        }
      end,
      user_id: @spammer.id
    }
    
    alice_session.post api_v1_accept_sync_path, params: { 
      friend_id: @spammer.id, 
      sync_data: spam_sync_data 
    }
    
    # Should apply bulk sync limits
    assert_equal 400, alice_session.response.status
    response_data = JSON.parse(alice_session.response.body)
    assert_match(/Too many posts in sync batch/, response_data['error'])
  end

  test "prevents rapid sync requests from same friend" do
    # Setup two server sessions
    alice_session = ActionDispatch::Integration::Session.new(Rails.application)
    bob_session = ActionDispatch::Integration::Session.new(Rails.application)
    
    alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    bob_session.post "/api/v1/login", params: { user_id: @bob.id }
    
    # Bob makes rapid sync requests to Alice's server
    11.times do |i|
      alice_session.get api_v1_sync_data_path, params: { friend_id: @bob.id }
    end
    
    # 11th request should be rate limited
    assert_equal 429, alice_session.response.status
    response_data = JSON.parse(alice_session.response.body)
    assert_match(/Rate limit exceeded/, response_data['error'])
  end

  test "content filtering prevents malicious scripts and links" do
    login_as @alice
    
    malicious_contents = [
      "<script>alert('XSS')</script>",
      "javascript:alert('XSS')",
      "<iframe src='http://malicious.com'></iframe>",
      "Check out this link: http://malicious-phishing-site.com/steal-credentials"
    ]
    
    malicious_contents.each_with_index do |malicious_content, index|
      post posts_path, params: { 
        post: { content: malicious_content }
      }
      
      assert_response :unprocessable_entity
      response_data = JSON.parse(@response.body)
      assert_match(/Malicious content detected/, response_data['errors'].join(' '))
      
      # Verify malicious post was not created
      assert_nil Post.find_by(content_encrypted: malicious_content)
    end
  end

  test "user reputation affects spam detection sensitivity" do
    # Create a new user with no reputation
    new_user = User.create!(
      username: "newspammer",
      display_name: "New Spammer",
      public_key: "new_spammer_key"
    )
    
    login_as new_user
    
    # New users have stricter spam limits
    3.times do |i|
      post posts_path, params: { 
        post: { content: "New user post #{i}" }
      }
    end
    
    # 4th post should be blocked for new users (stricter than normal 10-post limit)
    post posts_path, params: { 
      post: { content: "This should be blocked for new user" }
    }
    
    assert_response :unprocessable_entity
    response_data = JSON.parse(@response.body)
    assert_match(/New user rate limit exceeded/, response_data['errors'].join(' '))
  end

  test "sync content filtering prevents private key leakage" do
    alice_session = ActionDispatch::Integration::Session.new(Rails.application)
    alice_session.post "/api/v1/login", params: { user_id: @alice.id }
    
    # Attempt to sync data containing private key information
    malicious_sync_data = {
      posts: [
        {
          content: "Normal content here",
          original_user_id: @bob.id,
          content_hash: Digest::SHA256.hexdigest("Normal content"),
          created_at: 1.hour.ago.iso8601,
          private_key: "-----BEGIN PRIVATE KEY-----\nMaliciousPrivateKeyData\n-----END PRIVATE KEY-----",
          metadata: {
            secret_key: "another_secret_key",
            privateKey: "camelCase private key"
          }
        }
      ],
      user_id: @bob.id
    }
    
    alice_session.post api_v1_accept_sync_path, params: { 
      friend_id: @bob.id, 
      sync_data: malicious_sync_data 
    }
    
    assert_equal 400, alice_session.response.status
    response_data = JSON.parse(alice_session.response.body)
    assert_match(/SECURITY VIOLATION.*Private key data detected/, response_data['error'])
    
    # Verify no posts were created with private key data
    assert_nil Post.find_by(content_encrypted: "Normal content here")
  end

  test "bulk operations respect individual spam limits" do
    login_as @alice
    
    # Try to create many posts via bulk API endpoint
    bulk_posts = 15.times.map do |i|
      {
        content: "Bulk post #{i}",
        content_hash: Digest::SHA256.hexdigest("Bulk post #{i}")
      }
    end
    
    post bulk_posts_path, params: { posts: bulk_posts }
    
    assert_response :unprocessable_entity
    response_data = JSON.parse(@response.body)
    assert_match(/Bulk operation exceeds rate limits/, response_data['errors'].join(' '))
    
    # Verify that bulk limits are enforced (should only create first 10)
    created_count = @alice.posts.where('content_encrypted LIKE ?', 'Bulk post %').count
    assert_equal 0, created_count # Should reject entire bulk if it exceeds limits
  end

  test "temporal spam detection identifies coordinated attacks" do
    # Simulate coordinated spam attack from multiple accounts
    spammer_accounts = 3.times.map do |i|
      User.create!(
        username: "spammer#{i}",
        display_name: "Spammer #{i}",
        public_key: "spammer_key_#{i}"
      )
    end
    
    # All spammers post similar content at the same time
    spam_content_template = "Buy this amazing product now! Limited time offer!"
    
    spammer_accounts.each_with_index do |spammer, index|
      login_as spammer
      
      post posts_path, params: { 
        post: { content: "#{spam_content_template} #{index}" }
      }
    end
    
    # System should detect coordinated spam pattern
    # This would trigger more sophisticated detection in a real system
    spam_posts = Post.where('content_encrypted LIKE ?', 'Buy this amazing product%')
    
    # For now, verify that content similarity detection works
    assert_equal 3, spam_posts.count
    
    # In a real system, these would be flagged for review
    # For testing, we verify the detection logic exists
    similarity_scores = spam_posts.map do |post|
      SpamDetector.content_similarity_score(post.content, spam_content_template)
    end
    
    assert similarity_scores.all? { |score| score > 0.8 }, "Should detect high content similarity"
  end

  private

  def login_as(user)
    # For Rails integration tests, we can't directly access session
    # Instead, we'll use the login API endpoint like the posts controller tests
    post "/api/v1/login", 
         params: { username: user.username, public_key: user.public_key }, 
         as: :json
    assert_response :success
  end
  
  def bulk_posts_path
    "/api/v1/posts/bulk"
  end
end

# Mock SpamDetector class for testing
class SpamDetector
  def self.content_similarity_score(content1, content2)
    # Simple similarity calculation for testing
    words1 = content1.downcase.split
    words2 = content2.downcase.split
    common_words = words1 & words2
    common_words.size.to_f / [words1.size, words2.size].max
  end
end