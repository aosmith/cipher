class UsersController < ApplicationController
  before_action :set_user, only: [:show]

  def index
    @users = User.all.includes(:peers)
    @current_user = current_user_session
  end

  def show
    @posts = @user.posts.includes(:attachments).order(created_at: :desc).limit(20)
    @peer_count = @user.peers.active.count
  end

  def new
    @user = User.new
  end

  def create
    @user = User.new(user_params)
    
    if @user.save
      # Set this user as the current session user (simple approach)
      session[:user_id] = @user.id
      redirect_to @user, notice: 'User created successfully. Your encryption keys have been generated.'
    else
      render :new, status: :unprocessable_entity
    end
  end

  def export_keys
    @user = current_user_session
    return redirect_to root_path, alert: 'Please log in first' unless @user
    # Show the backup instructions page
  end

  def import_keys
    # Show the identity restoration page
    # Authentication is handled client-side via API
  end

  def host_dashboard
    # Blockchain host dashboard view
    @current_user = current_user_session
    render 'host_dashboard'
  end

  def local_hosting
    # Local hosting management page
    @current_user = current_user_session
    render 'local_hosting'
  end

  def friends
    # Friend management page
    @current_user = current_user_session
    render 'friends'
  end

  private

  def set_user
    @user = User.find(params[:id])
  end

  def user_params
    params.require(:user).permit(:username, :display_name).merge(
      public_key: params[:public_key]
    )
  end

  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end
end
