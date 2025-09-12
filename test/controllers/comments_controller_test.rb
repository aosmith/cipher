require "test_helper"

class CommentsControllerTest < ActionDispatch::IntegrationTest
  setup do
    # Clean up fixtures 
    AttachmentShare.destroy_all
    Attachment.destroy_all
    Comment.destroy_all
    SyncMessage.destroy_all
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all
    
    @user = User.create!(username: "testuser", public_key: "test_key")
    @other_user = User.create!(username: "otheruser", public_key: "other_key")
    @post = @user.posts.create!(content: "Test post")
    @comment = @post.comments.create!(user: @user, content: "Test comment")
  end

  test "should create comment when logged in" do
    login_as(@user)
    
    assert_difference('Comment.count') do
      post post_comments_path(@post), params: { comment: { content: "New comment" } }
    end

    follow_redirect!
    comment = Comment.last
    assert_equal "New comment", comment.content
    assert_equal @user, comment.user
    assert_equal @post, comment.post
  end

  test "should not create comment without login" do
    assert_no_difference('Comment.count') do
      post post_comments_path(@post), params: { comment: { content: "New comment" } }
    end

    assert_redirected_to root_path
  end

  test "should not create comment with empty content" do
    login_as(@user)
    
    assert_no_difference('Comment.count') do
      post post_comments_path(@post), params: { comment: { content: "" } }
    end

    follow_redirect!
  end

  test "should destroy own comment" do
    login_as(@user)
    
    assert_difference('Comment.count', -1) do
      delete post_comment_path(@post, @comment)
    end

    follow_redirect!
  end

  test "should not destroy other user's comment" do
    login_as(@user)
    other_comment = @post.comments.create!(user: @other_user, content: "Other's comment")
    
    assert_no_difference('Comment.count') do
      delete post_comment_path(@post, other_comment)
    end

    follow_redirect!
  end

  test "should not destroy comment without login" do
    assert_no_difference('Comment.count') do
      delete post_comment_path(@post, @comment)
    end

    assert_redirected_to root_path
  end

  test "should handle non-existent comment" do
    login_as(@user)
    
    assert_raises(ActiveRecord::RecordNotFound) do
      delete post_comment_path(@post, id: 99999)
    end
  end

  test "should handle non-existent post" do
    login_as(@user)
    
    assert_raises(ActiveRecord::RecordNotFound) do
      post "/posts/99999/comments", params: { comment: { content: "New comment" } }
    end
  end

  private

  def login_as(user)
    # Simple approach for testing - override the session
    # In a real app, you'd have a proper login mechanism
    # For testing purposes, we'll directly set the session
    post '/session_stub', params: { user_id: user.id } 
    @current_user = user
  end
end