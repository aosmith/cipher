require "application_system_test_case"

class FeedCommentingTest < ApplicationSystemTestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ Comment, Post, Friendship, User ].each(&:delete_all)
    end

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

    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")

    @post1 = @bob.posts.create!(content: "Hello everyone! This is my first post on Cipher.")
    @post2 = @bob.posts.create!(content: "Just finished setting up my secure communication. Privacy is amazing!")
  end

  test "user can view feed and comment on friends posts" do
    login_as(@alice)
    visit feed_path

    assert_text "Your Feed"
    assert_text "Posts from your 1 friend"

    within first(".message-item.post") do
      fill_in "comment[content]", with: "Great to see you here, Bob! Welcome to Cipher."
      click_button "Post"
    end

    assert_text "Comment added successfully!"
    assert_text "Great to see you here, Bob! Welcome to Cipher."
    assert_text "1 comment"

    within first(".message-item.post") do
      fill_in "comment[content]", with: "Looking forward to more posts!"
      click_button "Post"
    end

    assert_text "Comment added successfully!"
    assert_text "Looking forward to more posts!"
    assert_text "2 comments"

    visit feed_path

    within first(".message-item.post", visible: :all) do
      find("button", text: "×", match: :first, visible: :all).click
    end

    assert_text "Comment deleted successfully!"
    assert_text "1 comment"
  end

  test "user can access feed through navigation" do
    login_as(@alice)

    if page.has_css?(".desktop-nav-link")
      within(".desktop-nav") { click_link "📰 Feed" }
    else
      within(".nav-links") { click_link "Feed" }
    end

    assert_current_path feed_path
    assert_text "Your Feed"
  end

  test "comments display properly with user avatars and timestamps" do
    @post1.comments.create!(
      user: @alice,
      content: "Test comment for display",
      timestamp: 5.minutes.ago
    )

    login_as(@alice)
    visit feed_path

    assert_text "Test comment for display"
    assert_text @alice.username
    assert_text "5 minutes ago"
    assert_selector ".comment-author", text: @alice.username
  end

  test "user sees empty feed when no friends" do
    loner = User.create!(username: "loner", display_name: "Lone User", public_key: "lone_user_key")

    login_as(loner)
    visit feed_path

    assert_text "Posts from your 0 friends"
    assert_text "No posts yet"
    assert_text "You don't have any friends yet"
    assert_link "Find Friends"
    assert_link "Create Post"
  end

  test "comment form validation works" do
    login_as(@alice)
    visit feed_path

    within first(".message-item.post") do
      fill_in "comment[content]", with: ""
      click_button "Post"
    end

    assert_no_text "Comment added successfully!"
  end
end
