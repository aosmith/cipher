class Api::V1::P2pConnectionsController < ApplicationController
  before_action :require_login

  def establish
    friend_user = User.find(params[:friend_id])
    connection = current_user.establish_p2p_connection_with(friend_user)

    if connection
      connection.mark_connected!
      redirect_to friends_users_path, notice: "🟢 Connected to #{friend_user.username}"
    else
      redirect_to friends_users_path, alert: "❌ Cannot establish connection with #{friend_user.username}"
    end
  end

  def disconnect
    friend_user = User.find(params[:friend_id])
    connection = current_user.p2p_connection_with(friend_user)

    if connection
      connection.mark_disconnected!
      redirect_to friends_users_path, notice: "🔴 Disconnected from #{friend_user.username}"
    else
      redirect_to friends_users_path, alert: "❌ No connection found with #{friend_user.username}"
    end
  end

  def status
    connections = current_user.active_p2p_connections.includes(:friend_user, :user)

    render json: {
      active_count: connections.count,
      connections: connections.map do |conn|
        {
          friend_id: conn.friend_user_id == current_user.id ? conn.user_id : conn.friend_user_id,
          friend_username: conn.friend_user_id == current_user.id ? conn.user.username : conn.friend_user.username,
          status: conn.status,
          connection_type: conn.connection_type,
          last_seen: conn.last_seen
        }
      end
    }
  end

  private

  def current_user
    @current_user ||= User.find(session[:user_id]) if session[:user_id]
  end

  def require_login
    unless current_user
      render json: { error: "Authentication required" }, status: 401
    end
  end
end
