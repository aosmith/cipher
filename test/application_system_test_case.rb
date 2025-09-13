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

    # Use the app's natural authentication flow through the UI
    visit root_path

    # If already logged in, skip
    return if has_text?("Hi, #{user.username}", wait: 1)

    # Fill in login form (the app's homepage shows email/password form)
    fill_in "Email", with: "#{user.username}@test.com"
    fill_in "Password", with: "testpassword123"

    # Use JavaScript to handle the login with the user's actual credentials
    page.execute_script("
      // Simulate the app's client-side login process
      document.getElementById('loginEmail').value = '#{user.username}@test.com';
      document.getElementById('loginPassword').value = 'testpassword123';

      // Trigger form submission with proper authentication
      fetch('/api/v1/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Requested-With': 'XMLHttpRequest',
          'X-CSRF-Token': document.querySelector('meta[name=\"csrf-token\"]')?.getAttribute('content') || ''
        },
        body: JSON.stringify({
          username: '#{user.username}',
          public_key: '#{user.public_key}'
        })
      }).then(response => response.json()).then(data => {
        if (data.success) {
          window.location.reload();
        }
      });
    ")

    # Wait for the page to reload and show logged-in state
    sleep(2)

    # Verify authentication was successful
    assert_text "Hi, #{user.username}", wait: 5
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
