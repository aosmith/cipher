class AttachmentsController < ApplicationController
  before_action :require_user
  before_action :set_post_and_attachment

  def show
    # Ensure the user can access this attachment
    unless @attachment.accessible_by?(current_user_session)
      redirect_to root_path, alert: 'Access denied'
      return
    end

    # For development, serve decrypted data directly
    # In production, client-side crypto would handle decryption
    # Serve encrypted data for client-side decryption
    render json: {
      filename: @attachment.filename,
      content_type: @attachment.content_type,
      file_size: @attachment.file_size,
      human_size: @attachment.human_file_size,
      media_type: @attachment.media_type,
      encrypted_data: @attachment.data_encrypted,
      dev_owner_key: @attachment.dev_owner_key, # For development only
      access_granted: true,
      attachment_id: @attachment.id
    }
  end

  def create
    # This would handle additional file uploads to existing posts
    # Implementation depends on specific requirements
    head :not_implemented
  end

  private

  def set_post_and_attachment
    @post = Post.find(params[:post_id])
    @attachment = @post.attachments.find(params[:id])
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

  def decrypt_attachment_data(attachment)
    # For development: decrypt using the stored dev_owner_key
    # In production, this would use proper public key cryptography
    return nil unless attachment.dev_owner_key.present? && attachment.data_encrypted.present?
    
    begin
      require 'rbnacl'
      require 'base64'
      
      # Decode the key and encrypted data
      key = Base64.decode64(attachment.dev_owner_key)
      encrypted_data = Base64.decode64(attachment.data_encrypted)
      
      # Decrypt using SimpleBox
      simple_box = RbNaCl::SimpleBox.from_secret_key(key)
      decrypted_data = simple_box.decrypt(encrypted_data)
      
      return decrypted_data
    rescue => e
      Rails.logger.error "Failed to decrypt attachment data: #{e.message}"
      return nil
    end
  end

  def create_placeholder_image
    # Create a simple placeholder image for development
    # In production, this would decrypt the actual image data
    if @attachment.is_image?
      # Generate a simple 200x200 placeholder image
      require 'base64'
      
      # Simple 1x1 pixel PNG in base64
      placeholder_data = Base64.decode64("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChAI9jU77mgAAAABJRU5ErkJggg==")
      return placeholder_data
    else
      # Return empty data for non-images
      return ""
    end
  end
end