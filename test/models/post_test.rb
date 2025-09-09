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
end
