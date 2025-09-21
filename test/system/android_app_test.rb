require "application_system_test_case"

class AndroidAppTest < ApplicationSystemTestCase
  MOBILE_INDEX = Rails.root.join("public", "mobile-index.html")
  APP_HTML     = Rails.root.join("public", "app.html")

  test "mobile index ships bundled experience" do
    assert File.exist?(MOBILE_INDEX), "mobile-index.html must exist"

    content = File.read(MOBILE_INDEX)

    refute_includes content, "localhost", "mobile index should not reference localhost"
    assert_includes content, "Cipher", "App branding should be present"
    assert_includes content, "End-to-end encrypted", "Security messaging should be present"
    assert_includes content, "loginForm", "Login form markup should exist"
    assert_includes content, "registerForm", "Registration form markup should exist"
    assert_includes content, "toggleForm", "Form toggle logic should be bundled"
  end

  test "tauri.conf.json embeds bundled assets" do
    config_path = Rails.root.join("src-tauri", "tauri.conf.json")
    assert File.exist?(config_path), "tauri.conf.json must exist"

    config = JSON.parse(File.read(config_path))

    assert_equal "../public", config.dig("build", "frontendDist")
    assert_equal "http://localhost:3001", config.dig("build", "devUrl")

    resources = config.dig("bundle", "resources") || []
    assert_includes resources, "../app"
    assert_includes resources, "../config"
    assert_includes resources, "../Gemfile"
  end

  test "generated android config prefers static entry point" do
    android_config_path = Rails.root.join("src-tauri", "gen", "android", "app", "src", "main", "assets", "tauri.conf.json")

    skip "Android project not generated yet" unless File.exist?(android_config_path)

    config = JSON.parse(File.read(android_config_path))
    window_config = config.dig("app", "windows", 0) || {}

    if window_config["url"]
      assert_equal "index.html", window_config["url"], "Android build should load packaged index"
    end

    dev_url = config.dig("build", "devUrl")
    assert dev_url&.include?("localhost"), "Generated config uses loopback devUrl for development"
  end

  test "mobile.rs documents bundled startup" do
    mobile_rs = Rails.root.join("src-tauri", "src", "mobile.rs")
    assert File.exist?(mobile_rs)

    content = File.read(mobile_rs)
    refute_includes content, "http://localhost"
    assert_includes content, "mobile_entry_point"
    assert_includes content, "run_rails.sh"
  end

  test "static assets avoid localhost references" do
    [ MOBILE_INDEX, APP_HTML ].each do |file_path|
      next unless File.exist?(file_path)
      content = File.read(file_path)
      refute_includes content, "localhost", "#{file_path.basename} should not reference localhost"
    end
  end

  test "mobile shell markup includes required sections" do
    html = File.read(MOBILE_INDEX)

    assert_includes html, "<title>Cipher"
    assert_includes html, "id=\"loginForm\""
    assert_includes html, "id=\"registerForm\""
    assert_includes html, "Server is starting up, please wait..."
  end
end
