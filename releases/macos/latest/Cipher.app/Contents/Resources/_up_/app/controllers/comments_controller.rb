class CommentsController < ApplicationController
  before_action :require_user
  before_action :set_post

  def create
    @comment = @post.comments.build(comment_params)
    @comment.user = current_user_session

    if @comment.save
      redirect_back_or_to(@post, notice: "Comment added successfully!")
    else
      redirect_back_or_to(@post, alert: "Failed to add comment: #{@comment.errors.full_messages.join(', ')}")
    end
  end

  def destroy
    @comment = @post.comments.find(params[:id])
    
    if @comment.user == current_user_session
      @comment.destroy
      redirect_back_or_to(@post, notice: "Comment deleted successfully!")
    else
      redirect_back_or_to(@post, alert: "You can only delete your own comments.")
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

  def redirect_back_or_to(fallback_location, options = {})
    if request.referer
      redirect_back(fallback_location: fallback_location, **options)
    else
      redirect_to fallback_location, **options
    end
  end
end