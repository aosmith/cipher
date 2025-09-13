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
    assert_selector "h1", text: "Create Your Cipher Identity"

    fill_in "Username", with: "alice"
    fill_in "Display name", with: "Alice Smith"
    fill_in "Password", with: "securepassword123"
    fill_in "Confirm Password", with: "securepassword123"

    click_button "Create Account"

    # Verify successful signup
    assert_current_path dashboard_users_path
    assert_text "Welcome to Cipher, Alice Smith!"
    
    alice = User.find_by(username: "alice")
    assert_not_nil alice
    assert_equal "Alice Smith", alice.display_name

    # Step 2: Alice creates a post
    visit root_path
    
    # Should see post creation form on homepage
    assert_selector "form[action='/posts']"
    fill_in "post[content]", with: "Hello Cipher! This is my first post."
    click_button "Create Post"
    
    assert_text "Post created successfully!"
    assert_text "Hello Cipher! This is my first post."
    
    alice_post = Post.find_by(content_encrypted: "Hello Cipher! This is my first post.")
    assert_not_nil alice_post
    assert_equal alice, alice_post.user

    # Step 3: Create another user (Bob) to be Alice's friend
    # Simulate Bob signing up in a separate session
    click_link "Sign Out" if page.has_link?("Sign Out")
    
    visit new_user_path
    fill_in "Username", with: "bob"
    fill_in "Display name", with: "Bob Johnson"
    fill_in "Password", with: "anothersecurepassword"
    fill_in "Confirm Password", with: "anothersecurepassword"
    click_button "Create Account"
    
    bob = User.find_by(username: "bob")
    assert_not_nil bob

    # Bob creates a post for Alice to see later
    visit root_path
    fill_in "post[content]", with: "Hi everyone! Bob here with my first post."
    click_button "Create Post"
    
    bob_post = Post.find_by(content_encrypted: "Hi everyone! Bob here with my first post.")
    assert_not_nil bob_post

    # Step 4: Create friendship between Alice and Bob
    # For testing, we'll create the friendship directly since friend request flow
    # might be complex in system test
    Friendship.create!(
      requester: alice,
      addressee: bob,
      status: 'accepted'
    )

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
    
    # Confirm deletion
    page.accept_confirm

    assert_text "Comment deleted successfully!"
    assert_no_text "Great to see you here, Bob! Welcome to Cipher."
    assert_text "0 comments"

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
    
    within posts[0] do
      fill_in "comment[content]", with: "Comment on first post"
      click_button "Post"
    end
    
    assert_text "Comment added successfully!"
    
    within posts[1] do
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