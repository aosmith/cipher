require "application_system_test_case"

class FriendManagementTest < ApplicationSystemTestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ Comment, Post, Friendship, User ].each(&:delete_all)
    end

    @alice = User.create!(username: "alice", display_name: "Alice", public_key: "alice_key")
    @bob   = User.create!(username: "bob",   display_name: "Bob",   public_key: "bob_key")
  end

  test "logged in user sees friends dashboard sections" do
    login_as(@alice)
    visit friends_users_path

    assert_text "👥 Friends"
    assert_selector ".add-friend-form"
    assert_selector "#received-requests"
    assert_selector "#sent-requests"
    assert_selector "#friends-list"
  end

  test "friends list displays existing friend" do
    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")

    login_as(@alice)
    visit friends_users_path

    within "#friends-list" do
      assert_text @bob.username
      assert_text "Friends since"
    end
  end

  test "empty states render when no requests" do
    login_as(@alice)
    visit friends_users_path

    within "#received-requests" do
      assert_text "No pending requests"
    end

    within "#sent-requests" do
      assert_text "No sent requests"
    end
  end

  test "unauthenticated visitor is redirected with login prompt" do
    visit friends_users_path
    assert_current_path root_path
    assert_text "Session expired. Please sign in again."
  end
end
