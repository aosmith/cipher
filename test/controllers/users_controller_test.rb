require "test_helper"

class UsersControllerTest < ActionDispatch::IntegrationTest
  test "should get index" do
    get users_url
    assert_response :success
  end

  test "should get show" do
    user = User.create!(username: "testuser", public_key: "test_public_key_12345")
    get user_url(user)
    assert_response :success
  end

  test "should get new" do
    get new_user_url
    assert_response :success
  end

  test "should create user with valid params" do
    user_params = {
      user: {
        username: "newuser",
        display_name: "New User"
      },
      password: "securepassword123",
      confirm_password: "securepassword123"
    }
    
    assert_difference "User.count", 1 do
      post users_url, params: user_params
    end
    
    assert_redirected_to dashboard_users_path
    
    # Verify the user was created with a generated public key
    user = User.find_by(username: "newuser")
    assert_not_nil user
    assert_not_empty user.public_key
  end
end
