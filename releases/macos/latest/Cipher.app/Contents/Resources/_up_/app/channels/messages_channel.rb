class MessagesChannel < ApplicationCable::Channel
  def subscribed
    stream_for current_user
  end

  def unsubscribed
    # Any cleanup needed when channel is unsubscribed
  end

  private

  def current_user
    # Get user from session - you may need to adjust this based on your authentication
    User.find(params[:user_id]) if params[:user_id]
  end
end
