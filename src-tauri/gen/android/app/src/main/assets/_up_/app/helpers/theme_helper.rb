module ThemeHelper
  def detect_system_theme
    return @system_theme if defined?(@system_theme)

    @system_theme = case RbConfig::CONFIG['host_os']
    when /darwin/i
      # macOS
      result = `defaults read -g AppleInterfaceStyle 2>/dev/null`.strip
      result == "Dark" ? "dark" : "light"
    when /linux/i
      # Linux - check common desktop environments
      if ENV['DESKTOP_SESSION']&.include?('gnome') || ENV['XDG_CURRENT_DESKTOP']&.include?('GNOME')
        # GNOME
        result = `gsettings get org.gnome.desktop.interface gtk-theme 2>/dev/null`.strip
        result.downcase.include?('dark') ? "dark" : "light"
      elsif ENV['DESKTOP_SESSION']&.include?('kde') || ENV['XDG_CURRENT_DESKTOP']&.include?('KDE')
        # KDE
        result = `kreadconfig5 --group Colors --key ColorScheme 2>/dev/null`.strip
        result.downcase.include?('dark') ? "dark" : "light"
      else
        # Default to dark for unknown Linux environments
        "dark"
      end
    when /android/i
      # Android - for mobile Tauri builds
      # Android doesn't have a direct command, but we can try to detect through environment
      # Default to dark since most mobile users prefer dark mode
      "dark"
    when /ios/i
      # iOS - for mobile Tauri builds
      # iOS doesn't have direct command access in Tauri
      # Default to dark since most mobile users prefer dark mode
      "dark"
    else
      # Windows and other platforms
      if RbConfig::CONFIG['host_os'] =~ /mswin|mingw|cygwin/
        # Windows - check registry for dark mode
        result = `reg query "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize" /v AppsUseLightTheme 2>nul`.strip
        # If AppsUseLightTheme is 0, dark mode is enabled
        result.include?("0x0") ? "dark" : "light"
      else
        "dark" # Default fallback
      end
    end
  rescue
    # Fallback to dark theme if detection fails
    "dark"
  end

  def system_theme_class
    "data-theme-#{detect_system_theme}"
  end

  def current_theme
    # Check if user has manually set a theme preference
    theme_from_session = session[:theme]
    theme_from_cookie = cookies[:theme]

    # Priority: session > cookie > system detection
    theme_from_session || theme_from_cookie || detect_system_theme
  end
end