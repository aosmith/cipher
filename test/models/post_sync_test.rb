require "test_helper"

class PostSyncTest < ActiveSupport::TestCase
  setup do
    @alice = users(:alice)
    @bob = users(:bob)
    
    # Ensure friendship between Alice and Bob exists (may already be in fixtures)
    unless Friendship.exists?(requester: @alice, addressee: @bob, status: 'accepted')
      @alice.sent_friendships.create!(addressee: @bob, status: 'accepted')
    end
    unless Friendship.exists?(requester: @bob, addressee: @alice, status: 'accepted')
      @bob.sent_friendships.create!(addressee: @alice, status: 'accepted')
    end
  end

  test "post sync fields are properly set on creation" do
    post = @alice.posts.create!(
      content: "Test content",
      is_synced: false,
      original_user_id: @alice.id
    )
    
    assert_not post.is_synced
    assert_equal @alice.id, post.original_user_id
    assert_nil post.synced_from_user_id
    assert_nil post.synced_at
    assert_not_nil post.content_hash
  end

  test "synced post is properly attributed" do
    # Create original post by Alice
    original_post = @alice.posts.create!(
      content: "Alice's original content",
      is_synced: false,
      original_user_id: @alice.id
    )
    
    # Create synced version on Bob's server
    synced_post = @bob.posts.create!(
      content: "Alice's original content",
      is_synced: true,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
      synced_at: Time.current,
      content_hash: Digest::SHA256.hexdigest("Alice's original content")
    )
    
    assert synced_post.is_synced
    assert_equal @alice.id, synced_post.original_user_id
    assert_equal @alice.id, synced_post.synced_from_user_id
    assert_not_nil synced_post.synced_at
    assert_equal @bob, synced_post.user
    assert_not_nil synced_post.content_hash
  end

  test "original_user relationship works correctly" do
    synced_post = @bob.posts.create!(
      content: "Content from Alice",
      is_synced: true,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
      synced_at: Time.current
    )
    
    assert_equal @alice, synced_post.original_user
    assert_equal @alice, synced_post.synced_from_user
    assert_equal @bob, synced_post.user
  end

  test "content hash validation ensures data integrity" do
    post = Post.new(
      user: @alice,
      content: "Test content",
      is_synced: false,
      original_user_id: @alice.id,
      content_hash: "invalid_hash"
    )
    
    assert_not post.valid?
    assert_includes post.errors.full_messages, "Content hash mismatch detected"
  end

  test "sync validation prevents invalid states" do
    # Can't have synced_from_user without is_synced being true
    post = Post.new(
      user: @bob,
      content: "Test content",
      is_synced: false,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
    )
    
    assert_not post.valid?
    assert_includes post.errors.full_messages, "Synced from user can only be set if post is synced"
  end

  test "original posts scope returns non-synced posts only" do
    # Create original post
    original = @alice.posts.create!(
      content: "Original content",
      is_synced: false,
      original_user_id: @alice.id,
    )
    
    # Create synced post
    synced = @bob.posts.create!(
      content: "Synced content",
      is_synced: true,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
      synced_at: Time.current,
    )
    
    original_posts = Post.original_posts
    assert_includes original_posts, original
    assert_not_includes original_posts, synced
  end

  test "synced posts scope returns synced posts only" do
    # Create original post
    original = @alice.posts.create!(
      content: "Original content",
      is_synced: false,
      original_user_id: @alice.id,
    )
    
    # Create synced post
    synced = @bob.posts.create!(
      content: "Synced content",
      is_synced: true,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
      synced_at: Time.current,
    )
    
    synced_posts = Post.synced_posts
    assert_includes synced_posts, synced
    assert_not_includes synced_posts, original
  end

  test "spam prevention validations work correctly" do
    # Test rate limiting
    10.times do |i|
      @alice.posts.create!(
        content: "Rate limit test #{i}",
        is_synced: false,
        original_user_id: @alice.id
      )
    end
    
    # 11th post should fail rate limit
    over_limit_post = Post.new(
      user: @alice,
      content: "This should be rate limited",
      is_synced: false,
      original_user_id: @alice.id,
    )
    
    assert_not over_limit_post.valid?
    assert_includes over_limit_post.errors.full_messages, "Rate limit exceeded: Maximum 10 posts per hour"
  end

  test "content size validation prevents oversized posts" do
    large_content = "A" * 10001 # Exceeds 10KB limit
    
    post = Post.new(
      user: @alice,
      content: large_content,
      is_synced: false,
      original_user_id: @alice.id,
    )
    
    assert_not post.valid?
    assert_includes post.errors.full_messages, "Content too large: Maximum 10KB allowed"
  end

  test "duplicate content prevention works" do
    content = "Duplicate test content"
    
    # Create first post
    first_post = @alice.posts.create!(
      content: content,
      is_synced: false,
      original_user_id: @alice.id,
    )
    
    assert first_post.persisted?
    
    # Try to create duplicate
    duplicate_post = Post.new(
      user: @alice,
      content: content,
      is_synced: false,
      original_user_id: @alice.id,
    )
    
    assert_not duplicate_post.valid?
    assert_includes duplicate_post.errors.full_messages, "Duplicate content detected"
  end

  test "friendship validation for synced posts" do
    # Create a user who is not Alice's friend
    stranger = User.create!(
      username: "stranger",
      display_name: "Stranger",
      public_key: "stranger_key"
    )
    
    # Try to sync content from non-friend
    synced_from_stranger = Post.new(
      user: @alice,
      content: "Content from stranger",
      is_synced: true,
      original_user_id: stranger.id,
      synced_from_user_id: stranger.id,
      synced_at: Time.current,
    )
    
    assert_not synced_from_stranger.valid?
    assert_includes synced_from_stranger.errors.full_messages, "Can only sync content from friends"
  end

  test "sync timestamps are properly managed" do
    post = @bob.posts.create!(
      content: "Synced content with timestamp",
      is_synced: true,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
    )
    
    assert_not_nil post.synced_at
    assert post.synced_at <= Time.current
    assert post.synced_at >= 1.minute.ago
  end

  test "content hash is automatically generated if missing" do
    post = Post.create!(
      user: @alice,
      content: "Auto hash generation test",
      is_synced: false,
      original_user_id: @alice.id
      # Note: content_hash is not provided
    )
    
    assert_not_nil post.content_hash
    assert post.content_hash.length == 64 # SHA256 hex string length
  end

  test "sync metadata is properly tracked" do
    # Create original post
    original = @alice.posts.create!(
      content: "Original for metadata test",
      is_synced: false,
      original_user_id: @alice.id,
    )
    
    # Create synced version
    synced = @bob.posts.create!(
      content: "Original for metadata test",
      is_synced: true,
      original_user_id: @alice.id,
      synced_from_user_id: @alice.id,
      synced_at: Time.current,
      content_hash: original.content_hash
    )
    
    # Verify all sync metadata is present
    assert synced.is_synced?
    assert_equal @alice, synced.original_user
    assert_equal @alice, synced.synced_from_user
    assert_not_nil synced.synced_at
    # Both posts should have valid content hashes
    assert_not_nil original.content_hash
    assert_not_nil synced.content_hash
    
    # Verify relationships work both ways
    assert_equal @bob, synced.user
    assert_equal "Original for metadata test", synced.content
  end

  test "bulk sync operations maintain data integrity" do
    # Simulate bulk sync data
    sync_posts_data = 5.times.map do |i|
      {
        content: "Bulk sync post #{i}",
        original_user_id: @alice.id
      }
    end
    
    # Create all posts in a transaction to ensure consistency
    Post.transaction do
      sync_posts_data.each do |post_data|
        @bob.posts.create!(
          content: post_data[:content],
          is_synced: true,
          original_user_id: post_data[:original_user_id],
          synced_from_user_id: @alice.id,
          synced_at: Time.current,
          content_hash: post_data[:content_hash]
        )
      end
    end
    
    # Verify all posts were created correctly
    synced_posts = @bob.posts.synced_posts.where(synced_from_user_id: @alice.id)
    assert_equal 5, synced_posts.count
    
    synced_posts.each_with_index do |post, index|
      assert_equal "Bulk sync post #{index}", post.content
      assert_equal @alice.id, post.original_user_id
      assert_equal @alice.id, post.synced_from_user_id
      assert post.is_synced
    end
  end

  test "migration preserves existing data integrity" do
    # This test verifies that the migration adding sync fields doesn't break existing posts
    
    # Simulate pre-migration post (would have been created before sync fields existed)
    pre_migration_post = Post.new(
      user: @alice,
      content: "Pre-migration post",
      is_synced: false
    )
    
    # Manually set the original_user_id as the migration would
    pre_migration_post.original_user_id = @alice.id
    # content_hash will be set automatically by the model
    pre_migration_post.save!
    
    # Verify post is valid and has correct default values
    assert pre_migration_post.valid?
    assert_not pre_migration_post.is_synced
    assert_equal @alice.id, pre_migration_post.original_user_id
    assert_nil pre_migration_post.synced_from_user_id
    assert_nil pre_migration_post.synced_at
    assert_not_nil pre_migration_post.content_hash
  end
end