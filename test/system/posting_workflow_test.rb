require "application_system_test_case"

class PostingWorkflowTest < ApplicationSystemTestCase
  def setup
    ApplicationRecord.connection.disable_referential_integrity do
      [ Comment, Post, Friendship, User ].each(&:delete_all)
    end

    @user = User.create!(
      username: "testuser",
      public_key: "test_public_key_12345"
    )
  end

  test "user can create a text post from homepage" do
    login_as(@user)

    visit root_path
    assert_text "🔐 Cipher"

    fill_in "post[content]", with: "This is my first test post!"
    click_button "📤 Post Securely"

    assert_text "Post created successfully!"
    assert_text "This is my first test post!"

    post = Post.last
    assert_equal "This is my first test post!", post.content.strip
    assert_equal @user, post.user
  end

  test "user can create a post with file attachment" do
    login_as(@user)
    visit new_post_path

    fill_in "post[content]", with: "Post with attachment"

    attach_file "attachments[]", Rails.root.join("test/fixtures/files/test_file.txt")

    click_button "📤 Post Securely"

    assert_text "Post with attachment"
    assert_text "test_file.txt"

    post = Post.last
    assert_equal 1, post.attachments.count
    assert_equal "test_file.txt", post.attachments.first.filename
  end

  test "user can delete a post" do
    login_as(@user)

    post = @user.posts.create!(content: "Delete me")

    visit posts_path
    within first(".message-item.post", text: "Delete me") do
      click_button "Delete"
    end

    assert_text "Post deleted successfully!"
    refute Post.exists?(post.id)
  end

  test "user can create post without content but with attachment" do
    login_as(@user)

    visit new_post_path

    attach_file "attachments[]", Rails.root.join("test/fixtures/files/test_file.txt")

    click_button "📤 Post Securely"

    assert_text "test_file.txt"

    post = Post.last
    assert post.content.blank?
    assert_equal 1, post.attachments.count
  end

  test "user sees post timestamps" do
    login_as(@user)

    freeze_time = Time.current
    travel_to freeze_time do
      @user.posts.create!(content: "Timestamped post")
    end

    visit posts_path

    assert_text "Timestamped post"
    assert_selector "[data-timestamp]"
  end

  test "user can navigate between post pages" do
    login_as(@user)

    post = @user.posts.create!(content: "Navigation test post")

    visit root_path

    click_link "Posts"
    assert_current_path posts_path
    assert_text "📝 My Posts"

    click_link "Navigation test post"
    assert_current_path post_path(post)
    assert_text "Navigation test post"

    find('a[aria-label="Edit post"]').click
    assert_current_path edit_post_path(post)
    assert_field "post[content]", with: "Navigation test post"

    click_link "Cancel"
    assert_current_path post_path(post)
  end

  test "user sees media indicators for posts with images" do
    login_as(@user)

    post = @user.posts.create!(content: "Post with image")
    post.attachments.create!(
      filename: "photo.jpg",
      content_type: "image/jpeg",
      file_size: 1000,
      data_encrypted: "fake_encrypted_image_data",
      checksum: "fake_checksum"
    )

    visit posts_path

    assert_selector ".post[data-has-media='true']"
    assert_text "Post with image"
  end
end
