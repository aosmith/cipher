require "test_helper"

class AttachmentTest < ActiveSupport::TestCase
  def setup
    @user = User.create!(
      username: "testuser",
      public_key: "test_public_key_12345"
    )
    @post = @user.posts.create!(content: "Test post")
    @valid_attributes = {
      post: @post,
      filename: "test.jpg",
      content_type: "image/jpeg",
      file_size: 1000
    }
  end

  test "should be valid with valid attributes after encryption" do
    attachment = Attachment.new(@valid_attributes)
    attachment.encrypt_data("test file data")
    assert attachment.valid?
  end

  test "should require filename" do
    attachment = Attachment.new(@valid_attributes.except(:filename))
    attachment.encrypt_data("test data")
    assert_not attachment.valid?
    assert_includes attachment.errors[:filename], "can't be blank"
  end

  test "should require content_type" do
    attachment = Attachment.new(@valid_attributes.except(:content_type))
    attachment.encrypt_data("test data")
    assert_not attachment.valid?
    assert_includes attachment.errors[:content_type], "can't be blank"
  end

  test "should require file_size" do
    attachment = Attachment.new(@valid_attributes.except(:file_size))
    attachment.encrypt_data("test data")
    assert_not attachment.valid?
    assert_includes attachment.errors[:file_size], "can't be blank"
  end

  test "should require file_size to be greater than 0" do
    attachment = Attachment.new(@valid_attributes.merge(file_size: 0))
    attachment.encrypt_data("test data")
    assert_not attachment.valid?
    assert_includes attachment.errors[:file_size], "must be greater than 0"
  end

  test "should require data_encrypted after encryption" do
    attachment = Attachment.new(@valid_attributes)
    assert_not attachment.valid?
    assert_includes attachment.errors[:data_encrypted], "can't be blank"
  end

  test "should generate checksum even without original data" do
    attachment = Attachment.new(@valid_attributes)
    # Don't call encrypt_data, so it should use fallback checksum generation
    assert_not attachment.valid? # Should fail for other reasons (data_encrypted)
    # But checksum should be generated
    assert_not_nil attachment.checksum
    assert_equal 64, attachment.checksum.length
  end

  test "should belong to post" do
    attachment = Attachment.create!(@valid_attributes.merge(
      data_encrypted: "encrypted_data",
      checksum: "test_checksum"
    ))
    assert_equal @post, attachment.post
    assert_respond_to attachment, :post
  end

  test "should have many attachment_shares" do
    attachment = Attachment.create!(@valid_attributes.merge(
      data_encrypted: "encrypted_data",
      checksum: "test_checksum"
    ))
    assert_respond_to attachment, :attachment_shares
    assert attachment.attachment_shares.is_a?(ActiveRecord::Associations::CollectionProxy)
  end

  test "encrypt_data should encrypt and store data" do
    attachment = Attachment.new(@valid_attributes)
    test_data = "This is test file data"

    attachment.encrypt_data(test_data)

    assert_not_nil attachment.data_encrypted
    assert_not_equal test_data, attachment.data_encrypted
    assert attachment.data_encrypted.length > 0
  end

  test "encrypt_data should generate checksum" do
    attachment = Attachment.new(@valid_attributes)
    test_data = "This is test file data"

    attachment.encrypt_data(test_data)
    attachment.valid?

    assert_not_nil attachment.checksum
    assert_equal 64, attachment.checksum.length # SHA256 hex length
  end

  test "is_image? should return true for image content types" do
    attachment = Attachment.new(@valid_attributes.merge(content_type: "image/png"))
    assert attachment.is_image?

    attachment.content_type = "image/gif"
    assert attachment.is_image?
  end

  test "is_image? should return false for non-image content types" do
    attachment = Attachment.new(@valid_attributes.merge(content_type: "video/mp4"))
    assert_not attachment.is_image?
  end

  test "is_video? should return true for video content types" do
    attachment = Attachment.new(@valid_attributes.merge(content_type: "video/mp4"))
    assert attachment.is_video?

    attachment.content_type = "video/avi"
    assert attachment.is_video?
  end

  test "is_audio? should return true for audio content types" do
    attachment = Attachment.new(@valid_attributes.merge(content_type: "audio/mp3"))
    assert attachment.is_audio?

    attachment.content_type = "audio/wav"
    assert attachment.is_audio?
  end

  test "media_type should return correct type" do
    attachment = Attachment.new(@valid_attributes)

    attachment.content_type = "image/jpeg"
    assert_equal "image", attachment.media_type

    attachment.content_type = "video/mp4"
    assert_equal "video", attachment.media_type

    attachment.content_type = "audio/mp3"
    assert_equal "audio", attachment.media_type

    attachment.content_type = "text/plain"
    assert_equal "file", attachment.media_type
  end

  test "human_file_size should format bytes correctly" do
    attachment = Attachment.new(@valid_attributes)

    attachment.file_size = 500
    assert_equal "500.0 B", attachment.human_file_size

    attachment.file_size = 1536  # 1.5 KB
    assert_equal "1.5 KB", attachment.human_file_size

    attachment.file_size = 1572864  # 1.5 MB
    assert_equal "1.5 MB", attachment.human_file_size
  end

  test "calculate_blockchain_cost should return cost in CPH per KB" do
    attachment = Attachment.new(@valid_attributes)

    attachment.file_size = 1024  # 1 KB exactly
    assert_equal 1, attachment.calculate_blockchain_cost

    attachment.file_size = 1536  # 1.5 KB, should round up to 2
    assert_equal 2, attachment.calculate_blockchain_cost

    attachment.file_size = 500   # 0.5 KB, should round up to 1
    assert_equal 1, attachment.calculate_blockchain_cost
  end

  test "blockchain_file_hash_for_storage should return checksum" do
    attachment = Attachment.new(@valid_attributes)
    attachment.encrypt_data("test data")
    attachment.valid?

    assert_equal attachment.checksum, attachment.blockchain_file_hash_for_storage
  end

  test "to_blockchain_json should return correct format" do
    attachment = Attachment.new(@valid_attributes)
    attachment.encrypt_data("test data")
    attachment.valid?

    json = attachment.to_blockchain_json

    assert_equal "test.jpg", json[:filename]
    assert_equal "image/jpeg", json[:content_type]
    assert_equal 1000, json[:file_size]
    assert_equal 1, json[:file_size_kb]
    assert_equal attachment.checksum, json[:checksum]
    assert_equal 1, json[:blockchain_cost]
    assert_equal "image", json[:media_type]
    assert_equal "1000.0 B", json[:human_size]
  end

  test "accessible_by? should return true for post owner" do
    attachment = Attachment.create!(@valid_attributes.merge(
      data_encrypted: "encrypted_data",
      checksum: "test_checksum"
    ))

    assert attachment.accessible_by?(@user)
  end

  test "decrypt_data_for_user should return nil when no access" do
    attachment = Attachment.create!(@valid_attributes.merge(
      data_encrypted: "encrypted_data",
      checksum: "test_checksum"
    ))

    # Test with a different user who doesn't have access
    other_user = User.create!(username: "other_user", public_key: "other_key")
    result = attachment.decrypt_data_for_user(other_user)

    # Should return nil since there's no AttachmentShare for this user
    assert_nil result
  end

  test "generate_checksum should use original data when available" do
    attachment = Attachment.new(@valid_attributes)
    test_data = "specific test data for checksum"

    attachment.encrypt_data(test_data)
    attachment.valid?

    expected_checksum = Digest::SHA256.hexdigest(test_data)
    assert_equal expected_checksum, attachment.checksum
  end

  test "generate_checksum should use fallback when no original data" do
    attachment = Attachment.new(@valid_attributes)
    attachment.instance_variable_set(:@original_data, nil)

    attachment.valid?

    assert_not_nil attachment.checksum
    assert_equal 64, attachment.checksum.length
  end

  test "should create attachment shares after create" do
    attachment = Attachment.new(@valid_attributes)
    attachment.encrypt_data("test data", [ @user.public_key ])

    assert_difference "AttachmentShare.count", 1 do
      attachment.save!
    end
  end
end
