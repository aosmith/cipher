require "application_system_test_case"

class CompleteUserJourneyTest < ApplicationSystemTestCase
  setup do
    # Clean up any existing records to ensure test isolation
    AttachmentShare.destroy_all
    Attachment.destroy_all
    Comment.destroy_all
    SyncMessage.destroy_all
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all
  end

  test "complete user journey: signup, create post, make friends, comment on feed" do
    # Step 1: User signs up
    visit new_user_path
    assert_selector "h2", text: "Create Your Cipher Identity"

    fill_in "Username", with: "alice"
    fill_in "Display name", with: "Alice Smith"
    fill_in "Password", with: "securepassword123"
    fill_in "Confirm Password", with: "securepassword123"

    click_button "Create Account"

    # Verify successful signup
    assert_current_path dashboard_users_path
    assert_text "🎉 Welcome to Cipher, Alice Smith!"
    
    alice = User.find_by(username: "alice")
    assert_not_nil alice
    assert_equal "Alice Smith", alice.display_name

    # Step 2: Alice creates a post
    visit root_path
    
    # Should see post creation form on homepage
    assert_selector "form[action='/posts']"
    fill_in "post[content]", with: "Hello Cipher! This is my first post."
    click_button "📤 Post Securely"
    
    assert_text "Post created successfully!"
    assert_text "Hello Cipher! This is my first post."
    
    alice_post = alice.posts.last
    assert_not_nil alice_post
    assert_equal "Hello Cipher! This is my first post.", alice_post.content
    assert_equal alice, alice_post.user

    # Step 3: Create another user (Bob) to be Alice's friend
    # Create Bob directly for the test (simulating a separate user registration)
    bob = User.new(username: "bob", display_name: "Bob Johnson", email: "bob@example.com")

    private_key_bob = User.derive_private_key_from_credentials("bob", "anothersecurepassword")
    public_key_bob = User.public_key_from_private_key(private_key_bob)
    bob.public_key = Base64.strict_encode64(public_key_bob)
    bob.email_verified_at = Time.current

    assert bob.save, "Bob should be created successfully: #{bob.errors.full_messages}"

    # Verify Bob was created properly
    assert_not_nil User.find_by(username: "bob"), "Bob should exist in database"

    # Step 4: Create friendship between Alice and Bob first
    # For testing, we'll create the friendship directly since friend request flow
    # might be complex in system test
    friendship = Friendship.create!(
      requester: alice,
      addressee: bob,
      status: 'accepted'
    )

    # Bob creates a post for Alice to see later
    # Need to login as Bob first since he just created the user record
    login_as bob

    # Verify Bob is logged in properly
    visit root_path
    assert_text "Hi, bob", wait: 10  # Verify login worked

    fill_in "post[content]", with: "Hi everyone! Bob here with my first post."
    click_button "📤 Post Securely"

    # Check if post creation was successful by looking for success message or post content
    if page.has_text?("Post created successfully!")
      # Success - wait for post creation
      sleep(1)
    elsif page.has_text?("Please try again later")
      # Spam prevention might be blocking - let's wait and try again
      sleep(2)
      fill_in "post[content]", with: "Hi everyone! Bob here with my first post - attempt 2."
      click_button "📤 Post Securely"
    end

    # Wait for post to be created and reload Bob from database
    bob.reload
    bob_post = bob.posts.last

    if bob_post.nil?
      # Debug information
      puts "Bob posts count: #{bob.posts.count}"
      puts "All posts count: #{Post.count}"
      puts "Current page text: #{page.text}"
    end

    assert_not_nil bob_post, "Bob should have created a post. Bob has #{bob.posts.count} posts."
    assert_equal "Hi everyone! Bob here with my first post.", bob_post.content

    # Step 5: Test feed functionality with friends

    # Step 5: Log back in as Alice
    click_link "Sign Out" if page.has_link?("Sign Out")
    login_as(alice)

    # Step 6: Alice visits the feed and sees Bob's post
    visit feed_path
    
    assert_text "Your Feed"
    assert_text "Posts from your 1 friend"
    assert_text "Hi everyone! Bob here with my first post."
    assert_text bob.username

    # Step 7: Alice comments on Bob's post
    within ".post" do
      # Find the comment form
      fill_in "comment[content]", with: "Great to see you here, Bob! Welcome to Cipher."
      click_button "Post"
    end

    # Verify comment was created
    assert_text "Comment added successfully!"
    assert_text "Great to see you here, Bob! Welcome to Cipher."
    
    comment = Comment.find_by(content: "Great to see you here, Bob! Welcome to Cipher.")
    assert_not_nil comment
    assert_equal alice, comment.user
    assert_equal bob_post, comment.post

    # Step 8: Verify comment appears in feed
    visit feed_path
    assert_text "Great to see you here, Bob! Welcome to Cipher."
    assert_text "1 comment"

    # Step 9: Test deleting own comment
    within ".post" do
      # Alice should see delete button for her own comment
      click_button "×", match: :first
    end

    # Confirm deletion if dialog exists
    begin
      page.accept_confirm
    rescue Capybara::ModalNotFound
      # No confirmation dialog found, continue
    end

    # Verify comment was deleted - should not appear on the feed anymore
    assert_no_text "Great to see you here, Bob! Welcome to Cipher."

    # When no comments exist, the comments section is hidden, so we won't see "0 comments"
    # Instead, verify the comments section is not displayed at all
    assert_no_selector ".comments-section"

    # Verify comment was deleted from database
    assert_nil Comment.find_by(content: "Great to see you here, Bob! Welcome to Cipher.")
  end

  test "user cannot access feed without login" do
    visit feed_path
    
    # Should be redirected to root with login prompt
    assert_current_path root_path
    assert_text "Please log in to view your feed"
  end

  test "user sees empty feed message when no friends or friend posts" do
    # Create user with no friends
    user = User.create!(
      username: "loneuser",
      display_name: "Lone User",
      public_key: "lone_user_key"
    )
    
    login_as(user)
    visit feed_path
    
    assert_text "Your Feed"
    assert_text "Posts from your 0 friends"
    assert_text "No posts yet"
    assert_text "You don't have any friends yet"
    assert_link "Find Friends"
  end

  test "user can comment on multiple posts in feed" do
    # Setup users with proper keys for system testing
    alice = User.create!(username: "alice_system", display_name: "Alice", public_key: Base64.strict_encode64("alice_public_key_data_for_system_test"))
    bob = User.create!(username: "bob_system", display_name: "Bob", public_key: Base64.strict_encode64("bob_public_key_data_for_system_test"))
    charlie = User.create!(username: "charlie_system", display_name: "Charlie", public_key: Base64.strict_encode64("charlie_public_key_data_for_system_test"))
    
    # Create friendships
    Friendship.create!(requester: alice, addressee: bob, status: 'accepted')
    Friendship.create!(requester: alice, addressee: charlie, status: 'accepted')
    
    # Create posts
    bob_post = bob.posts.create!(content: "Bob's post")
    charlie_post = charlie.posts.create!(content: "Charlie's post")
    
    login_as(alice)
    visit feed_path
    
    assert_text "Posts from your 2 friends"
    assert_text "Bob's post"
    assert_text "Charlie's post"
    
    # Comment on both posts
    posts = page.all('.post')

    # Ensure we have at least 2 posts
    assert posts.length >= 2, "Expected at least 2 posts but found #{posts.length}"

    # Comment on first post
    within first('.post') do
      fill_in "comment[content]", with: "Comment on first post"
      click_button "Post"
    end

    assert_text "Comment added successfully!"

    # Re-find all posts after page may have refreshed, then comment on second post
    within page.all('.post')[1] do
      fill_in "comment[content]", with: "Comment on second post"
      click_button "Post"
    end
    
    assert_text "Comment added successfully!"
    
    # Verify both comments exist
    assert_equal 2, Comment.count
    assert_text "Comment on first post"
    assert_text "Comment on second post"
  end

  private

end