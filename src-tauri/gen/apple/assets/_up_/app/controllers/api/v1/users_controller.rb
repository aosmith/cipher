class Api::V1::UsersController < ApplicationController
  protect_from_forgery with: :null_session
  before_action :authenticate_user, only: [:current_user_with_private_key]

  def current_user_with_private_key
    # Allow user to get their own private key over localhost connection
    render json: serialize_user_safely(current_user, include_own_private_key: true)
  end

  def by_public_key
    public_key = params[:public_key]
    
    if public_key.blank?
      return render json: { error: 'public_key required' }, status: 400
    end
    
    user = User.find_by(public_key: public_key)
    
    if user
      render json: {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        public_key: user.public_key,
        created_at: user.created_at
      }
    else
      render json: { error: 'User not found' }, status: 404
    end
  end

  private

  def authenticate_user
    user_id = session[:user_id]
    @current_user = User.find_by(id: user_id) if user_id
    
    unless @current_user
      render json: { error: 'Authentication required' }, status: 401
    end
  end

  def current_user
    @current_user
  end
end