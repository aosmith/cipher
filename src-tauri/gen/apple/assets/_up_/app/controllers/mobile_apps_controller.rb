class MobileAppsController < ApplicationController
  def download_android
    # Find the most recent signed APK file
    apk_paths = [
      Rails.root.join("src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-signed.apk"),
      Rails.root.join("src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"),
      Rails.root.join("src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk")
    ]

    apk_path = apk_paths.find { |path| File.exist?(path) }

    if apk_path && File.exist?(apk_path)
      send_file apk_path,
                type: 'application/vnd.android.package-archive',
                disposition: 'attachment',
                filename: 'Cipher.apk'
    else
      render plain: "Android app not available yet. Please check back later.", status: 404
    end
  end
end