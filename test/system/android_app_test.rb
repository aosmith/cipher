require "application_system_test_case"

class AndroidAppTest < ApplicationSystemTestCase
  test "public/index.html exists and contains proper mobile interface" do
    index_path = Rails.root.join("public", "index.html")
    
    assert File.exist?(index_path), "index.html must exist in public directory"
    
    content = File.read(index_path)
    
    # Verify it doesn't reference localhost:3001
    refute_includes content, "localhost:3001", "index.html should not reference localhost:3001"
    refute_includes content, "http://localhost", "index.html should not reference localhost URLs"
    
    # Verify it contains essential mobile app elements
    assert_includes content, "Cipher", "Must contain app name"
    assert_includes content, "End-to-End Encrypted", "Must contain encryption messaging"
    assert_includes content, "loginForm", "Must contain login form"
    assert_includes content, "registerForm", "Must contain registration form"
    assert_includes content, "dark", "Must have dark theme"
    assert_includes content, "Mobile App", "Must indicate mobile app status"
    
    # Verify essential JavaScript functionality exists
    assert_includes content, "toggleForm", "Must have form toggle function"
    assert_includes content, "localStorage", "Must use local storage"
    assert_includes content, "showMainApp", "Must have main app function"
  end
  
  test "tauri.conf.json is configured correctly for mobile" do
    config_path = Rails.root.join("src-tauri", "tauri.conf.json")
    
    assert File.exist?(config_path), "tauri.conf.json must exist"
    
    content = File.read(config_path)
    config = JSON.parse(content)
    
    # Verify build configuration doesn't use devUrl for mobile
    refute config.dig("build", "devUrl"), "devUrl should not be set for mobile builds"
    
    # Verify frontendDist points to correct directory
    assert_equal "../public", config.dig("build", "frontendDist"), "frontendDist should point to ../public"
    
    # Verify bundle resources include Rails files for self-contained app
    resources = config.dig("bundle", "resources")
    assert_includes resources, "../app", "Must bundle Rails app directory"
    assert_includes resources, "../config", "Must bundle Rails config directory"
    assert_includes resources, "../Gemfile", "Must bundle Gemfile"
  end
  
  test "generated Android config uses static files not localhost" do
    android_config_path = Rails.root.join("src-tauri", "gen", "android", "app", "src", "main", "assets", "tauri.conf.json")
    
    if File.exist?(android_config_path)
      content = File.read(android_config_path)
      config = JSON.parse(content)
      
      # Check that the app window URL points to index.html, not localhost
      window_config = config.dig("app", "windows", 0)
      if window_config && window_config["url"]
        assert_equal "index.html", window_config["url"], "Android app should load index.html directly"
      end
      
      # Verify build config doesn't point to localhost in generated config
      build_config = config["build"]
      if build_config
        if build_config["devUrl"]
          refute_includes build_config["devUrl"], "localhost:3001", "Generated config should not use localhost:3001"
        end
        if build_config["frontendDist"] 
          refute_includes build_config["frontendDist"], "localhost:3001", "Generated frontendDist should not use localhost:3001"
        end
      end
    else
      skip "Android project not generated yet"
    end
  end
  
  test "mobile.rs contains correct configuration" do
    mobile_rs_path = Rails.root.join("src-tauri", "src", "mobile.rs")
    
    assert File.exist?(mobile_rs_path), "mobile.rs must exist"
    
    content = File.read(mobile_rs_path)
    
    # Verify it doesn't try to start Rails server
    refute_includes content, "rails server", "mobile.rs should not start Rails server"
    refute_includes content, "bin/rails", "mobile.rs should not reference bin/rails"
    refute_includes content, "localhost:3001", "mobile.rs should not reference localhost:3001"
    
    # Verify it has mobile entry point
    assert_includes content, "tauri::mobile_entry_point", "Must have mobile entry point"
    assert_includes content, "Mobile app running with bundled assets", "Must indicate bundled assets"
  end
  
  test "APK can be built without localhost references" do
    # This test simulates what the build process should do
    
    # Verify the beforeBuildCommand would work
    build_command = "RAILS_ENV=production rails assets:precompile"
    
    # Check that assets get precompiled to public directory
    system("cd #{Rails.root} && #{build_command}")
    
    # Verify assets were created in public
    assert Dir.exist?(Rails.root.join("public", "assets")), "Assets should be precompiled to public/assets"
    
    # Verify index.html is still there and valid
    index_path = Rails.root.join("public", "index.html") 
    assert File.exist?(index_path), "index.html should exist after asset precompilation"
    
    content = File.read(index_path)
    refute_includes content, "localhost:3001", "index.html should not reference localhost after build"
  end
  
  test "can test mobile app functionality using browser" do
    # Open the actual index.html file to test the interface
    visit "file://#{Rails.root.join('public', 'index.html')}"
    
    # Verify the page loads without localhost errors
    assert_no_text "localhost:3001"
    assert_no_text "ERR_CONNECTION_REFUSED"
    
    # Verify essential elements are present
    assert_text "Cipher"
    assert_text "End-to-End Encrypted"
    
    # Verify forms are present - wait a moment for JavaScript to initialize
    sleep(0.5)
    assert_selector "#loginForm"

    # The registerForm should start hidden unless someone is already logged in
    if has_selector?("#registerForm.hidden")
      assert_selector "#registerForm.hidden"  # Should start hidden
    else
      # If not hidden, it means someone is logged in - clear localStorage and reload
      page.execute_script("localStorage.clear(); location.reload();")
      sleep(0.5)
      assert_selector "#registerForm.hidden"
    end
    
    # Test form toggle
    click_link "Need an account? Sign up"
    assert_selector "#loginForm.hidden"
    assert_selector "#registerForm:not(.hidden)"
    
    # Test registration flow
    fill_in "registerEmail", with: "test@example.com"
    fill_in "registerPassword", with: "password123"
    fill_in "confirmPassword", with: "password123"
    
    click_button "Create Account"
    
    # Should show success and main app
    assert_text "Welcome to Cipher", wait: 3
    assert_text "test@example.com"
  end
end