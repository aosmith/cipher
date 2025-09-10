# Desktop environment configuration
Rails.application.configure do
  # Settings specified here will take precedence over those in config/application.rb.

  # In the desktop environment we want faster startup and no reloading
  config.enable_reloading = false

  # Eager load for faster startup
  config.eager_load = true

  # Show full error reports.
  config.consider_all_requests_local = true

  # Enable server timing
  config.server_timing = true

  # Enable/disable caching. By default caching is disabled.
  if Rails.root.join("tmp/caching-dev.txt").exist?
    config.action_controller.perform_caching = true
    config.action_controller.enable_fragment_cache_logging = true
    config.cache_store = :solid_cache_store
    config.public_file_server.headers = { "cache-control" => "public, max-age=172800" }
  else
    config.action_controller.perform_caching = false
    config.cache_store = :null_store
  end

  # Don't care if the mailer can't send.
  config.action_mailer.raise_delivery_errors = false
  config.action_mailer.perform_caching = false

  # Desktop app specific settings
  config.force_ssl = false
  config.hosts.clear
  config.hosts << "localhost"
  config.hosts << "127.0.0.1"
  config.hosts << /.*\.local/

  # Desktop app URL helpers
  config.action_mailer.default_url_options = { host: "localhost", port: 3001 }
  
  # Logging
  config.log_level = :info
  config.log_tags = [ :request_id ]

  # Use a simple file watcher instead of evented (listen gem not available)
  config.file_watcher = ActiveSupport::FileUpdateChecker

  # Uncomment if you wish to allow Action Cable access from any origin.
  config.action_cable.disable_request_forgery_protection = true
  config.action_cable.allowed_request_origins = [
    "tauri://localhost",
    "https://tauri.localhost",
    "http://localhost:3001"
  ]

  # Desktop app specific configurations
  config.desktop_mode = true
  
  # Allow embedding in desktop app
  config.force_ssl = false
  config.ssl_options = { redirect: false }
  
  # CSP for desktop app - disable CSP completely
  config.content_security_policy_report_only = false

  # Keep CSRF enabled but configure for desktop app
  config.action_controller.default_protect_from_forgery = true
end