class FeedController < ApplicationController
  before_action :require_user_session

  def index
    @posts = current_user_session.friends_posts.recent.includes(:user, :attachments, comments: :user)
    @friends_count = current_user_session.friends.count
  end
end
