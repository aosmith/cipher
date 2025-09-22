require "application_system_test_case"

class MobileAccountCreationTest < ApplicationSystemTestCase
  test "mobile account creation redirects properly to dashboard" do
    # Simulate mobile environment
    Rails.application.config.hosts << "10.0.2.2"

    visit new_user_path

    # Fill in the form
    fill_in "Username", with: "mobileuser"
    fill_in "Display name", with: "Mobile User"
    fill_in "Password", with: "securepass123"
    fill_in "Confirm Password", with: "securepass123"

    # Submit the form
    click_button "🚀 Create Account"

    # Check that we're redirected to dashboard
    assert_current_path dashboard_users_path

    # Check that the dashboard content is visible
    assert_text "Welcome to Cipher, Mobile User!"
    assert_text "Your Cryptographic Identity"
    assert_text "Getting Started"

    # Check that user was created and logged in
    user = User.find_by(username: "mobileuser")
    assert_not_nil user
    assert_not_empty user.public_key
  end

  test "mobile account creation with android environment" do
    # Test specifically with Android environment
    original_env = Rails.env
    begin
      Rails.env = ActiveSupport::StringInquirer.new("android")

      visit new_user_path

      # Fill in the form
      fill_in "Username", with: "androiduser"
      fill_in "Display name", with: "Android User"
      fill_in "Password", with: "securepass123"
      fill_in "Confirm Password", with: "securepass123"

      # Submit the form
      click_button "🚀 Create Account"

      # Check that we're redirected to dashboard (not back to home)
      assert_current_path dashboard_users_path

      # Check that the dashboard content is visible
      assert_text "Welcome to Cipher, Android User!"

    ensure
      Rails.env = original_env
    end
  end
end