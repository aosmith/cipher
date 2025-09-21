require "application_system_test_case"

class UserRegistrationTest < ApplicationSystemTestCase
  setup do
    # Clean up any existing records to ensure test isolation
    # Need to clean up dependent records first due to foreign key constraints
    AttachmentShare.destroy_all
    Attachment.destroy_all
    SyncMessage.destroy_all
    Friendship.destroy_all
    Peer.destroy_all
    Post.destroy_all
    User.destroy_all
  end

  test "successful user registration with server-side key generation" do
    # Visit the user registration page
    visit new_user_path
    
    # Verify the page loaded correctly
    assert_selector "h2", text: "Create Your Cipher Identity"
    assert_selector "form.user-form"
    
    # Fill in the form fields
    fill_in "Username", with: "testuser123"
    fill_in "Display name", with: "Test User"
    fill_in "Password", with: "securepassword123"
    fill_in "Confirm Password", with: "securepassword123"
    
    # Submit the form
    click_button "Create Account"
    
    # Verify successful redirect to dashboard
    assert_current_path dashboard_users_path
    assert_text "Welcome to Cipher, Test User!"
    
    # Verify user was created in database with generated keys
    user = User.find_by(username: "testuser123")
    assert_not_nil user
    assert_equal "Test User", user.display_name
    assert_not_empty user.public_key
    
    # Verify public key is valid base64 and proper length
    decoded_key = Base64.decode64(user.public_key)
    assert_equal 32, decoded_key.length, "Public key should be 32 bytes when decoded"
  end

  test "password validation errors are displayed" do
    visit new_user_path
    
    fill_in "Username", with: "testuser456"
    fill_in "Password", with: "short"
    fill_in "Confirm Password", with: "short"
    
    click_button "Create Account"
    
    assert_text "Password must be at least 8 characters long"
  end

  test "password mismatch validation" do
    visit new_user_path
    
    fill_in "Username", with: "testuser789"
    fill_in "Password", with: "securepassword123"
    fill_in "Confirm Password", with: "differentpassword"
    
    click_button "Create Account"
    
    assert_text "Password and confirmation do not match"
  end

  test "username validation errors" do
    visit new_user_path
    
    # Test duplicate username (create a user first)
    User.create!(username: "existinguser", display_name: "Existing", public_key: "test_key")
    
    fill_in "Username", with: "existinguser"
    fill_in "Password", with: "securepassword123"
    fill_in "Confirm Password", with: "securepassword123"
    
    click_button "Create Account"
    
    assert_text "Username is already taken. Please choose a different username."
  end

  test "empty fields validation" do
    visit new_user_path
    
    # Leave fields empty and submit
    click_button "Create Account"
    
    assert_text "Password and confirmation are required"
  end

  test "form displays correctly" do
    visit new_user_path
    
    # Verify all expected form elements are present
    assert_selector "input[name='user[username]']"
    assert_selector "input[name='user[display_name]']"
    assert_selector "input[name='password']"
    assert_selector "input[name='confirm_password']"
    assert_selector "input[type='submit'][value='🚀 Create Account']"
    
    # Verify informational content
    assert_text "Automatic Key Generation"
    assert_text "What happens next?"
  end
end
