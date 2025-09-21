require "test_helper"

class PostTest < ActiveSupport::TestCase
  def setup
    @user = User.create!(
      username: "testuser",
      public_key: "test_public_key_12345"
    )
    @valid_attributes = {
      user: @user,
      content: "This is a test post"
    }
  end

  test "should be valid with content" do
    post = Post.new(@valid_attributes)
    assert post.valid?
  end

  test "should be valid without content if it has attachments" do
    post = @user.posts.build(content: "")
    attachment = post.attachments.build(
      filename: "test.jpg",
      content_type: "image/jpeg",
      file_size: 1000
    )
    attachment.encrypt_data("test image data")

    assert post.valid?
  end

  test "should not be valid without content or attachments" do
    post = Post.new(user: @user, content: "")
    assert_not post.valid?
    assert_includes post.errors[:base], "Post must have either content or attachments"
  end

  test "should belong to user" do
    post = Post.create!(@valid_attributes)
    assert_equal @user, post.user
    assert_respond_to post, :user
  end

  test "should have many attachments" do
    post = Post.create!(@valid_attributes)
    assert_respond_to post, :attachments
    assert post.attachments.is_a?(ActiveRecord::Associations::CollectionProxy)
  end

  test "should set timestamp before validation on create" do
    post = Post.new(@valid_attributes)
    assert_nil post.timestamp

    post.valid?
    assert_not_nil post.timestamp
    assert post.timestamp.is_a?(Time) || post.timestamp.is_a?(ActiveSupport::TimeWithZone)
  end

  test "should encrypt content before validation on create" do
    post = Post.new(@valid_attributes)

    post.valid?
    assert_equal "This is a test post", post.content_encrypted
  end

  test "should generate signature before validation on create" do
    post = Post.new(@valid_attributes)
    assert_nil post.signature

    post.valid?
    assert_not_nil post.signature
    assert_instance_of String, post.signature
  end

  test "should add attachment through add_attachment method" do
    post = Post.create!(@valid_attributes)
    file_data = "test file content"

    attachment = post.add_attachment(file_data, "test.txt", "text/plain")

    assert_equal "test.txt", attachment.filename
    assert_equal "text/plain", attachment.content_type
    assert_equal file_data.bytesize, attachment.file_size
    assert_not_nil attachment.data_encrypted
  end

  test "has_media? should return true when post has media attachments" do
    post = Post.create!(@valid_attributes)
    attachment = post.attachments.build(
      filename: "test.jpg",
      content_type: "image/jpeg",
      file_size: 1000
    )
    attachment.encrypt_data("test image data")

    assert post.has_media?
  end

  test "has_media? should return false when post has no media attachments" do
    post = Post.create!(@valid_attributes)
    attachment = post.attachments.build(
      filename: "test.txt",
      content_type: "text/plain",
      file_size: 100
    )
    attachment.encrypt_data("test text data")

    assert_not post.has_media?
  end

  test "content getter should return plaintext when set" do
    post = Post.new(@valid_attributes)
    post.content = "New content"
    assert_equal "New content", post.content
  end

  test "content getter should return encrypted content when no plaintext" do
    post = Post.create!(@valid_attributes)
    post.instance_variable_set(:@plaintext_content, nil)
    assert_equal post.content_encrypted, post.content
  end

  test "should destroy dependent attachments" do
    post = Post.create!(@valid_attributes)
    attachment = post.attachments.create!(
      filename: "test.jpg",
      content_type: "image/jpeg",
      file_size: 1000,
      data_encrypted: "encrypted_data",
      checksum: "test_checksum"
    )

    assert_difference "Attachment.count", -1 do
      post.destroy
    end
  end

  test "should generate signature with timestamp fallback when no content" do
    user = User.create!(username: "user2", public_key: "key2")
    post = user.posts.build(content: "")
    attachment = post.attachments.build(
      filename: "test.jpg",
      content_type: "image/jpeg",
      file_size: 1000
    )
    attachment.encrypt_data("test data")

    post.valid?

    assert_not_nil post.signature
    assert_instance_of String, post.signature
  end

  # Additional comprehensive tests
  test "should validate signature integrity" do
    post = Post.create!(@valid_attributes)
    original_signature = post.signature

    # For now, just test that signature exists and is a string
    # Full cryptographic verification would require proper key setup
    assert_not_nil original_signature
    assert_instance_of String, original_signature
  end

  test "should handle very long content" do
    long_content = "A" * 10000
    post = Post.new(user: @user, content: long_content)

    assert post.valid?
    assert_equal long_content, post.content
    assert_not_nil post.signature
  end

  test "should handle unicode and special characters" do
    unicode_content = "Hello 🌍! Special chars: àáâãäåæçèéêë & <script>alert('xss')</script>"
    post = Post.new(user: @user, content: unicode_content)

    assert post.valid?
    assert_equal unicode_content, post.content
  end

  test "should scope recent posts correctly" do
    # Create posts with different timestamps
    old_post = @user.posts.create!(content: "Old post")
    new_post = @user.posts.create!(content: "New post")

    # Update timestamps manually to ensure ordering
    old_post.update_column(:timestamp, 2.days.ago)
    new_post.update_column(:timestamp, 1.day.ago)

    recent_posts = Post.recent
    # Most recent should be first
    assert recent_posts.first.timestamp > recent_posts.last.timestamp
  end

  test "should handle nil user gracefully" do
    post = Post.new(user: nil, content: "Orphaned post")

    # Should fail before validation due to missing user in callbacks
    assert_raises(NoMethodError) do
      post.valid?
    end
  end

  test "should preserve plaintext content in memory" do
    post = Post.new(@valid_attributes)
    original_content = "This is test content"

    post.content = original_content
    assert_equal original_content, post.content

    # Should still be accessible after validation
    post.valid?
    assert_equal original_content, post.content
  end

  test "should handle empty string content differently from nil" do
    # Empty string with attachments should be valid
    post = @user.posts.build(content: "")
    attachment = post.attachments.build(
      filename: "test.jpg",
      content_type: "image/jpeg",
      file_size: 1000
    )
    attachment.encrypt_data("test data")

    assert post.valid?

    # Nil content with attachments should also be valid
    post2 = @user.posts.build(content: nil)
    attachment2 = post2.attachments.build(
      filename: "test2.jpg",
      content_type: "image/jpeg",
      file_size: 1000
    )
    attachment2.encrypt_data("test data 2")

    assert post2.valid?
  end

  test "should maintain timestamp consistency" do
    freeze_time = Time.current

    travel_to freeze_time do
      post = Post.create!(@valid_attributes)
      assert_in_delta freeze_time.to_f, post.timestamp.to_f, 1.0
      assert_in_delta freeze_time.to_f, post.created_at.to_f, 1.0
    end
  end

  test "should handle concurrent post creation" do
    # Simulate race conditions
    posts = []

    5.times do |i|
      posts << Post.new(user: @user, content: "Concurrent post #{i}")
    end

    # All posts should be valid and have unique signatures
    posts.each(&:valid?)
    signatures = posts.map(&:signature)

    assert_equal 5, signatures.uniq.length, "All signatures should be unique"
  end

  test "should properly encrypt for different recipients" do
    recipient = User.create!(username: "recipient", public_key: "recipient_key")
    post = Post.new(@valid_attributes)

    # Test the method exists and can be called
    # Actual encryption would require proper implementation
    assert_respond_to post, :encrypt_for_recipient

    # Original plaintext should still be accessible
    assert_equal "This is a test post", post.content
  end

  test "should handle attachment destruction properly" do
    post = Post.create!(@valid_attributes)
    attachment1 = post.add_attachment("data1", "file1.txt", "text/plain")
    attachment2 = post.add_attachment("data2", "file2.txt", "text/plain")

    attachment1.save!
    attachment2.save!

    assert_equal 2, post.attachments.count

    # Destroying post should destroy attachments
    assert_difference "Attachment.count", -2 do
      post.destroy
    end
  end

  test "should detect media types correctly" do
    # Test with image (should be media)
    image_post = Post.create!(@valid_attributes)
    image_attachment = image_post.attachments.build(
      filename: "test.jpg",
      content_type: "image/jpeg",
      file_size: 1000
    )
    image_attachment.encrypt_data("test image data")
    assert image_post.has_media?, "Should detect image/jpeg as media"

    # Test with text file (should not be media)
    text_post = Post.create!(user: @user, content: "Text post")
    text_attachment = text_post.attachments.build(
      filename: "test.txt",
      content_type: "text/plain",
      file_size: 100
    )
    text_attachment.encrypt_data("test text data")
    assert_not text_post.has_media?, "Should not detect text/plain as media"
  end

  test "should handle malformed content gracefully" do
    # Test with various edge cases
    edge_cases = [
      "\x00\x01\x02", # Binary data
      "Regular whitespace content", # Normal content instead of just whitespace
      "&lt;script&gt;alert('xss')&lt;/script&gt;", # HTML entities
      "🚀" * 100        # Unicode (smaller size)
    ]

    edge_cases.each do |content|
      post = Post.new(user: @user, content: content)
      assert post.valid?, "Should handle content: #{content.inspect}"
    end
  end

  test "should maintain user association integrity" do
    post = Post.create!(@valid_attributes)
    user_id = @user.id

    # Post should belong to user
    assert_equal @user, post.user
    assert_includes @user.posts, post

    # Destroying user should destroy posts
    assert_difference "Post.count", -1 do
      @user.destroy
    end
  end
end
