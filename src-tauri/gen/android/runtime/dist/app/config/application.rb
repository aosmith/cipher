require_relative "boot"

require "rails/all"

# Require the gems listed in Gemfile, including any gems
# you've limited to :test, :development, or :production.
Bundler.require(*Rails.groups)

module Cipher
  class Application < Rails::Application
    # Initialize configuration defaults for originally generated Rails version.
    config.load_defaults 8.0

    # Please, add to the `ignore` list any other `lib` subdirectories that do
    # not contain `.rb` files, or that should not be reloaded or eager loaded.
    # Common ones are `templates`, `generators`, or `middleware`, for example.
    config.autoload_lib(ignore: %w[assets tasks])

    # Configuration for the application, engines, and railties goes here.
    #
    # These settings can be overridden in specific environments using the files
    # in config/environments, which are processed later.
    #
    # config.time_zone = "Central Time (US & Canada)"
    # config.eager_load_paths << Rails.root.join("extras")

    # Browser runtime is intentionally disabled; all interactions are handled on the
    # per-user Rails server to keep state and latency local.
    config.x.browser_runtime_enabled = false

    # SECURITY: Filter private keys and sensitive data from logs
    config.filter_parameters += [
      :private_key, :privateKey, :private_key_encrypted,
      :password, :password_confirmation, :current_password,
      :secret, :token, :api_key, :auth_token,
      :crypto_key, :encryption_key, :signing_key,
      /private.*key/i, /.*secret.*/i, /.*token.*/i, /.*key.*private/i
    ]
  end
end
