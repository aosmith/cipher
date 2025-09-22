class Api::V1::AuthController < ApplicationController
  # Use null session for API endpoints to prevent CSRF issues while maintaining protection
  protect_from_forgery with: :null_session

  def verify_identity
    username = params[:username]
    public_key = params[:public_key]

    if username.blank? || public_key.blank?
      render json: { valid: false, error: "Username and public key required" }, status: 400
      return
    end

    user = User.find_by(username: username)

    if user && user.public_key == public_key
      render json: { valid: true, user_id: user.id, username: user.username }
    else
      render json: { valid: false, error: "Invalid credentials" }
    end
  rescue => e
    Rails.logger.error "Identity verification error: #{e.message}"
    render json: { valid: false, error: "Server error" }, status: 500
  end

  def login
    # SECURITY: Only allow user_id login in test environment
    if params[:user_id].present? && Rails.env.test?
      # Test environment only: simple user_id login for system tests
      user = User.find(params[:user_id])
      session[:user_id] = user.id

      # For system tests, also set a cookie that persists across browser sessions
      cookies[:test_user_id] = user.id

      if params[:test_login].present?
        # For system tests - redirect to homepage after setting session
        redirect_to root_path
        return
      else
        # For API calls - return JSON
        render json: { success: true, user_id: user.id, username: user.username }
        return
      end
    elsif params[:user_id].present? && !Rails.env.test?
      # SECURITY: Block user_id login in non-test environments
      render json: { success: false, error: "Invalid authentication method" }, status: 400
      return
    end

    # Production login with username/password OR username/public_key
    username = params[:username]
    password = params[:password]
    public_key = params[:public_key]

    if username.blank? || (password.blank? && public_key.blank?)
      render json: { success: false, error: "Username and password or public key required" }, status: 400
      return
    end

    user = User.find_by(username: username)

    unless user
      render json: { success: false, error: "Invalid credentials" }, status: 401
      return
    end

    authenticated = false

    if password.present?
      # Password-based authentication: derive keys and verify
      begin
        # Derive private key from username + password (same as user creation)
        derived_private_key = User.derive_private_key_from_credentials(username, password)
        derived_public_key = User.public_key_from_private_key(derived_private_key)

        # Verify the derived public key matches the stored one
        if user.public_key == Base64.strict_encode64(derived_public_key)
          authenticated = true
        end
      rescue => e
        Rails.logger.error "Key derivation error during login: #{e.message}"
        # Fall through to failed authentication
      end
    elsif public_key.present?
      # Public key based authentication
      if user.public_key == public_key
        authenticated = true
      end
    end

    if authenticated
      # Set session
      session[:user_id] = user.id

      # Handle redirects for web forms vs API calls
      if request.format.html? || params[:test_login].present?
        redirect_to root_path
      else
        render json: { success: true, user_id: user.id, username: user.username }
      end
    else
      if request.format.html?
        redirect_to import_keys_users_path, alert: "Invalid username or password"
      else
        render json: { success: false, error: "Invalid credentials" }, status: 401
      end
    end
  rescue => e
    Rails.logger.error "Login error: #{e.message}"
    render json: { success: false, error: "Server error" }, status: 500
  end

  def logout
    session[:user_id] = nil
    render json: { success: true }
  rescue => e
    Rails.logger.error "Logout error: #{e.message}"
    render json: { success: false, error: "Server error" }, status: 500
  end
end
