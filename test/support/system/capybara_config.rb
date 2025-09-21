require "capybara"
require "selenium/webdriver"

Capybara.register_driver :selenium_chrome do |app|
  options = ::Selenium::WebDriver::Chrome::Options.new
  options.add_argument("--disable-backgrounding-occluded-windows")
  options.add_argument("--window-size=1400,1400")
  options.add_argument("--disable-popup-blocking")
  options.add_argument("--disable-dev-shm-usage")
  options.add_argument("--disable-infobars")
  options.add_argument("--remote-allow-origins=*")
  options.add_argument("--disable-renderer-backgrounding")
  options.add_argument("--disable-component-update")
  options.add_argument("--disable-permissions-api")
  options.add_argument("--no-first-run")
  options.add_argument("--no-default-browser-check")
  options.add_argument("--mute-audio")
  options.add_argument("--disable-background-networking")
  options.add_argument("--disable-sync")
  options.add_argument("--disable-translate")
  options.add_argument("--disable-extensions")
  options.add_argument("--metrics-recording-only")
  options.add_argument("--disable-site-isolation-trials")
  options.add_argument("--disable-features=PrivacySandboxSettings4,ReducedReferrerGranularity,TranslateUI")
  options.add_argument("--disable-notifications")
  options.add_argument("--window-position=-32000,-32000")
  options.add_preference("credentials_enable_service", false)
  options.add_preference("profile.password_manager_enabled", false)
  options.add_preference("profile.default_content_setting_values.notifications", 2)
  options.add_argument("--disable-gpu") if ENV["CI"]

  Capybara::Selenium::Driver.new(app, browser: :chrome, options: options)
end

Capybara.configure do |config|
  config.javascript_driver = :selenium_chrome
end
