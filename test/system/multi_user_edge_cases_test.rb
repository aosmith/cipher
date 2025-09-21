require "application_system_test_case"

class MultiUserEdgeCasesTest < ApplicationSystemTestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ Comment, Post, Friendship, User ].each(&:delete_all)
    end

    @alice = User.create!(username: "alice", display_name: "Alice", public_key: "alice_key")
    @bob   = User.create!(username: "bob",   display_name: "Bob",   public_key: "bob_key")
    @carol = User.create!(username: "carol", display_name: "Carol", public_key: "carol_key")

    [ @alice, @bob, @carol ].combination(2).each do |(requester, addressee)|
      Friendship.create!(requester:, addressee:, status: "accepted")
    end

    @bob_post = @bob.posts.create!(content: "Bob's shared update")
  end

  test "multiple users can view hosting dashboard concurrently" do
    Capybara.using_session(:alice) do
      login_as(@alice)
      visit local_hosting_users_path
      assert_text "Hosting Status"
    end

    Capybara.using_session(:bob) do
      login_as(@bob)
      visit local_hosting_users_path
      assert_text "Hosting Status"
    end

    Capybara.using_session(:carol) do
      login_as(@carol)
      visit local_hosting_users_path
      assert_text "Hosting Status"
    end
  end

  test "feed renders consistently with concurrent sessions" do
    Capybara.using_session(:alice) do
      login_as(@alice)
      visit feed_path
      assert_text "Your Feed"
      assert_text @bob_post.content
    end

    Capybara.using_session(:bob) do
      login_as(@bob)
      visit feed_path
      assert_text "Your Feed"
    end
  end
end
