class Api::V1::AuthController < ApplicationController
  skip_before_action :verify_authenticity_token
  
  def verify_identity
    username = params[:username]
    public_key = params[:public_key]
    
    if username.blank? || public_key.blank?
      render json: { valid: false, error: 'Username and public key required' }, status: 400
      return
    end
    
    user = User.find_by(username: username)
    
    if user && user.public_key == public_key
      render json: { valid: true, user_id: user.id, username: user.username }
    else
      render json: { valid: false, error: 'Invalid credentials' }
    end
  rescue => e
    Rails.logger.error "Identity verification error: #{e.message}"
    render json: { valid: false, error: 'Server error' }, status: 500
  end
  
  def login
    # SECURITY: Only allow user_id login in test environment
    if params[:user_id].present? && Rails.env.test?
      # Test environment only: simple user_id login for system tests
      user = User.find(params[:user_id])
      session[:user_id] = user.id
      render json: { success: true, user_id: user.id, username: user.username }
      return
    elsif params[:user_id].present? && !Rails.env.test?
      # SECURITY: Block user_id login in non-test environments
      render json: { success: false, error: 'Invalid authentication method' }, status: 400
      return
    end
    
    # Production login with username/public_key
    username = params[:username]
    public_key = params[:public_key]
    
    if username.blank? || public_key.blank?
      render json: { success: false, error: 'Username and public key required' }, status: 400
      return
    end
    
    user = User.find_by(username: username)
    
    if user && user.public_key == public_key
      # Set session
      session[:user_id] = user.id
      render json: { success: true, user_id: user.id, username: user.username }
    else
      render json: { success: false, error: 'Invalid credentials' }, status: 401
    end
  rescue => e
    Rails.logger.error "Login error: #{e.message}"
    render json: { success: false, error: 'Server error' }, status: 500
  end

  def logout
    session[:user_id] = nil
    render json: { success: true }
  rescue => e
    Rails.logger.error "Logout error: #{e.message}"
    render json: { success: false, error: 'Server error' }, status: 500
  end
end