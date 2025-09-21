require "test_helper"

class MessagesControllerTest < ActionController::TestCase
  def setup
    @alice = users(:alice)
    @bob = users(:bob)
    login_as(@alice)
  end

  test "should get index when logged in" do
    get :index
    assert_response :success
  end

  test "should redirect to login when not logged in for index" do
    logout
    get :index
    assert_redirected_to root_path
  end

  test "should show conversation with specific user" do
    get :show, params: { user_id: @bob.id }
    assert_response :success
  end

  test "should create message" do
    assert_difference("Message.count", 1) do
      post :create, params: {
        message: { content: "Test message" },
        user_id: @bob.id
      }
    end
    assert_redirected_to user_message_path(@bob)
  end

  test "should not create message without login" do
    logout
    assert_no_difference("Message.count") do
      post :create, params: {
        message: { content: "Test message" },
        user_id: @bob.id
      }
    end
    assert_redirected_to root_path
  end

  test "should destroy own message" do
    message = Message.create!(sender: @alice, recipient: @bob, content: "Test")
    assert_difference("Message.count", -1) do
      delete :destroy, params: { id: message.id }
    end
  end

  test "should not destroy other user's message" do
    message = Message.create!(sender: @bob, recipient: @alice, content: "Test")
    assert_no_difference("Message.count") do
      delete :destroy, params: { id: message.id }
    end
  end

  private

  def login_as(user)
    session[:user_id] = user.id
  end

  def logout
    session.delete(:user_id)
  end
end
