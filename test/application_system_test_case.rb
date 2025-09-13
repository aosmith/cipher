require "test_helper"

class ApplicationSystemTestCase < ActionDispatch::SystemTestCase
  # Use regular Chrome for debugging, headless_chrome for CI
  driven_by :selenium, using: :chrome, screen_size: [ 1400, 1400 ]

  # Configure Selenium to support ES6 modules and modern JavaScript
  setup do
    # Add a small delay to ensure page fully loads
    Capybara.default_max_wait_time = 10
  end

  # SECURITY: Test-only authentication helper
  def login_as(user)
    raise "login_as helper is only for test environment" unless Rails.env.test?

    # For system tests with Selenium, we need to set up the session through the web interface
    # Since it's local, we can use a simple approach by visiting the login endpoint directly
    visit "/api/v1/login?user_id=#{user.id}&test_login=true"

    # Now visit the main page where the session should be active
    visit root_path

    # Verify authentication was successful
    assert_text "Hi, #{user.username}", wait: 10
  end

  # Helper to ensure user is logged out
  def logout_user
    visit root_path
    page.execute_script("
      fetch('/api/v1/logout', {
        method: 'POST',
        headers: {
          'X-Requested-With': 'XMLHttpRequest'
        }
      });
    ")
    sleep(0.5)
    visit root_path
  end
end
