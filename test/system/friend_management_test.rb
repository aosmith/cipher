require "application_system_test_case"

class FriendManagementTest < ApplicationSystemTestCase
  setup do
    # Clean up any existing records to ensure test isolation
    # Need to clean up dependent records first due to foreign key constraints
    AttachmentShare.destroy_all if defined?(AttachmentShare)
    Attachment.destroy_all if defined?(Attachment)
    SyncMessage.destroy_all if defined?(SyncMessage)
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all
    
    # Create test users
    @user1 = User.create!(
      username: "alice123", 
      display_name: "Alice Smith",
      public_key: "test_public_key_alice_123"
    )
    
    @user2 = User.create!(
      username: "bob456", 
      display_name: "Bob Johnson",
      public_key: "test_public_key_bob_456"
    )
    
    @user3 = User.create!(
      username: "charlie789", 
      display_name: "Charlie Brown",
      public_key: "test_public_key_charlie_789"
    )
  end

  teardown do
    # Clean up after each test
    AttachmentShare.destroy_all if defined?(AttachmentShare)
    Attachment.destroy_all if defined?(Attachment)
    SyncMessage.destroy_all if defined?(SyncMessage)
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all
  end

  test "user can access friends page when logged in" do
    # Log in as user1
    login_as(@user1)
    
    # Navigate to friends page
    visit friends_users_path
    
    # Verify page loads correctly
    assert_selector "h1", text: "👥 Friends"
    assert_text "Manage your connections for secure file sharing"
    
    # Verify main sections are present
    assert_selector ".add-friend-form"
    assert_selector "#received-requests"
    assert_selector "#sent-requests" 
    assert_selector "#friends-list"
    
    # Verify form elements
    assert_selector "input#friend-username"
    assert_selector "button[type='submit']", text: "Send Request"
  end

  test "user can send a friend request successfully" do
    login_as(@user1)
    visit friends_users_path
    
    # Wait for page to load completely
    assert_selector ".add-friend-form", wait: 5
    
    # Fill in the friend request form
    fill_in "friend-username", with: @user2.username
    
    # Submit the form
    click_button "Send Request"
    
    # Wait for the JavaScript to process the request
    assert_text "Friend request sent successfully", wait: 10
    
    # Verify the request appears in sent requests section
    within "#sent-requests" do
      assert_text @user2.username
      assert_text @user2.display_name
      assert_selector "button", text: "Cancel"
    end
    
    # Verify friendship was created in database
    friendship = Friendship.find_by(requester: @user1, addressee: @user2)
    assert_not_nil friendship
    assert_equal "pending", friendship.status
  end

  test "user receives error when sending request to non-existent user" do
    login_as(@user1)
    visit friends_users_path
    
    # Wait for page to load
    assert_selector ".add-friend-form", wait: 5
    
    # Try to send request to non-existent user
    fill_in "friend-username", with: "nonexistentuser"
    click_button "Send Request"
    
    # Should show error message
    assert_text "User not found", wait: 10
  end

  test "user cannot send friend request to themselves" do
    login_as(@user1)
    visit friends_users_path
    
    # Wait for page to load
    assert_selector ".add-friend-form", wait: 5
    
    # Try to send request to self
    fill_in "friend-username", with: @user1.username
    click_button "Send Request"
    
    # Should show error message
    assert_text "You can't send a friend request to yourself", wait: 10
  end

  test "user can view and respond to incoming friend requests" do
    # Create a pending friendship request
    friendship = Friendship.create!(
      requester: @user2,
      addressee: @user1,
      status: 'pending'
    )
    
    login_as(@user1)
    visit friends_users_path
    
    # Wait for page to load and requests to populate
    assert_selector "#received-requests", wait: 5
    
    # Verify the incoming request is displayed
    within "#received-requests" do
      assert_text @user2.username
      assert_text @user2.display_name
      assert_selector "button", text: "Accept"
      assert_selector "button", text: "Decline"
    end
  end

  test "user can accept a friend request" do
    # Create a pending friendship request
    friendship = Friendship.create!(
      requester: @user2,
      addressee: @user1,
      status: 'pending'
    )
    
    login_as(@user1)
    visit friends_users_path
    
    # Wait for requests to load
    assert_selector "#received-requests", wait: 5
    
    # Accept the friend request
    within "#received-requests" do
      click_button "Accept"
    end
    
    # Wait for success message
    assert_text "Friend request accepted", wait: 10
    
    # Verify friendship status updated in database
    friendship.reload
    assert_equal "accepted", friendship.status
    
    # Verify the friend now appears in friends list
    within "#friends-list" do
      assert_text @user2.username
      assert_text @user2.display_name
      assert_selector "button", text: "Remove"
    end
    
    # Verify request is removed from received requests
    within "#received-requests" do
      assert_text "No pending requests"
    end
  end

  test "user can decline a friend request" do
    # Create a pending friendship request
    friendship = Friendship.create!(
      requester: @user2,
      addressee: @user1,
      status: 'pending'
    )
    
    login_as(@user1)
    visit friends_users_path
    
    # Wait for requests to load
    assert_selector "#received-requests", wait: 5
    
    # Decline the friend request
    within "#received-requests" do
      click_button "Decline"
    end
    
    # Wait for success message
    assert_text "Friend request declined", wait: 10
    
    # Verify friendship status updated in database
    friendship.reload
    assert_equal "declined", friendship.status
    
    # Verify request is removed from received requests
    within "#received-requests" do
      assert_text "No pending requests"
    end
  end

  test "user can cancel a sent friend request" do
    # Create a pending friendship request sent by user1
    friendship = Friendship.create!(
      requester: @user1,
      addressee: @user2,
      status: 'pending'
    )
    
    login_as(@user1)
    visit friends_users_path
    
    # Wait for requests to load
    assert_selector "#sent-requests", wait: 5
    
    # Verify sent request is displayed
    within "#sent-requests" do
      assert_text @user2.username
      click_button "Cancel"
    end
    
    # Wait for success message
    assert_text "Friendship removed successfully", wait: 10
    
    # Verify friendship was removed from database
    assert_nil Friendship.find_by(id: friendship.id)
    
    # Verify request is removed from sent requests
    within "#sent-requests" do
      assert_text "No sent requests"
    end
  end

  test "user can see remove button for existing friends" do
    # Create an accepted friendship
    friendship = Friendship.create!(
      requester: @user1,
      addressee: @user2,
      status: 'accepted'
    )
    
    login_as(@user1)
    visit friends_users_path
    
    # Wait for friends to load
    assert_selector "#friends-list", wait: 5
    
    # Verify friend is displayed with remove button
    within "#friends-list" do
      assert_text @user2.username
      assert_text @user2.display_name
      assert_selector "button", text: "Remove"
    end
    
    # Verify friends count is correct
    assert_selector "#friends-count", text: "1"
    
    # Note: The actual remove functionality requires friendship ID mapping
    # which is a frontend implementation detail that could be improved
  end

  test "friends count is displayed correctly" do
    # Create two accepted friendships for user1
    Friendship.create!(requester: @user1, addressee: @user2, status: 'accepted')
    Friendship.create!(requester: @user3, addressee: @user1, status: 'accepted')
    
    login_as(@user1)
    visit friends_users_path
    
    # Wait for friends to load
    assert_selector "#friends-list", wait: 5
    
    # Verify friends count is correct
    assert_selector "#friends-count", text: "2"
    
    # Verify both friends are displayed
    within "#friends-list" do
      assert_text @user2.username
      assert_text @user3.username
    end
  end

  test "empty states are displayed correctly" do
    login_as(@user1)
    visit friends_users_path
    
    # Wait for page to load
    assert_selector "#friends-list", wait: 5
    
    # Verify empty states are shown
    within "#friends-list" do
      assert_text "No friends yet"
      assert_text "Add friends to start sharing files securely"
    end
    
    within "#received-requests" do
      assert_text "No pending requests"
    end
    
    within "#sent-requests" do
      assert_text "No sent requests"
    end
    
    # Verify friends count shows 0
    assert_selector "#friends-count", text: "0"
  end

  test "unauthenticated user can visit friends page but APIs will fail" do
    # Try to visit friends page without logging in
    visit friends_users_path
    
    # Page should load (no server-side authentication required)
    assert_current_path friends_users_path
    assert_selector "h1", text: "👥 Friends"
    
    # But the JavaScript should show empty states since API calls will fail
    assert_selector "#friends-list", wait: 5
  end

  test "duplicate friend requests are prevented" do
    # Create existing pending request
    Friendship.create!(
      requester: @user1,
      addressee: @user2,
      status: 'pending'
    )
    
    login_as(@user1)
    visit friends_users_path
    
    # Wait for page to load
    assert_selector ".add-friend-form", wait: 5
    
    # Try to send another request to same user
    fill_in "friend-username", with: @user2.username
    click_button "Send Request"
    
    # Should show error message about existing request
    assert_text "Friend request could not be sent", wait: 10
  end

  test "complex friend workflow with multiple users" do
    login_as(@user1)
    visit friends_users_path
    
    # Send friend request to user2
    assert_selector ".add-friend-form", wait: 5
    fill_in "friend-username", with: @user2.username
    click_button "Send Request"
    assert_text "Friend request sent successfully", wait: 10
    
    # Verify request appears in sent requests
    within "#sent-requests" do
      assert_text @user2.username
    end
    
    # Now log in as user2 to accept the request
    login_as(@user2)
    visit friends_users_path
    
    # Should see incoming request
    assert_selector "#received-requests", wait: 5
    within "#received-requests" do
      assert_text @user1.username
      click_button "Accept"
    end
    
    assert_text "Friend request accepted", wait: 10
    
    # Verify user1 appears in user2's friends list
    within "#friends-list" do
      assert_text @user1.username
    end
    
    # Switch back to user1 and verify friendship is bidirectional
    login_as(@user1)
    visit friends_users_path
    
    assert_selector "#friends-list", wait: 5
    within "#friends-list" do
      assert_text @user2.username
    end
    
    # Verify sent request is no longer shown
    within "#sent-requests" do
      assert_text "No sent requests"
    end
  end

  private

end