require "test_helper"

class CommentsControllerTest < ActionDispatch::IntegrationTest
  setup do
    @user = users(:alice)
    @other_user = users(:bob)
    @post = posts(:alice_post)
    @comment = comments(:alice_comment)
  end

  test "should create comment when logged in" do
    login_as(@user)

    assert_difference("Comment.count") do
      post post_comments_path(@post), params: { comment: { content: "New comment" } }
    end

    follow_redirect!
    comment = Comment.last
    assert_equal "New comment", comment.content
    assert_equal @user, comment.user
    assert_equal @post, comment.post
  end

  test "should not create comment without login" do
    assert_no_difference("Comment.count") do
      post post_comments_path(@post), params: { comment: { content: "New comment" } }
    end

    assert_redirected_to root_path
  end

  test "should not create comment with empty content" do
    login_as(@user)

    assert_no_difference("Comment.count") do
      post post_comments_path(@post), params: { comment: { content: "" } }
    end

    follow_redirect!
  end

  test "should destroy own comment" do
    login_as(@user)

    assert_difference("Comment.count", -1) do
      delete post_comment_path(@post, @comment)
    end

    follow_redirect!
  end

  test "should not destroy other user's comment" do
    login_as(@user)
    other_comment = @post.comments.create!(user: @other_user, content: "Other's comment")

    assert_no_difference("Comment.count") do
      delete post_comment_path(@post, other_comment)
    end

    follow_redirect!
  end

  test "should not destroy comment without login" do
    assert_no_difference("Comment.count") do
      delete post_comment_path(@post, @comment)
    end

    assert_redirected_to root_path
  end

  test "should handle non-existent comment" do
    login_as(@user)

    assert_no_difference("Comment.count") do
      delete post_comment_path(@post, id: 99999)
    end
    # Should return 404 for non-existent resource
    assert_response :not_found
  end

  test "should handle non-existent post" do
    login_as(@user)

    assert_no_difference("Comment.count") do
      post "/posts/99999/comments", params: { comment: { content: "New comment" } }
    end
    # Should return 404 for non-existent resource
    assert_response :not_found
  end

  private

  def login_as(user)
    # Use the application's login mechanism via API
    post "/api/v1/login",
         params: { username: user.username, public_key: user.public_key },
         as: :json
    @current_user = user
  end
end
