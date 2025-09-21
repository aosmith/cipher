class ApplicationController < ActionController::Base
  # Only allow modern browsers supporting webp images, web push, badges, import maps, CSS nesting, and CSS :has.
  # Skip browser check in test environment to allow test runners
  allow_browser versions: :modern unless Rails.env.test?

  # CSRF protection for all controllers
  protect_from_forgery with: :exception

  protected

  def current_user_session
    # Test environment: also check for test_user_id cookie for system tests
    if Rails.env.test? && cookies[:test_user_id].present?
      @current_user_session ||= User.find_by(id: cookies[:test_user_id])
    elsif session[:user_id]
      @current_user_session ||= User.find_by(id: session[:user_id])
    end
  end

  helper_method :current_user_session

  def require_user_session
    return if current_user_session

    message = "Session expired. Please sign in again."
    session.delete(:user_id)
    cookies.delete(:test_user_id)
    redirect_to root_path, alert: message
  end
end
