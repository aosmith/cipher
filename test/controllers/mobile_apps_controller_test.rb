require "test_helper"

class MobileAppsControllerTest < ActionDispatch::IntegrationTest
  test "should serve APK file when available" do
    # Create a mock APK file for testing
    apk_dir = Rails.root.join("src-tauri/gen/android/app/build/outputs/apk/universal/debug")
    FileUtils.mkdir_p(apk_dir) unless Dir.exist?(apk_dir)
    apk_path = apk_dir.join("app-universal-debug.apk")

    File.write(apk_path, "Mock APK content for testing")

    begin
      get "/cipher.apk"

      assert_response :success
      assert_equal "application/vnd.android.package-archive", @response.content_type
      assert_match(/attachment.*filename="Cipher.apk"/, @response.headers["Content-Disposition"])
      assert_equal "Mock APK content for testing", @response.body
    ensure
      # Clean up test file
      File.delete(apk_path) if File.exist?(apk_path)
    end
  end

  test "should return 404 when no APK file available" do
    # Ensure no APK files exist for this test
    apk_paths = [
      "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-signed.apk",
      "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk",
      "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk"
    ]

    # Temporarily rename any existing APK files
    backup_paths = []
    apk_paths.each do |apk_path|
      full_path = Rails.root.join(apk_path)
      if File.exist?(full_path)
        backup_path = "#{full_path}.backup"
        File.rename(full_path, backup_path)
        backup_paths << [full_path, backup_path]
      end
    end

    begin
      get "/cipher.apk"

      assert_response :not_found
      assert_match(/Android app not available yet/, @response.body)
    ensure
      # Restore any backed up files
      backup_paths.each do |original, backup|
        File.rename(backup, original) if File.exist?(backup)
      end
    end
  end

  test "should set correct content type and headers for APK download" do
    # Create a temporary APK file for testing
    apk_dir = Rails.root.join("src-tauri/gen/android/app/build/outputs/apk/universal/debug")
    FileUtils.mkdir_p(apk_dir)
    apk_path = apk_dir.join("app-universal-debug.apk")

    File.write(apk_path, "Test APK content")

    begin
      get "/cipher.apk"

      assert_response :success
      assert_equal "application/vnd.android.package-archive", @response.content_type
      assert_match(/attachment.*filename="Cipher.apk"/, @response.headers["Content-Disposition"])
    ensure
      File.delete(apk_path) if File.exist?(apk_path)
    end
  end

  test "should prioritize signed release APK over debug APK" do
    # Create both debug and signed release APKs
    debug_dir = Rails.root.join("src-tauri/gen/android/app/build/outputs/apk/universal/debug")
    release_dir = Rails.root.join("src-tauri/gen/android/app/build/outputs/apk/universal/release")

    FileUtils.mkdir_p(debug_dir)
    FileUtils.mkdir_p(release_dir)

    debug_apk = debug_dir.join("app-universal-debug.apk")
    signed_apk = release_dir.join("app-universal-release-signed.apk")

    File.write(debug_apk, "Debug APK content")
    File.write(signed_apk, "Signed Release APK content")

    begin
      get "/cipher.apk"

      assert_response :success
      # Should serve the signed release APK, not the debug one
      assert_equal "Signed Release APK content", @response.body
    ensure
      File.delete(debug_apk) if File.exist?(debug_apk)
      File.delete(signed_apk) if File.exist?(signed_apk)
    end
  end
end