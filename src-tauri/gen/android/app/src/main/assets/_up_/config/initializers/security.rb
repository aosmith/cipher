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
  # Safe user serialization for API responses
  def serialize_user_safely(user, include_own_private_key: false)
    if include_own_private_key
      Rails.logger.warn "Private key serialization requested for user ##{user.id}, but keys are managed client-side"
    end

    user.as_json(except: [ :private_key, :private_key_encrypted ])
  end
end

# Include helpers in all controllers
if defined?(ApplicationController)
  ApplicationController.send(:include, ControllerSecurityHelpers)
end

# Log security configuration on startup
Rails.logger.info "🔐 Security: Private key filtering enabled"
Rails.logger.info "🔐 Security: Parameter filtering configured for #{Rails.application.config.filter_parameters.size} patterns"
