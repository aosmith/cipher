require "test_helper"

class PostsControllerTest < ActionDispatch::IntegrationTest
  def setup
    @user = User.create!(
      username: "testuser",
      public_key: "test_public_key_12345"
    )
    @other_user = User.create!(
      username: "otheruser", 
      public_key: "other_public_key_67890"
    )
    @post = @user.posts.create!(content: "Test post content")
  end

  # Test authentication requirements
  test "should redirect to root when not logged in for index" do
    get posts_path
    assert_redirected_to root_path
    assert_equal "Please create an account first", flash[:alert]
  end

  test "should redirect to root when not logged in for new" do
    get new_post_path
    assert_redirected_to root_path
    assert_equal "Please create an account first", flash[:alert]
  end

  test "should redirect to root when not logged in for create" do
    post posts_path, params: { post: { content: "New post" } }
    assert_redirected_to root_path
    assert_equal "Please create an account first", flash[:alert]
  end

  test "should redirect to root when not logged in for show" do
    get post_path(@post)
    assert_redirected_to root_path
    assert_equal "Please create an account first", flash[:alert]
  end

  test "should redirect to root when not logged in for edit" do
    get edit_post_path(@post)
    assert_redirected_to root_path
    assert_equal "Please create an account first", flash[:alert]
  end

  test "should redirect to root when not logged in for update" do
    patch post_path(@post), params: { post: { content: "Updated content" } }
    assert_redirected_to root_path
    assert_equal "Please create an account first", flash[:alert]
  end

  test "should redirect to root when not logged in for destroy" do
    delete post_path(@post)
    assert_redirected_to root_path
    assert_equal "Please create an account first", flash[:alert]
  end

  # Test authorized actions
  test "should get index when logged in" do
    login_as(@user)
    get posts_path
    assert_response :success
    assert_select "h1", "📝 My Posts"
  end

  test "should show user's posts in index" do
    login_as(@user)
    other_post = @other_user.posts.create!(content: "Other user's post")
    
    get posts_path
    assert_response :success
    
    # Should show user's post
    assert_match @post.content, response.body
    # Should not show other user's post
    assert_no_match other_post.content, response.body
  end

  test "should get new when logged in" do
    login_as(@user)
    get new_post_path
    assert_response :success
    assert_select "form[action=?]", posts_path
  end

  test "should create post with valid content" do
    login_as(@user)
    
    assert_difference "Post.count", 1 do
      post posts_path, params: { post: { content: "New test post" } }
    end
    
    assert_redirected_to root_path
    assert_equal "Post created successfully!", flash[:notice]
    
    created_post = Post.last
    assert_equal "New test post", created_post.content
    assert_equal @user, created_post.user
  end

  test "should not create post with empty content and no attachments" do
    login_as(@user)
    
    assert_no_difference "Post.count" do
      post posts_path, params: { post: { content: "" } }
    end
    
    assert_response :unprocessable_content
    assert_select ".error", /Post must have either content or attachments/
  end

  test "should create post with attachments but no content" do
    login_as(@user)
    file = fixture_file_upload("test_file.txt", "text/plain")
    
    assert_difference "Post.count", 1 do
      post posts_path, params: { 
        post: { content: "" }, 
        attachments: [file] 
      }
    end
    
    created_post = Post.last
    assert_equal 1, created_post.attachments.count
    assert_equal "test_file.txt", created_post.attachments.first.filename
  end

  test "should show own post" do
    login_as(@user)
    get post_path(@post)
    assert_response :success
    assert_match @post.content, response.body
  end

  test "should not show other user's post" do
    login_as(@user)
    other_post = @other_user.posts.create!(content: "Private post")
    
    get post_path(other_post)
    assert_redirected_to root_path
    assert_equal "Access denied", flash[:alert]
  end

  test "should get edit for own post" do
    login_as(@user)
    get edit_post_path(@post)
    assert_response :success
    assert_select "form[action=?]", post_path(@post)
    assert_select "textarea[name=?]", "post[content]"
  end

  test "should not get edit for other user's post" do
    login_as(@user)
    other_post = @other_user.posts.create!(content: "Other post")
    
    get edit_post_path(other_post)
    assert_redirected_to root_path
    assert_equal "Access denied", flash[:alert]
  end

  test "should update own post with valid content" do
    login_as(@user)
    
    patch post_path(@post), params: { post: { content: "Updated content" } }
    
    assert_redirected_to post_path(@post)
    assert_equal "Post updated successfully!", flash[:notice]
    
    @post.reload
    assert_equal "Updated content", @post.content
  end

  test "should not update post with invalid content" do
    login_as(@user)
    
    patch post_path(@post), params: { post: { content: "" } }
    
    assert_response :unprocessable_content
    
    @post.reload
    assert_not_equal "", @post.content
  end

  test "should not update other user's post" do
    login_as(@user)
    other_post = @other_user.posts.create!(content: "Other post")
    original_content = other_post.content
    
    patch post_path(other_post), params: { post: { content: "Hacked content" } }
    assert_redirected_to root_path
    assert_equal "Access denied", flash[:alert]
    
    other_post.reload
    assert_equal original_content, other_post.content
  end

  test "should destroy own post" do
    login_as(@user)
    
    assert_difference "Post.count", -1 do
      delete post_path(@post)
    end
    
    assert_redirected_to root_path
    assert_equal "Post deleted successfully!", flash[:notice]
  end

  test "should not destroy other user's post" do
    login_as(@user)
    other_post = @other_user.posts.create!(content: "Other post")
    
    assert_no_difference "Post.count" do
      delete post_path(other_post)
    end
    
    assert_redirected_to root_path
    assert_equal "Access denied", flash[:alert]
  end

  test "should handle multiple file attachments" do
    login_as(@user)
    file1 = fixture_file_upload("test_file.txt", "text/plain")
    file2 = fixture_file_upload("test_image.jpg", "image/jpeg")
    
    assert_difference "Post.count", 1 do
      assert_difference "Attachment.count", 2 do
        post posts_path, params: { 
          post: { content: "Post with multiple attachments" }, 
          attachments: [file1, file2] 
        }
      end
    end
    
    created_post = Post.last
    assert_equal 2, created_post.attachments.count
    filenames = created_post.attachments.pluck(:filename)
    assert_includes filenames, "test_file.txt"
    assert_includes filenames, "test_image.jpg"
  end

  test "should set post timestamp automatically" do
    login_as(@user)
    freeze_time = Time.current
    
    travel_to freeze_time do
      post posts_path, params: { post: { content: "Timestamped post" } }
    end
    
    created_post = Post.last
    assert_not_nil created_post.timestamp
    assert_in_delta freeze_time.to_f, created_post.timestamp.to_f, 1.0
  end

  test "should generate signature automatically" do
    login_as(@user)
    
    post posts_path, params: { post: { content: "Signed post" } }
    
    created_post = Post.last
    assert_not_nil created_post.signature
    assert_instance_of String, created_post.signature
  end

  test "should encrypt content automatically" do
    login_as(@user)
    content = "Secret message"
    
    post posts_path, params: { post: { content: content } }
    
    created_post = Post.last
    assert_equal content, created_post.content # Should decrypt for display
    assert_not_nil created_post.content_encrypted # Should be encrypted in DB
  end

  private

  def login_as(user)
    # For Rails integration tests, we can't directly access session
    # Instead, we'll use the login API endpoint
    post "/api/v1/login", 
         params: { username: user.username, public_key: user.public_key }, 
         as: :json
    assert_response :success
  end
end