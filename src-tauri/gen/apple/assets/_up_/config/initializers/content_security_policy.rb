# Be sure to restart your server when you modify this file.

# Define an application-wide content security policy.
# See the Securing Rails Applications Guide for more information:
# https://guides.rubyonrails.org/security.html#content-security-policy-header

Rails.application.configure do
  config.content_security_policy do |policy|
    # For local-only app, allow localhost and self
    policy.default_src :self, :https, 'http://localhost:*', 'https://localhost:*'
    policy.font_src    :self, :https, :data
    policy.img_src     :self, :https, :data, 'blob:'
    policy.object_src  :none
    # Allow scripts from self and importmaps
    policy.script_src  :self, :https
    # Allow inline styles with nonce
    policy.style_src   :self, :https, :unsafe_inline
    policy.connect_src :self, :https, 'http://localhost:*', 'https://localhost:*'
    policy.media_src   :self, :data, 'blob:'

    # Specify URI for violation reports (optional)
    # policy.report_uri "/csp-violation-report-endpoint"
  end

  # Generate session nonces for permitted importmap, inline scripts, and inline styles.
  config.content_security_policy_nonce_generator = ->(request) { request.session.id.to_s }
  config.content_security_policy_nonce_directives = %w(script-src)

  # Report violations without enforcing the policy in development
  config.content_security_policy_report_only = Rails.env.development?
end
