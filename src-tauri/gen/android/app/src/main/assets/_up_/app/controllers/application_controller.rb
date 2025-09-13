class ApplicationController < ActionController::Base
  # Only allow modern browsers supporting webp images, web push, badges, import maps, CSS nesting, and CSS :has.
  # Skip browser check in test environment to allow test runners
  allow_browser versions: :modern unless Rails.env.test?
  
  protected
  
  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end
  
  helper_method :current_user_session
end
