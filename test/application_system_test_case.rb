require "test_helper"

class ApplicationSystemTestCase < ActionDispatch::SystemTestCase
  DRIVER = ENV.fetch("SYSTEM_TEST_DRIVER", "selenium_chrome").to_sym

  case DRIVER
  when :selenium_chrome
    driven_by :selenium, using: :chrome, screen_size: [1400, 900]
  when :selenium_chrome_headless
    driven_by :selenium, using: :chrome, screen_size: [1400, 900], options: { args: %w[headless disable-gpu no-sandbox disable-dev-shm-usage] }
  else
    driven_by DRIVER
  end

  setup do
    Capybara.default_max_wait_time = 5
  end

  # SECURITY: Test-only authentication helper
  def login_as(user)
    raise "login_as helper is only for test environment" unless Rails.env.test?

    # For system tests with Selenium, we need to set up the session through the web interface
    # Since it's local, we can use a simple approach by visiting the login endpoint directly
    visit "/api/v1/login?user_id=#{user.id}&test_login=true"
    visit root_path

    # Verify authentication was successful
    assert_text "Hi, #{user.username}", wait: 10
  end

  # Helper to ensure user is logged out
  def logout_user
    visit "/api/v1/logout?test_logout=true"
    visit root_path
  end
end
