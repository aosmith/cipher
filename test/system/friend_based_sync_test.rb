require "application_system_test_case"

class FriendBasedSyncTest < ApplicationSystemTestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [Comment, Post, Friendship, User].each(&:delete_all)
    end

    @alice = User.create!(username: "alice", display_name: "Alice", public_key: "alice_public_key")
    @bob   = User.create!(username: "bob",   display_name: "Bob",   public_key: "bob_public_key")

    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")
    @bob_post = @bob.posts.create!(content: "Bob's original post that should appear in Alice's feed")
  end

  test "local hosting dashboard shows core sections" do
    login_as(@alice)
    visit local_hosting_users_path

    assert_text "Hosting Status"
    assert_text "Storage Quota"
    assert_text "Earnings Dashboard"

    within ".hosting-overview" do
      assert_selector "#hosting-status-text", text: /Hosting/i
      assert_selector "#hosting-toggle", visible: :all
    end

    within ".quota-config" do
      assert_selector "#quota-slider"
      assert_selector "#quota-input"
    end
  end

  test "friends content is visible in feed without manual sync" do
    login_as(@alice)
    visit feed_path

    assert_text "Your Feed"
    assert_text "Posts from your 1 friend"
    assert_text @bob_post.content
    assert_text @bob.username
  end

  test "multiple visits to hosting dashboard keep status stable" do
    login_as(@alice)

    2.times do
      visit local_hosting_users_path
      within ".hosting-overview" do
        assert_selector "#hosting-status-text", text: "Hosting"
      end
    end
  end
end
