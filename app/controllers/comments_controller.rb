class CommentsController < ApplicationController
  before_action :require_user
  before_action :set_post

  def create
    @comment = @post.comments.build(comment_params)
    @comment.user = current_user_session

    if @comment.save
      redirect_to feed_path, notice: "Comment added successfully!"
    else
      redirect_to feed_path, alert: "Failed to add comment: #{@comment.errors.full_messages.join(', ')}"
    end
  end

  def destroy
    @comment = @post.comments.find(params[:id])

    if @comment.user == current_user_session
      @comment.destroy
      redirect_to feed_path, notice: "Comment deleted successfully!"
    else
      redirect_to feed_path, alert: "You can only delete your own comments."
    end
  end

  private

  def set_post
    @post = Post.find(params[:post_id])
  end

  def comment_params
    params.require(:comment).permit(:content)
  end

  def require_user
    unless current_user_session
      redirect_to root_path, alert: "Please log in to comment"
    end
  end

  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end
end