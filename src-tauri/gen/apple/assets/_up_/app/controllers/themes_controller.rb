class ThemesController < ApplicationController
  def toggle
    current_theme = cookies[:theme]
    new_theme = current_theme == "light" ? "dark" : "light"

    cookies[:theme] = {
      value: new_theme,
      expires: 1.year.from_now,
      same_site: :lax
    }

    redirect_back fallback_location: root_path
  end
end
