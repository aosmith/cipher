class PostsController < ApplicationController
  before_action :require_user
  before_action :set_post, only: [:show, :edit, :update, :destroy]

  def index
    @posts = current_user_session.posts.includes(:attachments).order(created_at: :desc)
    @post = Post.new # For inline post creation
  end

  def show
    # @post is already set by set_post callback
    # Access control is handled in set_post
  end

  def new
    @post = current_user_session.posts.build
  end

  def create
    @post = current_user_session.posts.build(post_params)
    
    # Handle file attachments
    if params[:attachments].present?
      handle_attachments(params[:attachments])
    end
    
    if @post.save
      redirect_to root_path, notice: 'Post created successfully!'
    else
      # Check if it's a spam prevention error and handle with redirect
      spam_errors = @post.errors.full_messages.select do |msg|
        msg.include?('Rate limit exceeded: Maximum') || 
        msg.include?('Daily limit exceeded: Maximum') || 
        msg.include?('Duplicate content detected') ||
        msg.include?('Malicious content detected') ||
        msg.include?('New users must have at least one friend to post')
      end
      
      if spam_errors.any?
        redirect_to root_path, alert: "Please try again later. #{spam_errors.first}"
      else
        @posts = current_user_session.posts.includes(:attachments).order(created_at: :desc)
        render :index, status: :unprocessable_content
      end
    end
  end

  def edit
  end

  def update
    if @post.update(post_params)
      redirect_to @post, notice: 'Post updated successfully!'
    else
      render :edit, status: :unprocessable_content
    end
  end

  def destroy
    @post.destroy
    redirect_to root_path, notice: 'Post deleted successfully!'
  end

  private

  def set_post
    @post = current_user_session.posts.find(params[:id])
  rescue ActiveRecord::RecordNotFound
    redirect_to root_path, alert: 'Access denied' and return
  end

  def post_params
    params.require(:post).permit(:content)
  end

  def handle_attachments(attachment_params)
    attachment_params.each do |file|
      next unless file.present?
      
      # Read file data
      file_data = file.read
      
      # Create attachment
      @post.add_attachment(
        file_data,
        file.original_filename,
        file.content_type
      )
    end
  end

  def require_user
    unless current_user_session
      redirect_to root_path, alert: 'Please create an account first'
    end
  end

  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end
end