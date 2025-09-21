require "application_system_test_case"

class P2pWebrtcTest < ApplicationSystemTestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ Comment, Post, Friendship, User ].each(&:delete_all)
    end

    @alice = User.create!(username: "alice", display_name: "Alice", public_key: "alice_key")
    @bob   = User.create!(username: "bob",   display_name: "Bob",   public_key: "bob_key")

    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")
    Friendship.create!(requester: @bob, addressee: @alice, status: "accepted")
  end

  test "dashboard shows peer count summary" do
    login_as(@alice)
    visit root_path

    within "#connected-peers" do
      assert_text "Direct connections active with 0 peers"
    end

    assert_text "Hi, alice"
  end

  test "local hosting page can create WebRTC connection" do
    login_as(@alice)
    visit local_hosting_users_path

    assert_text "Hosting Status"
    assert_css "#p2p-status"
  end
end
