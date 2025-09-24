require "test_helper"

class MobileBannerHelperTest < ActionView::TestCase
  include ApplicationHelper

  setup do
    # Set up controller context for helpers
    controller = ApplicationController.new
    controller.request = ActionDispatch::TestRequest.create
    @controller = controller

    # Setup view context
    view_context = ActionView::Base.new(ActionView::LookupContext.new([]), {}, controller)
    view_context.extend ApplicationHelper
    @view = view_context
  end

  test "mobile_browser? detects Android devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36"
    assert mobile_browser?, "Should detect Android as mobile browser"
  end

  test "mobile_browser? detects iPhone devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15"
    assert mobile_browser?, "Should detect iPhone as mobile browser"
  end

  test "mobile_browser? detects iPad devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (iPad; CPU OS 14_0 like Mac OS X) AppleWebKit/605.1.15"
    assert mobile_browser?, "Should detect iPad as mobile browser"
  end

  test "mobile_browser? detects BlackBerry devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (BlackBerry; U; BlackBerry 9900; en) AppleWebKit/534.11"
    assert mobile_browser?, "Should detect BlackBerry as mobile browser"
  end

  test "mobile_browser? does not detect desktop browsers" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
    assert_not mobile_browser?, "Should not detect desktop Safari as mobile browser"

    @request.headers["User-Agent"] = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
    assert_not mobile_browser?, "Should not detect desktop Chrome as mobile browser"
  end

  test "android_device? detects Android devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36"
    assert android_device?, "Should detect Android device"
  end

  test "android_device? does not detect iOS devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15"
    assert_not android_device?, "Should not detect iPhone as Android device"
  end

  test "ios_device? detects iPhone devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15"
    assert ios_device?, "Should detect iPhone as iOS device"
  end

  test "ios_device? detects iPad devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (iPad; CPU OS 14_0 like Mac OS X) AppleWebKit/605.1.15"
    assert ios_device?, "Should detect iPad as iOS device"
  end

  test "ios_device? detects iPod devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (iPod touch; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15"
    assert ios_device?, "Should detect iPod as iOS device"
  end

  test "ios_device? does not detect Android devices" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36"
    assert_not ios_device?, "Should not detect Android as iOS device"
  end

  test "should_show_mobile_install_banner? returns true for mobile browsers" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36"
    # Ensure cookies are clean for this test
    cookies.clear
    assert should_show_mobile_install_banner?, "Should show banner for mobile browsers"
  end

  test "should_show_mobile_install_banner? returns false for desktop browsers" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
    assert_not should_show_mobile_install_banner?, "Should not show banner for desktop browsers"
  end

  test "should_show_mobile_install_banner? returns false when banner is dismissed" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36"
    cookies[:hide_mobile_banner] = "true"
    assert_not should_show_mobile_install_banner?, "Should not show banner when dismissed"
  end

  test "should_show_mobile_install_banner? returns false in mobile app environments" do
    @request.headers["User-Agent"] = "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36"

    # Mock Rails.env for different environments
    Rails.env.define_singleton_method(:android?) { true }
    assert_not should_show_mobile_install_banner?, "Should not show banner in Android app"

    Rails.env.define_singleton_method(:android?) { false }
    Rails.env.define_singleton_method(:ios?) { true }
    assert_not should_show_mobile_install_banner?, "Should not show banner in iOS app"

    Rails.env.define_singleton_method(:ios?) { false }
    Rails.env.define_singleton_method(:desktop?) { true }
    assert_not should_show_mobile_install_banner?, "Should not show banner in desktop app"
  end
end