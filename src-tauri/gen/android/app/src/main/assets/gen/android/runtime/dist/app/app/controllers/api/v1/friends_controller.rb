class Api::V1::FriendsController < ApplicationController
  protect_from_forgery with: :null_session
  before_action :authenticate_user

  def index
    friends = current_user.friends.select(:id, :username, :display_name, :public_key, :created_at)
    render json: friends
  end

  def search_by_public_key
    public_key = params[:public_key]

    if public_key.blank?
      return render json: { error: "public_key required" }, status: 400
    end

    user = User.find_by(public_key: public_key)

    if user
      # Only return public information, never private keys
      render json: {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        public_key: user.public_key,
        created_at: user.created_at,
        is_friend: current_user.friends_with?(user),
        pending_request: current_user.sent_friendships.pending.exists?(addressee: user)
      }
    else
      render json: { error: "User not found" }, status: 404
    end
  end

  def send_request
    # Accept either username or public_key
    if params[:username].present?
      target_user = User.find_by(username: params[:username])
    elsif params[:public_key].present?
      target_user = User.find_by(public_key: params[:public_key])
    else
      respond_to do |format|
        format.json { render json: { error: "Username or public key required" }, status: 400 }
        format.html {
          flash[:error] = "Username or public key required"
          redirect_to friends_users_path and return
        }
      end
    end

    if target_user.nil?
      respond_to do |format|
        format.json { render json: { error: "User not found" }, status: 404 }
        format.html {
          flash[:error] = "User not found"
          redirect_to friends_users_path and return
        }
      end
    end

    if target_user == current_user
      respond_to do |format|
        format.json { render json: { error: "You can't send a friend request to yourself" }, status: 400 }
        format.html {
          flash[:error] = "You can't send a friend request to yourself"
          redirect_to friends_users_path and return
        }
      end
    end

    friendship = current_user.send_friend_request_to(target_user)

    if friendship
      respond_to do |format|
        format.json {
          render json: {
            message: "Friend request sent successfully",
            friendship: {
              id: friendship.id,
              status: friendship.status,
              addressee: {
                id: target_user.id,
                username: target_user.username,
                display_name: target_user.display_name
              }
            }
          }
        }
        format.html {
          flash[:success] = "Friend request sent successfully"
          redirect_to friends_users_path
        }
      end
    else
      respond_to do |format|
        format.json { render json: { error: "Friend request could not be sent. You may already be friends or have a pending request." }, status: 400 }
        format.html {
          flash[:error] = "Friend request could not be sent. You may already be friends or have a pending request."
          redirect_to friends_users_path
        }
      end
    end
  end

  def respond_to_request
    friendship_id = params[:friendship_id]
    action = params[:action_type] # 'accept', 'decline', 'block'

    friendship = current_user.received_friendships.find_by(id: friendship_id)

    if friendship.nil?
      respond_to do |format|
        format.json { render json: { error: "Friend request not found" }, status: 404 }
        format.html {
          flash[:error] = "Friend request not found"
          redirect_to friends_users_path and return
        }
      end
    end

    case action
    when "accept"
      friendship.accept!
      message = "Friend request accepted"
    when "decline"
      friendship.decline!
      message = "Friend request declined"
    when "block"
      friendship.block!
      message = "User blocked"
    else
      respond_to do |format|
        format.json { render json: { error: "Invalid action" }, status: 400 }
        format.html {
          flash[:error] = "Invalid action"
          redirect_to friends_users_path and return
        }
      end
    end

    respond_to do |format|
      format.json {
        render json: {
          message: message,
          friendship: {
            id: friendship.id,
            status: friendship.status,
            requester: {
              id: friendship.requester.id,
              username: friendship.requester.username,
              display_name: friendship.requester.display_name
            }
          }
        }
      }
      format.html {
        flash[:success] = message
        redirect_to friends_users_path
      }
    end
  end

  def destroy
    friendship = Friendship.involving_user(current_user).find_by(id: params[:id])

    if friendship.nil?
      respond_to do |format|
        format.json { render json: { error: "Friendship not found" }, status: 404 }
        format.html {
          flash[:error] = "Friendship not found"
          redirect_to friends_users_path and return
        }
      end
    end

    friendship.destroy

    respond_to do |format|
      format.json { render json: { message: "Friendship removed successfully" } }
      format.html {
        flash[:success] = "Friendship removed successfully"
        redirect_to friends_users_path
      }
    end
  end

  # Get pending friend requests
  def show
    case params[:id]
    when "requests"
      pending_received = current_user.pending_friend_requests.map do |friendship|
        {
          id: friendship.id,
          type: "received",
          user: {
            id: friendship.requester.id,
            username: friendship.requester.username,
            display_name: friendship.requester.display_name,
            public_key: friendship.requester.public_key
          },
          created_at: friendship.created_at
        }
      end

      pending_sent = current_user.sent_friend_requests.map do |friendship|
        {
          id: friendship.id,
          type: "sent",
          user: {
            id: friendship.addressee.id,
            username: friendship.addressee.username,
            display_name: friendship.addressee.display_name,
            public_key: friendship.addressee.public_key
          },
          created_at: friendship.created_at
        }
      end

      render json: {
        received: pending_received,
        sent: pending_sent
      }
    else
      render json: { error: "Invalid request" }, status: 400
    end
  end

  private

  def authenticate_user
    user_id = session[:user_id]
    @current_user = User.find_by(id: user_id) if user_id

    unless @current_user
      render json: { error: "Authentication required" }, status: 401
    end
  end

  def current_user
    @current_user
  end
end
