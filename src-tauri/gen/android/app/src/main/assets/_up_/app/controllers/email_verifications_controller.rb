class EmailVerificationsController < ApplicationController
  before_action :require_user_session
  before_action :find_user_by_email, only: [ :verify ]

  def show
    # Show the verification form
    @user = current_user_session
  end

  def verify
    verification_code = params[:verification_code]

    if @user.verify_email_with_code(verification_code)
      redirect_to root_path, notice: "Email verified successfully! You can now use all features."
    else
      @error = "Invalid or expired verification code. Please try again."
      render :show, status: :unprocessable_content
    end
  end

  def resend
    @user = current_user_session

    if @user.email_verified?
      redirect_to root_path, notice: "Email is already verified."
    else
      @user.resend_verification_code
      redirect_to email_verification_path, notice: "Verification code resent to #{@user.email}"
    end
  end

  private

  def require_user
    unless current_user_session
      redirect_to root_path, alert: "Please log in first"
    end
  end

  def find_user_by_email
    @user = current_user_session

    unless @user
      redirect_to root_path, alert: "User not found"
    end
  end

  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end
end
