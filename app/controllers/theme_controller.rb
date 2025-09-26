class ThemeController < ApplicationController
  include ThemeHelper

  def toggle
    current = current_theme
    new_theme = current == "dark" ? "light" : "dark"

    # Store theme preference in session and cookie
    session[:theme] = new_theme
    cookies[:theme] = { value: new_theme, expires: 1.year.from_now }

    redirect_back(fallback_location: root_path)
  end

  def set
    theme = params[:theme]

    if %w[light dark auto].include?(theme)
      if theme == "auto"
        # Clear manual preference to use system detection
        session.delete(:theme)
        cookies.delete(:theme)
      else
        session[:theme] = theme
        cookies[:theme] = { value: theme, expires: 1.year.from_now }
      end
    end

    redirect_back(fallback_location: root_path)
  end
end