require "application_system_test_case"

class FeedCommentingTest < ApplicationSystemTestCase
  setup do
    # Clean up any existing records
    AttachmentShare.destroy_all
    Attachment.destroy_all
    Comment.destroy_all
    SyncMessage.destroy_all
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all
    
    # Create test users
    @alice = User.create!(
      username: "alice",
      display_name: "Alice Smith",
      public_key: "alice_test_key_12345"
    )
    
    @bob = User.create!(
      username: "bob", 
      display_name: "Bob Johnson",
      public_key: "bob_test_key_67890"
    )
    
    # Create friendship
    Friendship.create!(
      requester: @alice,
      addressee: @bob,
      status: 'accepted'
    )
    
    # Bob creates some posts for Alice to see
    @post1 = @bob.posts.create!(
      content: "Hello everyone! This is my first post on Cipher."
    )
    
    @post2 = @bob.posts.create!(
      content: "Just finished setting up my secure communication. Privacy is amazing!"
    )
  end

  test "user can view feed and comment on friends posts" do
    # Login as Alice
    login_user(@alice)
    
    # Navigate to feed
    click_link "Feed"
    assert_current_path feed_path
    
    # Should see Alice's feed with Bob's posts
    assert_text "Your Feed"
    assert_text "Posts from your 1 friend"
    assert_text "Hello everyone! This is my first post"
    assert_text "Just finished setting up my secure communication"
    assert_text @bob.username
    
    # Comment on the first post
    within first('.post') do
      fill_in "comment[content]", with: "Great to see you here, Bob! Welcome to Cipher."
      click_button "Post"
    end
    
    # Should see success message and comment
    assert_text "Comment added successfully!"
    assert_text "Great to see you here, Bob! Welcome to Cipher."
    
    # Verify comment count updated
    assert_text "1 comment"
    
    # Add another comment
    within first('.post') do
      fill_in "comment[content]", with: "Looking forward to more posts!"
      click_button "Post"
    end
    
    assert_text "Comment added successfully!"
    assert_text "Looking forward to more posts!"
    assert_text "2 comments"
    
    # Test deleting own comment
    within first('.post') do
      # Click delete button on own comment (should be visible)
      accept_confirm do
        first("input[type='submit'][value='×']").click
      end
    end
    
    assert_text "Comment deleted successfully!"
    assert_text "1 comment"
    
    # Verify comment was actually deleted from database
    assert_equal 1, Comment.count
  end

  test "user can access feed through navigation" do
    # Login as Alice using API
    login_user(@alice)
    
    # Test desktop navigation
    if page.has_css?('.desktop-nav-link')
      within('.desktop-nav') do
        click_link "📰 Feed"
      end
    else
      # Test web navigation
      within('.nav-links') do
        click_link "Feed"
      end
    end
    
    assert_current_path feed_path
    assert_text "Your Feed"
  end

  test "comments display properly with user avatars and timestamps" do
    # Add a comment to test data
    @post1.comments.create!(
      user: @alice,
      content: "Test comment for display",
      timestamp: 5.minutes.ago
    )
    
    # Login as Alice
    login_user(@alice)
    
    visit feed_path
    
    # Check comment display
    assert_text "Test comment for display"
    assert_text @alice.username
    assert_text "5 minutes ago"
    
    # Check comment author is present
    assert_selector ".comment-author", text: @alice.username
  end

  test "user sees empty feed when no friends" do
    # Create user with no friends
    @loner = User.create!(
      username: "loner",
      display_name: "Lone User", 
      public_key: "lone_user_key"
    )
    
    login_user(@loner)
    
    visit feed_path
    
    assert_text "Posts from your 0 friends"
    assert_text "No posts yet"
    assert_text "You don't have any friends yet"
    assert_link "Find Friends"
    assert_link "Create Post"
  end

  test "comment form validation works" do
    # Login as Alice
    login_user(@alice)
    
    visit feed_path
    
    # Try to submit empty comment
    within first('.post') do
      fill_in "comment[content]", with: ""
      click_button "Post"
    end
    
    # Should show validation error or prevent submission
    # HTML5 validation should prevent empty required field submission
    assert_no_text "Comment added successfully!"
  end

  private

  def login_user(user)
    # Visit any page first to establish session
    visit root_path
    
    # Use JavaScript to make API call and login
    script = <<~JAVASCRIPT
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
      }).then(response => response.json())
        .then(data => {
          if (data.success) {
            window.loginSuccess = true;
          } else {
            window.loginError = data.error;
          }
        });
    JAVASCRIPT
    
    page.execute_script(script)
    
    # Wait for login to complete
    sleep 1
    
    # Visit dashboard to verify login worked
    visit dashboard_users_path
    assert_text "Hi, #{user.username}"
  end
end