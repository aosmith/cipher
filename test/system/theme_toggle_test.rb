require "application_system_test_case"

class ThemeToggleTest < ApplicationSystemTestCase
  test "user can toggle between light and dark themes" do
    visit root_path

    initial_theme = find("body")["data-theme"]
    assert_includes %w[dark light], initial_theme

    click_button "Toggle theme"

    expected_theme = initial_theme == "light" ? "dark" : "light"
    assert_selector "body[data-theme='#{expected_theme}']"
    toggled_theme = find("body")["data-theme"]
    expected_theme = initial_theme == "light" ? "dark" : "light"
    assert_equal expected_theme, toggled_theme
  end
end
