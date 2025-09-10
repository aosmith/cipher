require "test_helper"

class UserTest < ActiveSupport::TestCase
  def setup
    @valid_attributes = {
      username: "testuser",
      public_key: "test_public_key_12345"
    }
  end

  test "should be valid with valid attributes" do
    user = User.new(@valid_attributes)
    assert user.valid?
  end

  test "should require username" do
    user = User.new(@valid_attributes.except(:username))
    assert_not user.valid?
    assert_includes user.errors[:username], "can't be blank"
  end

  test "should require unique username" do
    User.create!(@valid_attributes)
    duplicate_user = User.new(@valid_attributes)
    assert_not duplicate_user.valid?
    assert_includes duplicate_user.errors[:username], "is already taken. Please choose a different username."
  end

  test "should require public key" do
    user = User.new(@valid_attributes.except(:public_key))
    assert_not user.valid?
    # The validation message comes from the base error, not public_key field
    assert_includes user.errors[:base], "⚠️ Key generation failed. Please ensure JavaScript is enabled and try refreshing the page."
  end

  test "should require unique public key" do
    User.create!(@valid_attributes)
    duplicate_user = User.new(@valid_attributes.merge(username: "different_user"))
    assert_not duplicate_user.valid?
    assert_includes duplicate_user.errors[:public_key], "is already registered to another account. Please regenerate your keys."
  end

  test "should have many posts" do
    user = User.create!(@valid_attributes)
    assert_respond_to user, :posts
    assert user.posts.is_a?(ActiveRecord::Associations::CollectionProxy)
  end

  test "should have many peers" do
    user = User.create!(@valid_attributes)
    assert_respond_to user, :peers
    assert user.peers.is_a?(ActiveRecord::Associations::CollectionProxy)
  end

  test "should have friendship associations" do
    user = User.create!(@valid_attributes)
    assert_respond_to user, :sent_friendships
    assert_respond_to user, :received_friendships
    assert_respond_to user, :friends_as_requester
    assert_respond_to user, :friends_as_addressee
  end

  test "should destroy dependent records" do
    user = User.create!(@valid_attributes)

    # Create a post
    post = user.posts.create!(content: "test content")

    assert_difference [ "User.count", "Post.count" ], -1 do
      user.destroy
    end
  end

  test "generate_keypair should validate public key presence" do
    user = User.new(@valid_attributes.except(:public_key))

    # Should not be valid without public key
    assert_not user.valid?
    assert_includes user.errors[:base], "⚠️ Key generation failed. Please ensure JavaScript is enabled and try refreshing the page."
  end
end
