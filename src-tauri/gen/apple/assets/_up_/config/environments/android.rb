require "active_support/core_ext/integer/time"

Rails.application.configure do
  # Settings specified here will take precedence over those in config/application.rb.

  # Android mobile environment - optimized for mobile Tauri app
  # This environment is based on development but with mobile-specific optimizations

  # Do not eager load code on boot.
  config.eager_load = false

  # Enable/disable caching. By default caching is disabled.
  # Run rails dev:cache to toggle caching.
  if Rails.root.join('tmp/caching-dev.txt').exist?
    config.action_controller.perform_caching = true
    config.action_controller.enable_fragment_cache_logging = true

    config.cache_store = :memory_store
    config.public_file_server.headers = {
      'Cache-Control' => 'public, max-age=172800'
    }
  else
    config.action_controller.perform_caching = false

    config.cache_store = :null_store
  end

  # Store uploaded files on the local file system (see config/storage.yml for options).
  config.active_storage.variant_processor = :mini_magick

  # Don't care if the mailer can't send.
  config.action_mailer.raise_delivery_errors = false

  config.action_mailer.perform_caching = false

  # Print deprecation notices to the Rails logger.
  config.active_support.deprecation = :log

  # Raise exceptions for disallowed deprecations.
  config.active_support.disallowed_deprecation = :raise

  # Tell Active Support which deprecation messages to disallow.
  config.active_support.disallowed_deprecation_warnings = []

  # Raise an error on page load if there are pending migrations.
  config.active_record.migration_error = :page_load

  # Highlight code that triggered database queries in logs.
  config.active_record.verbose_query_logs = true

  # Debug mode disables concatenation and preprocessing of assets.
  # This option may cause significant delays in view rendering with a large
  # number of complex assets.
  config.assets.debug = false

  # Suppress logger output for asset requests.
  config.assets.quiet = true

  # Mobile-specific optimizations
  config.force_ssl = false
  
  # Enable serving of images, stylesheets, and JavaScripts from an asset server
  config.asset_host = 'http://localhost:3001'

  # Raises error for missing translations.
  # config.i18n.raise_on_missing_translations = true

  # Use an evented file watcher to asynchronously detect changes in source code,
  # routes, locales, etc. This feature depends on the listen gem.
  config.file_watcher = ActiveSupport::EventedFileUpdateChecker

  # Uncomment if you wish to allow Action Cable access from any origin.
  # config.action_cable.disable_request_forgery_protection = true

  # Android mobile app specific configurations
  config.hosts << "localhost"
  config.hosts << "127.0.0.1"
  
  # Configure web console if available (development only)
  if defined?(Rails::Console)
    config.web_console.permissions = '127.0.0.1' if Rails.application.config.respond_to?(:web_console)
  end
  
  # Optimize for mobile performance
  config.logger = ActiveSupport::Logger.new(STDOUT)
  config.log_level = :info
  
  # Allow all parameter
  config.action_controller.allow_forgery_protection = false
  
  # Mobile environment identifier
  config.mobile_platform = 'android'
end

# Make Rails.env.ios? return true
module Rails
  module_function
  
  def env
    @_env ||= ActiveSupport::StringInquirer.new(
      ENV["RAILS_ENV"].presence || ENV["RACK_ENV"].presence || "development"
    )
  end
end

# Add mobile environment methods to Rails environment
ActiveSupport::StringInquirer.prepend(Module.new do
  def ios?
    self == 'ios'
  end
  
  def android?
    self == 'android'
  end
  
  def mobile?
    ios? || android?
  end
end)