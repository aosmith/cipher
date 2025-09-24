require "application_system_test_case"

class MobileInstallBannerTest < ApplicationSystemTestCase
  setup do
    # Create a test user for some test scenarios
    @user = User.create!(
      username: "testuser",
      email: "test@example.com",
      public_key: "test_public_key_123",
      email_verified_at: Time.current
    )
  end

  test "mobile banner shows on mobile devices" do
    # Spoof Android user agent using Chrome DevTools Protocol
    page.driver.browser.execute_cdp(
      'Network.setUserAgentOverride',
      userAgent: 'Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/80.0.3987.162 Mobile Safari/537.36'
    )

    visit root_path

    # Check if mobile banner is visible
    assert_selector "#mobile-install-banner", visible: true
    assert_text "Get the Cipher Mobile App"
    assert_text "Experience secure, decentralized communication on your mobile device"

    # Check for Play Store link
    assert_text "Get on Play Store"
  end

  test "mobile banner shows iOS message for iOS devices" do
    # Spoof iOS user agent
    page.driver.browser.execute_cdp(
      'Network.setUserAgentOverride',
      userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.0 Mobile/15E148 Safari/604.1'
    )

    visit root_path

    # Check if mobile banner is visible
    assert_selector "#mobile-install-banner", visible: true
    assert_text "Get the Cipher Mobile App"

    # Check for App Store link
    assert_text "Get on App Store"
  end

  test "mobile banner shows both options for generic mobile" do
    # Spoof generic mobile user agent
    page.driver.browser.execute_cdp(
      'Network.setUserAgentOverride',
      userAgent: 'Mozilla/5.0 (Mobile; rv:40.0) Gecko/40.0 Firefox/40.0'
    )

    visit root_path

    # Check if mobile banner is visible
    assert_selector "#mobile-install-banner", visible: true

    # Check for both store options
    assert_text "Play Store"
    assert_text "App Store"
  end

  test "mobile banner does not show on desktop" do
    # Use default desktop user agent
    visit root_path

    # Mobile banner should not be present
    assert_no_selector "#mobile-install-banner", visible: true
  end

  test "mobile banner can be dismissed" do
    # Spoof Android user agent
    page.driver.browser.execute_cdp(
      'Network.setUserAgentOverride',
      userAgent: 'Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36'
    )

    visit root_path

    # Banner should be visible initially
    assert_selector "#mobile-install-banner", visible: true

    # Click the close button
    find(".mobile-banner-close").click

    # Banner should be hidden after clicking close
    assert_selector "#mobile-install-banner.hidden", visible: false
  end

  test "mobile banner respects dismissed cookie" do
    # Visit the page first to establish domain context
    visit root_path

    # Set the cookie that indicates user has dismissed the banner
    page.driver.browser.manage.add_cookie(name: "hide_mobile_banner", value: "true", domain: "127.0.0.1")

    # Spoof Android user agent
    page.driver.browser.execute_cdp(
      'Network.setUserAgentOverride',
      userAgent: 'Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36'
    )

    # Visit the page again with the cookie and spoofed user agent
    visit root_path

    # Banner should not be visible when cookie is set
    assert_no_selector "#mobile-install-banner", visible: true
  end


  test "mobile banner shows appropriate coming soon messages" do
    # Spoof Android user agent
    page.driver.browser.execute_cdp(
      'Network.setUserAgentOverride',
      userAgent: 'Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36'
    )

    visit root_path

    # Click on the Play Store button and check alert
    accept_alert "Coming soon to Google Play Store!" do
      find("a", text: "Get on Play Store").click
    end
  end

  test "mobile banner styling is applied correctly" do
    # Spoof iOS user agent
    page.driver.browser.execute_cdp(
      'Network.setUserAgentOverride',
      userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15'
    )

    visit root_path

    # Check that banner has correct CSS classes and structure
    assert_selector ".mobile-install-banner"
    assert_selector ".mobile-banner-content"
    assert_selector ".mobile-banner-icon"
    assert_selector ".mobile-banner-text"
    assert_selector ".mobile-banner-actions"
    assert_selector ".mobile-banner-close"

    # Check that the emoji icon is present
    assert_text "📱"
  end

end