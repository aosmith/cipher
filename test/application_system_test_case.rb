require "test_helper"

class ApplicationSystemTestCase < ActionDispatch::SystemTestCase
  # Use regular Chrome for debugging, headless_chrome for CI
  driven_by :selenium, using: :chrome, screen_size: [ 1400, 1400 ]
  
  # Configure Selenium to support ES6 modules and modern JavaScript
  setup do
    # Add a small delay to ensure page fully loads
    Capybara.default_max_wait_time = 10
  end
end
