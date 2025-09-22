class UsersController < ApplicationController
  before_action :set_user, only: [ :show ]

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

    # Generate cryptographic keys from username + password (for local-only app)
    if @user.errors.empty?
      begin
        # Derive private key from username + password using same method as authentication
        private_key = User.derive_private_key_from_credentials(@user.username, password)
        public_key = User.public_key_from_private_key(private_key)

        # Store the public key (private key is derived on demand)
        @user.public_key = Base64.strict_encode64(public_key)

        Rails.logger.info "Derived key pair for user: #{@user.username}"
        Rails.logger.info "Public key length: #{@user.public_key.length} characters"
        # NOTE: Private key can be re-derived from credentials when needed

      rescue => e
        Rails.logger.error "Key derivation failed: #{e.message}"
        @user.errors.add(:base, "Key generation failed. Please try again.")
      end
    end

    if @user.errors.empty? && @user.save
      # Set this user as the current session user
      session[:user_id] = @user.id
      redirect_to dashboard_users_path, notice: "Welcome to Cipher! Your account has been created successfully."
    else
      Rails.logger.error "User creation failed: #{@user.errors.full_messages}"
      render :new, status: :unprocessable_content
    end
  end

  def export_keys
    @user = current_user_session
    redirect_to root_path, alert: "Please log in first" unless @user
    # Show the backup instructions page
  end

  def import_keys
    # Show the identity restoration page
    # Authentication is handled client-side via API
  end

  def host_dashboard
    # Blockchain host dashboard view
    @current_user = current_user_session
    render "host_dashboard"
  end

  def local_hosting
    # Local hosting management page
    @current_user = current_user_session
    render "local_hosting"
  end

  def p2p_status
    # API endpoint to get real-time P2P connection status
    @current_user = current_user_session
    return render json: { error: "Unauthorized" }, status: :unauthorized unless @current_user

    # Check ActionCable connection status
    cable_connected = ActionCable.server.connections.any?

    # Count active peers
    active_peers = @current_user.peers.where("last_seen > ?", 5.minutes.ago).count

    # Calculate connection status
    status = if cable_connected && active_peers > 0
      "Connected"
    elsif cable_connected
      "Online (No peers)"
    else
      "Disconnected"
    end

    render json: {
      p2p_status: status,
      active_peers: active_peers,
      cable_connected: cable_connected,
      last_updated: Time.current.iso8601
    }
  end

  def friends
    # Friend management page
    @current_user = current_user_session
    unless @current_user
      require_user_session
      return
    end
    render "friends"
  end

  def dashboard
    # User dashboard showing their keys and getting started guide
    @current_user = current_user_session
    return redirect_to root_path, alert: "Please log in first" unless @current_user
    render "dashboard"
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
