class ThemesController < ApplicationController
  def toggle
    current_theme = session[:theme] || cookies[:theme] || "dark"
    new_theme = current_theme == "dark" ? "light" : "dark"

    session[:theme] = new_theme
    cookies[:theme] = { value: new_theme, expires: 1.year.from_now }

    redirect_back fallback_location: root_path
  end
end
