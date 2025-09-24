require "test_helper"

class MobileBannerIntegrationTest < ActionDispatch::IntegrationTest
  test "mobile banner shows for Android devices" do
    get "/", headers: { "User-Agent" => "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36" }

    assert_response :success
    assert_select "#mobile-install-banner", count: 1
    assert_select ".mobile-banner-text h3", text: "Get the Cipher Mobile App"
    assert_select "a", text: "Get on Play Store"
  end

  test "mobile banner shows for iOS devices" do
    get "/", headers: { "User-Agent" => "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15" }

    assert_response :success
    assert_select "#mobile-install-banner", count: 1
    assert_select "a", text: "Get on App Store"
  end

  test "mobile banner shows both options for generic mobile" do
    get "/", headers: { "User-Agent" => "Mozilla/5.0 (Mobile; rv:40.0) Gecko/40.0 Firefox/40.0" }

    assert_response :success
    assert_select "#mobile-install-banner", count: 1
    assert_select "a", text: "Play Store"
    assert_select "a", text: "App Store"
  end

  test "mobile banner does not show for desktop browsers" do
    get "/", headers: { "User-Agent" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36" }

    assert_response :success
    assert_select "#mobile-install-banner", count: 0
  end

  test "mobile banner does not show when dismissed via cookie" do
    cookies[:hide_mobile_banner] = "true"

    get "/", headers: { "User-Agent" => "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36" }

    assert_response :success
    assert_select "#mobile-install-banner", count: 0
  end

  test "mobile banner has close button" do
    get "/", headers: { "User-Agent" => "Mozilla/5.0 (Linux; Android 11; SM-G981B) AppleWebKit/537.36" }

    assert_response :success
    assert_select ".mobile-banner-close", count: 1
  end

  test "mobile banner has proper structure and styling" do
    get "/", headers: { "User-Agent" => "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) AppleWebKit/605.1.15" }

    assert_response :success
    assert_select ".mobile-install-banner" do
      assert_select ".mobile-banner-content" do
        assert_select ".mobile-banner-icon", text: "📱"
        assert_select ".mobile-banner-text" do
          assert_select "h3", text: "Get the Cipher Mobile App"
          assert_select "p", text: /secure.*decentralized/
        end
        assert_select ".mobile-banner-actions"
        assert_select ".mobile-banner-close"
      end
    end
  end
end