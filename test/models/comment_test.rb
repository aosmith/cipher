require "test_helper"

class CommentTest < ActiveSupport::TestCase
  def setup
    # Clean up fixtures to avoid interference - respect foreign key constraints
    AttachmentShare.destroy_all
    Attachment.destroy_all
    Comment.destroy_all
    SyncMessage.destroy_all
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all

    @user = User.create!(username: "testuser", public_key: "test_key")
    @post = @user.posts.create!(content: "Test post")
    @comment = Comment.new(user: @user, post: @post, content: "Test comment")
  end

  test "should be valid with valid attributes" do
    assert @comment.valid?
  end

  test "should require content" do
    @comment.content = nil
    assert_not @comment.valid?
    assert_includes @comment.errors[:content], "can't be blank"
  end

  test "should require user" do
    @comment.user = nil
    assert_not @comment.valid?
    assert_includes @comment.errors[:user], "must exist"
  end

  test "should require post" do
    @comment.post = nil
    assert_not @comment.valid?
    assert_includes @comment.errors[:post], "must exist"
  end

  test "should set timestamp on creation" do
    @comment.save!
    assert_not_nil @comment.timestamp
    assert_kind_of Time, @comment.timestamp
  end

  test "should order by recent timestamp" do
    # Create multiple comments with different timestamps
    comment1 = @post.comments.create!(user: @user, content: "First comment")
    sleep(0.01) # Ensure different timestamps
    comment2 = @post.comments.create!(user: @user, content: "Second comment")
    sleep(0.01)
    comment3 = @post.comments.create!(user: @user, content: "Third comment")

    recent_comments = Comment.recent
    assert_equal comment3, recent_comments.first
    assert_equal comment1, recent_comments.last
  end

  test "should belong to user and post" do
    @comment.save!

    assert_equal @user, @comment.user
    assert_equal @post, @comment.post
    assert_includes @user.comments, @comment
    assert_includes @post.comments, @comment
  end

  test "should be destroyed when post is destroyed" do
    @comment.save!
    comment_id = @comment.id

    @post.destroy
    assert_not Comment.exists?(comment_id)
  end

  test "should be destroyed when user is destroyed" do
    @comment.save!
    comment_id = @comment.id

    @user.destroy
    assert_not Comment.exists?(comment_id)
  end

  test "timestamp should not be updated on content change" do
    @comment.save!
    original_timestamp = @comment.timestamp

    sleep(0.01)
    @comment.update!(content: "Updated content")

    assert_equal original_timestamp.to_i, @comment.timestamp.to_i
  end
end
