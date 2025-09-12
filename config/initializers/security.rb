# Security configuration for Cipher application

# Ensure private keys are always filtered from logs and parameters
Rails.application.configure do
  # Additional parameter filtering beyond what's in application.rb
  config.filter_parameters += %w[
    private_key
    privateKey
    private_key_encrypted
    privateKeyEncrypted
    secret_key
    secretKey
    signing_key
    signingKey
    encryption_key
    encryptionKey
  ]
end

# Helper method for controllers to set serialization context
module ControllerSecurityHelpers
  def set_serialization_context(user)
    Thread.current[:current_user_for_serialization] = user&.id
  end
  
  def clear_serialization_context
    Thread.current[:current_user_for_serialization] = nil
  end
  
  # Safe user serialization for API responses
  def serialize_user_safely(user, include_own_private_key: false)
    current_user_id = session[:user_id] 
    
    # Only include private key if it's the current user AND explicitly requested
    if include_own_private_key && user.id == current_user_id
      set_serialization_context(user)
      result = user.as_json(include_private_key: true)
      clear_serialization_context
      result
    else
      # Always exclude private keys for other users or when not requested
      user.as_json(except: [:private_key, :private_key_encrypted])
    end
  end
end

# Include helpers in all controllers
if defined?(ApplicationController)
  ApplicationController.send(:include, ControllerSecurityHelpers)
end

# Log security configuration on startup
Rails.logger.info "🔐 Security: Private key filtering enabled"
Rails.logger.info "🔐 Security: Parameter filtering configured for #{Rails.application.config.filter_parameters.size} patterns"