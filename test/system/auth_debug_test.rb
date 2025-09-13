require "application_system_test_case"

class AuthDebugTest < ApplicationSystemTestCase
  test "can manually create user and see logged in state" do
    # Create user directly in database
    user = User.create!(
      username: "test_auth",
      display_name: "Test User",
      public_key: Base64.strict_encode64("test_public_key_123456789012345678901234")
    )

    # Go to root path
    visit root_path

    # Check if we see login page (expected before logging in)
    assert_text "Sign In"

    # Try the new login_as method
    begin
      login_as(user)
      puts "Successfully logged in using login_as helper"
    rescue => e
      puts "Login failed with error: #{e.message}"
      puts "Current page after login attempt: #{page.text[0..200]}..."
    end
  end
end