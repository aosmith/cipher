require "application_system_test_case"

class DesktopAppTest < ApplicationSystemTestCase
  def setup
    super
    # Skip this test if not running in desktop mode
    skip "Desktop tests only run when DESKTOP_TEST=true" unless ENV["DESKTOP_TEST"]

    # Configure Capybara for desktop app testing
    if ENV["DESKTOP_TEST"]
      # Don't start Capybara's own server - connect to desktop app instead
      Capybara.run_server = false
      Capybara.app_host = "http://127.0.0.1:3000"

      # Wait for desktop app to be ready
      wait_for_desktop_app
    end
  end

  test "desktop app launches and loads homepage" do
    visit "/"

    # Should see the main Cipher interface
    assert_text "Cipher"
    assert_text "Create Account"
    assert_text "Sign In"
  end

  test "desktop app account creation works" do
    visit "/"

    # Wait for homepage to load and verify it's working
    assert_text "Cipher"

    # Navigate to create account
    click_link "Create Account"

    # Fill in the account creation form using CSS selectors for native app
    fill_in "Username", with: "desktopuser"
    fill_in "Display name", with: "Desktop User"
    fill_in "Password", with: "securepass123"
    fill_in "Confirm Password", with: "securepass123"

    # Submit the form
    click_button "🚀 Create Account"

    # Should redirect to dashboard without database errors
    assert_text "Welcome to Cipher"
    assert_text "Your Cryptographic Identity"

    # For native app, we can't directly access the database from the test
    # Just verify the UI shows success
  end

  test "desktop app can create posts" do
    # First create a user
    visit "/"
    assert_text "Cipher"
    click_link "Create Account"

    fill_in "Username", with: "postuser"
    fill_in "Display name", with: "Post User"
    fill_in "Password", with: "securepass123"
    fill_in "Confirm Password", with: "securepass123"
    click_button "🚀 Create Account"

    # Navigate to create post
    visit "/posts/new"

    fill_in "Title", with: "Test Desktop Post"
    fill_in "Content", with: "This post was created from the desktop app!"

    click_button "Create Post"

    # Should see the post
    assert_text "Test Desktop Post"
    assert_text "This post was created from the desktop app!"
  end

  private

  def wait_for_desktop_app
    # Wait up to 30 seconds for the desktop app to be ready
    attempts = 0
    begin
      visit "/"
      return if page.has_text?("Cipher", wait: 1)
    rescue => e
      attempts += 1
      if attempts < 30
        sleep 1
        retry
      else
        raise "Desktop app did not start within 30 seconds: #{e.message}"
      end
    end
  end
end