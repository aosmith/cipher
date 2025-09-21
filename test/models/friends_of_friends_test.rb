require "test_helper"

class FriendsOfFriendsTest < ActiveSupport::TestCase
  def setup
    @alice = users(:alice)
    @bob = users(:bob)
    @david = users(:david)

    # Create a new user who will be a friend of friends
    @eve = User.create!(
      username: "eve",
      display_name: "Eve",
      public_key: "eve_public_key"
    )

    # Setup friendship network:
    # Alice <-> Bob <-> David
    # Alice <-> Eve (so Eve and Bob are friends of friends)

    # Alice and Bob are already friends from fixtures

    # Add Bob <-> David (already in fixtures)

    # Add Alice <-> Eve
    unless Friendship.exists?(requester: @alice, addressee: @eve, status: "accepted")
      @alice.sent_friendships.create!(addressee: @eve, status: "accepted")
    end
    unless Friendship.exists?(requester: @eve, addressee: @alice, status: "accepted")
      @eve.sent_friendships.create!(addressee: @alice, status: "accepted")
    end
  end

  test "should identify friends of friends correctly" do
    # Alice's direct friends: Bob, Eve
    assert @alice.friends_with?(@bob)
    assert @alice.friends_with?(@eve)
    assert_not @alice.friends_with?(@david)

    # Alice and David should be friends of friends (through Bob)
    assert @alice.friends_of_friends_with?(@david)

    # Bob and Eve should be friends of friends (through Alice)
    assert @bob.friends_of_friends_with?(@eve)
    assert @eve.friends_of_friends_with?(@bob)

    # Direct friends should not be considered friends of friends
    assert_not @alice.friends_of_friends_with?(@bob) # Direct friends
    assert_not @bob.friends_of_friends_with?(@alice) # Direct friends
  end

  test "should allow sync between friends of friends" do
    # David creates a post
    david_post = @david.posts.create!(
      content: "Hello from David!",
      is_synced: false,
      original_user_id: @david.id
    )

    # Alice should be able to sync David's posts (they are friends of friends through Bob)
    assert david_post.can_be_synced_by?(@alice)

    # Create a synced post on Alice's server from David
    synced_post = @alice.posts.create!(
      content_encrypted: david_post.content_encrypted,
      is_synced: true,
      original_user_id: @david.id,
      synced_from_user_id: @bob.id, # Bob is the mutual friend
      synced_at: Time.current,
      content_hash: Digest::SHA256.hexdigest(david_post.content_encrypted)
    )

    assert synced_post.valid?
    assert synced_post.is_synced?
    assert_equal @david.id, synced_post.original_user_id
    assert_equal @alice, synced_post.user
  end

  test "friends_of_friends query should return correct users" do
    friends_of_friends = @alice.friends_of_friends.to_a

    # Alice should see David as a friend of a friend (through Bob)
    assert_includes friends_of_friends, @david

    # But not direct friends Bob or Eve
    assert_not_includes friends_of_friends, @bob
    assert_not_includes friends_of_friends, @eve

    # And not self
    assert_not_includes friends_of_friends, @alice
  end
end
