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
    
    # Validate password fields
    password = params[:password]
    confirm_password = params[:confirm_password]
    
    if password.blank? || confirm_password.blank?
      @user.errors.add(:base, "Password and confirmation are required")
    elsif password.length < 8
      @user.errors.add(:base, "Password must be at least 8 characters long")
    elsif password != confirm_password
      @user.errors.add(:base, "Password and confirmation do not match")
    end
    
    # Generate cryptographic keys server-side if validation passes
    if @user.errors.empty?
      begin
        # Generate a random 32-byte key pair using RbNaCl
        private_key = RbNaCl::PrivateKey.generate
        public_key = private_key.public_key
        
        # Store ONLY the public key (private key is never stored or transmitted)
        @user.public_key = Base64.encode64(public_key.to_bytes).strip
        
        Rails.logger.info "Generated key pair for user: #{@user.username}"
        Rails.logger.info "Public key length: #{@user.public_key.length} characters"
        # NOTE: Private key is generated but immediately discarded for security
        
      rescue => e
        Rails.logger.error "Key generation failed: #{e.message}"
        @user.errors.add(:base, "Key generation failed. Please try again.")
      end
    end
    
    if @user.errors.empty? && @user.save
      # Set this user as the current session user
      session[:user_id] = @user.id
      redirect_to dashboard_users_path, notice: 'Welcome to Cipher! Your account has been created successfully.'
    else
      Rails.logger.error "User creation failed: #{@user.errors.full_messages}"
      render :new, status: :unprocessable_content
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

  def dashboard
    # User dashboard showing their keys and getting started guide
    @current_user = current_user_session
    return redirect_to root_path, alert: 'Please log in first' unless @current_user
    render 'dashboard'
  end

  private

  def set_user
    @user = User.find(params[:id])
  end

  def user_params
    params.require(:user).permit(:username, :display_name)
  end

  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end
end
