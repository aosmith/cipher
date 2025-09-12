class FeedController < ApplicationController
  before_action :require_user

  def index
    @posts = current_user_session.friends_posts.recent.includes(:user, :attachments, comments: :user)
    @friends_count = current_user_session.friends.count
  end

  private

  def require_user
    unless current_user_session
      redirect_to root_path, alert: "Please log in to view your feed"
    end
  end
end