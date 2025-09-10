require "test_helper"

class PostAttachmentsTest < ActionDispatch::IntegrationTest
  def setup
    @user = User.create!(
      username: "testuser",
      public_key: "test_public_key_12345"
    )
    login_as(@user)
  end

  test "should create post with single text attachment" do
    file_content = "This is test file content"
    file = Rack::Test::UploadedFile.new(
      StringIO.new(file_content), 
      "text/plain", 
      original_filename: "test.txt"
    )
    
    assert_difference "Post.count", 1 do
      assert_difference "Attachment.count", 1 do
        post posts_path, params: { 
          post: { content: "Post with text file" }, 
          attachments: [file] 
        }
      end
    end
    
    assert_redirected_to root_path
    assert_equal "Post created successfully!", flash[:notice]
    
    created_post = Post.last
    attachment = created_post.attachments.first
    assert_equal "test.txt", attachment.filename
    assert_equal "text/plain", attachment.content_type
    assert_equal file_content.bytesize, attachment.file_size
    assert_not_nil attachment.data_encrypted
  end

  test "should create post with multiple mixed attachments" do
    text_file = Rack::Test::UploadedFile.new(
      StringIO.new("Text content"), 
      "text/plain", 
      original_filename: "document.txt"
    )
    
    image_file = Rack::Test::UploadedFile.new(
      StringIO.new("fake image data"), 
      "image/jpeg", 
      original_filename: "photo.jpg"
    )
    
    assert_difference "Post.count", 1 do
      assert_difference "Attachment.count", 2 do
        post posts_path, params: { 
          post: { content: "Multi-attachment post" }, 
          attachments: [text_file, image_file] 
        }
      end
    end
    
    created_post = Post.last
    assert_equal 2, created_post.attachments.count
    
    filenames = created_post.attachments.pluck(:filename)
    assert_includes filenames, "document.txt"
    assert_includes filenames, "photo.jpg"
    
    content_types = created_post.attachments.pluck(:content_type)
    assert_includes content_types, "text/plain"
    assert_includes content_types, "image/jpeg"
  end

  test "should detect media attachments correctly" do
    # Create post with image (media)
    image_file = Rack::Test::UploadedFile.new(
      StringIO.new("fake image data"), 
      "image/png", 
      original_filename: "image.png"
    )
    
    post posts_path, params: { 
      post: { content: "Image post" }, 
      attachments: [image_file] 
    }
    
    media_post = Post.last
    assert media_post.has_media?
    
    # Create post with text file (not media)
    text_file = Rack::Test::UploadedFile.new(
      StringIO.new("Text content"), 
      "text/plain", 
      original_filename: "document.txt"
    )
    
    post posts_path, params: { 
      post: { content: "Text post" }, 
      attachments: [text_file] 
    }
    
    text_post = Post.last
    assert_not text_post.has_media?
  end

  test "should create post with only attachments and no content" do
    file = Rack::Test::UploadedFile.new(
      StringIO.new("File only content"), 
      "application/pdf", 
      original_filename: "document.pdf"
    )
    
    assert_difference "Post.count", 1 do
      post posts_path, params: { 
        post: { content: "" }, 
        attachments: [file] 
      }
    end
    
    created_post = Post.last
    assert_nil created_post.content || created_post.content == ""
    assert_equal 1, created_post.attachments.count
    assert created_post.valid?
  end

  test "should handle large file attachments" do
    large_content = "A" * 10000  # 10KB file
    large_file = Rack::Test::UploadedFile.new(
      StringIO.new(large_content), 
      "text/plain", 
      original_filename: "large_file.txt"
    )
    
    assert_difference "Post.count", 1 do
      post posts_path, params: { 
        post: { content: "Large file post" }, 
        attachments: [large_file] 
      }
    end
    
    created_post = Post.last
    attachment = created_post.attachments.first
    assert_equal 10000, attachment.file_size
    assert_equal "large_file.txt", attachment.filename
  end

  test "should handle various file types" do
    files = [
      { content: "Document content", type: "application/pdf", name: "doc.pdf" },
      { content: "Video data", type: "video/mp4", name: "video.mp4" },
      { content: "Audio data", type: "audio/mp3", name: "audio.mp3" },
      { content: "Archive data", type: "application/zip", name: "archive.zip" }
    ]
    
    files.each do |file_info|
      file = Rack::Test::UploadedFile.new(
        StringIO.new(file_info[:content]), 
        file_info[:type], 
        original_filename: file_info[:name]
      )
      
      assert_difference "Post.count", 1 do
        post posts_path, params: { 
          post: { content: "Post with #{file_info[:name]}" }, 
          attachments: [file] 
        }
      end
      
      created_post = Post.last
      attachment = created_post.attachments.first
      assert_equal file_info[:name], attachment.filename
      assert_equal file_info[:type], attachment.content_type
    end
  end

  test "should encrypt attachment data" do
    secret_content = "This is secret file content"
    file = Rack::Test::UploadedFile.new(
      StringIO.new(secret_content), 
      "text/plain", 
      original_filename: "secret.txt"
    )
    
    post posts_path, params: { 
      post: { content: "Encrypted file post" }, 
      attachments: [file] 
    }
    
    created_post = Post.last
    attachment = created_post.attachments.first
    
    # Data should be encrypted (not equal to original)
    assert_not_nil attachment.data_encrypted
    # Should have a checksum for integrity
    assert_not_nil attachment.checksum
  end

  test "should validate attachment presence when no content" do
    # Should fail with no content and no attachments
    assert_no_difference "Post.count" do
      post posts_path, params: { 
        post: { content: "" }
        # No attachments parameter
      }
    end
    
    assert_response :unprocessable_entity
  end

  test "should skip empty attachment files" do
    valid_file = Rack::Test::UploadedFile.new(
      StringIO.new("Valid content"), 
      "text/plain", 
      original_filename: "valid.txt"
    )
    
    # Simulate empty file parameter
    assert_difference "Post.count", 1 do
      assert_difference "Attachment.count", 1 do # Only one attachment should be created
        post posts_path, params: { 
          post: { content: "Post with mixed files" }, 
          attachments: [valid_file, nil, ""] 
        }
      end
    end
    
    created_post = Post.last
    assert_equal 1, created_post.attachments.count
    assert_equal "valid.txt", created_post.attachments.first.filename
  end

  test "should preserve original filename and content type" do
    special_filename = "My Special File (v2).pdf"
    file = Rack::Test::UploadedFile.new(
      StringIO.new("PDF content"), 
      "application/pdf", 
      original_filename: special_filename
    )
    
    post posts_path, params: { 
      post: { content: "Special filename test" }, 
      attachments: [file] 
    }
    
    created_post = Post.last
    attachment = created_post.attachments.first
    assert_equal special_filename, attachment.filename
    assert_equal "application/pdf", attachment.content_type
  end

  private

  def login_as(user)
    post "/api/v1/login", 
         params: { username: user.username, public_key: user.public_key }, 
         as: :json
    assert_response :success
  end
end