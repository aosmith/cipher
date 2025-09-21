require "test_helper"

class EmailVerificationTest < ActiveSupport::TestCase
  def setup
    @user = User.new(
      username: "testuser",
      display_name: "Test User",
      public_key: "test_public_key_123",
      email: "test@example.com"
    )
  end

  test "should generate verification code on create" do
    assert @user.save
    assert_not_nil @user.verification_code
    assert @user.verification_code.length == 6
    assert @user.verification_code.match?(/\A[A-Z0-9]+\z/) # Alphanumeric uppercase
    assert_not_nil @user.verification_code_expires_at
    assert @user.verification_code_expires_at > Time.current
  end

  test "should not be verified initially" do
    @user.save
    assert_not @user.email_verified?
    assert_nil @user.email_verified_at
  end

  test "should verify email with correct code" do
    @user.save
    code = @user.verification_code

    result = @user.verify_email_with_code(code)

    assert result
    assert @user.email_verified?
    assert_not_nil @user.email_verified_at
    assert_nil @user.verification_code
    assert_nil @user.verification_code_expires_at
  end

  test "should not verify email with incorrect code" do
    @user.save

    result = @user.verify_email_with_code("WRONG1")

    assert_not result
    assert_not @user.email_verified?
    assert_not_nil @user.verification_code # Code should still be there
  end

  test "should not verify email with expired code" do
    @user.save
    code = @user.verification_code

    # Manually expire the code
    @user.update!(verification_code_expires_at: 1.minute.ago)

    result = @user.verify_email_with_code(code)

    assert_not result
    assert_not @user.email_verified?
  end

  test "should be case insensitive for verification code" do
    @user.save
    code = @user.verification_code.downcase

    result = @user.verify_email_with_code(code)

    assert result
    assert @user.email_verified?
  end

  test "should regenerate verification code when resending" do
    @user.save
    original_code = @user.verification_code
    original_expires_at = @user.verification_code_expires_at

    sleep(1) # Ensure time difference
    @user.resend_verification_code

    assert_not_equal original_code, @user.verification_code
    assert @user.verification_code_expires_at > original_expires_at
  end

  test "should find users by email" do
    @user.save

    results = User.search_by_email("test@example")
    assert_includes results, @user

    results = User.search_by_email("example.com")
    assert_includes results, @user

    results = User.search_by_email("nonexistent@email.com")
    assert_not_includes results, @user
  end

  test "should have verified and unverified scopes" do
    verified_user = User.create!(
      username: "verified_user",
      display_name: "Verified User",
      public_key: "verified_public_key",
      email: "verified@example.com"
    )
    verified_user.update!(email_verified_at: Time.current)

    unverified_user = User.create!(
      username: "unverified_user",
      display_name: "Unverified User",
      public_key: "unverified_public_key",
      email: "unverified@example.com"
    )

    verified_users = User.verified
    unverified_users = User.unverified

    assert_includes verified_users, verified_user
    assert_not_includes verified_users, unverified_user

    assert_includes unverified_users, unverified_user
    assert_not_includes unverified_users, verified_user
  end

  test "should validate email format" do
    @user.email = "invalid_email"
    assert_not @user.valid?
    assert_includes @user.errors[:email], "must be a valid email address"

    @user.email = "valid@email.com"
    assert @user.valid?
  end

  test "should enforce email uniqueness" do
    @user.save!

    duplicate_user = User.new(
      username: "different_user",
      display_name: "Different User",
      public_key: "different_public_key",
      email: "test@example.com" # Same email
    )

    assert_not duplicate_user.valid?
    assert_includes duplicate_user.errors[:email], "is already registered to another account."
  end
end
