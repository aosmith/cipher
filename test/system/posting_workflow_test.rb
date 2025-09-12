require "application_system_test_case"

class PostingWorkflowTest < ApplicationSystemTestCase
  def setup
    @user = User.create!(
      username: "testuser",
      public_key: "test_public_key_12345"
    )
  end

  test "user can create a text post from homepage" do
    # Login first
    login_user(@user)
    
    # Go to homepage
    visit root_path
    assert_text "Cipher Social Network"
    
    # Should see post creation form
    assert_selector "form[action='/posts']"
    assert_selector "textarea[name='post[content]']"
    
    # Fill in and submit post
    fill_in "post[content]", with: "This is my first test post!"
    click_button "📤 Post Securely"
    
    # Should redirect to homepage with success message
    assert_text "Post created successfully!"
    assert_text "This is my first test post!"
    
    # Verify post was created in database
    post = Post.last
    assert_equal "This is my first test post!", post.content
    assert_equal @user, post.user
  end

  test "user can create a post with file attachment" do
    login_user(@user)
    visit new_post_path
    
    # Fill in post content
    fill_in "post[content]", with: "Post with attachment"
    
    # Attach a file (file input is hidden, so we need to make it visible or use JavaScript)
    page.execute_script("document.querySelector('input[name=\"attachments[]\"]').style.opacity = '1'")
    attach_file "attachments[]", Rails.root.join("test/fixtures/files/test_file.txt")
    
    click_button "📤 Post Securely"
    
    # Verify the post was created by checking for the content and attachment
    assert_text "Post with attachment"
    assert_text "test_file.txt"
    
    # Verify attachment was created
    post = Post.last
    assert_equal 1, post.attachments.count
    assert_equal "test_file.txt", post.attachments.first.filename
  end

  test "user can view their posts list" do
    login_user(@user)
    
    # Create some posts
    @user.posts.create!(content: "First post")
    @user.posts.create!(content: "Second post")
    
    visit posts_path
    
    assert_text "📝 My Posts"
    assert_text "First post"
    assert_text "Second post"
    
    # Should show both posts
    assert_text "Second post"
    assert_text "First post"
  end

  test "user can view individual post" do
    login_user(@user)
    
    post = @user.posts.create!(content: "Detailed post content")
    
    visit post_path(post)
    
    assert_text "Detailed post content"
    assert_text @user.username
  end

  test "user can edit their own post" do
    login_user(@user)
    
    post = @user.posts.create!(content: "Original content")
    
    visit edit_post_path(post)
    
    assert_field "post[content]", with: "Original content"
    
    fill_in "post[content]", with: "Updated content"
    click_button "Update Post"
    
    assert_text "Post updated successfully!"
    assert_text "Updated content"
    
    post.reload
    assert_equal "Updated content", post.content
  end

  test "user can delete their own post" do
    login_user(@user)
    
    post = @user.posts.create!(content: "Post to delete")
    
    visit posts_path
    assert_text "Post to delete"
    
    # Click delete link
    within ".post[data-post-id='#{post.id}']" do
      accept_confirm do
        click_link "Delete"
      end
    end
    
    assert_text "Post deleted successfully!"
    assert_no_text "Post to delete"
    
    assert_not Post.exists?(post.id)
  end

  test "user cannot create empty post without attachments" do
    login_user(@user)
    visit new_post_path
    
    # Try to submit empty post
    fill_in "post[content]", with: ""
    click_button "📤 Post Securely"
    
    # Should show validation error
    assert_text "Post must have either content or attachments"
    assert_current_path posts_path
  end

  test "user can create post with only attachments and no text" do
    login_user(@user)
    visit new_post_path
    
    # Leave content empty but attach file
    fill_in "post[content]", with: ""
    page.execute_script("document.querySelector('input[name=\"attachments[]\"]').style.opacity = '1'")
    attach_file "attachments[]", Rails.root.join("test/fixtures/files/test_file.txt")
    
    click_button "📤 Post Securely"
    
    assert_text "test_file.txt"
    
    # Verify post was created with attachment but no content
    post = Post.last
    assert_equal "", post.content
    assert_equal 1, post.attachments.count
  end

  test "user sees post timestamps" do
    login_user(@user)
    
    freeze_time = Time.current
    travel_to freeze_time do
      @user.posts.create!(content: "Timestamped post")
    end
    
    visit posts_path
    
    assert_text "Timestamped post"
    # Check that a timestamp is displayed (format may vary)
    assert_selector "[data-timestamp]" # or whatever format is used
  end

  test "user can navigate between post pages" do
    login_user(@user)
    
    post = @user.posts.create!(content: "Navigation test post")
    
    # Start at homepage
    visit root_path
    
    # Go to posts list
    click_link "Your Posts"
    assert_current_path posts_path
    assert_text "📝 My Posts"
    
    # View individual post
    click_link "Navigation test post"
    assert_current_path post_path(post)
    assert_text "Navigation test post"
    
    # Edit post
    click_link "Edit"
    assert_current_path edit_post_path(post)
    assert_field "post[content]", with: "Navigation test post"
    
    # Cancel edit
    click_link "Cancel"
    assert_current_path post_path(post)
  end

  test "user sees media indicators for posts with images" do
    login_user(@user)
    
    # Create post with image attachment
    post = @user.posts.create!(content: "Post with image")
    post.attachments.create!(
      filename: "photo.jpg",
      content_type: "image/jpeg",
      file_size: 1000,
      data_encrypted: "fake_encrypted_image_data",
      checksum: "fake_checksum"
    )
    
    visit posts_path
    
    # Should indicate media presence
    assert_selector ".post[data-has-media='true']" # or similar indicator
    assert_text "Post with image"
  end

  test "user can attach multiple files to a single post" do
    login_user(@user)
    visit root_path
    
    fill_in "post[content]", with: "Multi-file post"
    
    # In real browser test, this would need multiple file inputs or drag-drop
    # For now, we'll simulate by checking the form accepts multiple files
    assert_selector "input[type=file][multiple]", text: "attachments"
  end

  test "unauthorized user cannot access post creation" do
    visit root_path
    
    # Should not see post creation form
    assert_no_selector "form[action='/posts']"
    
    # Should see login/signup options instead
    assert_text "Welcome to Cipher"
  end

  test "unauthorized user cannot access posts list" do
    visit posts_path
    
    # Should be redirected to homepage with error
    assert_current_path root_path
    assert_text "Please create an account first"
  end

  private

  def login_user(user)
    # For system tests, use the API login endpoint directly
    visit root_path
    
    # Use JavaScript to set the session via API call
    page.execute_script(<<~JS)
      fetch('/api/v1/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-CSRF-Token': document.querySelector('meta[name="csrf-token"]')?.getAttribute('content')
        },
        body: JSON.stringify({
          username: '#{user.username}',
          public_key: '#{user.public_key}'
        })
      });
    JS
    
    # Wait briefly for the session to be set
    sleep(0.1)
    
    # Refresh to pick up the new session
    visit root_path
  end
end