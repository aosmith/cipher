require "application_system_test_case"

class P2PConnectivityTest < ApplicationSystemTestCase
  setup do
    ApplicationRecord.connection.disable_referential_integrity do
      [ Comment, Post, Friendship, User ].each(&:delete_all)
    end

    @alice = User.create!(username: "alice", display_name: "Alice", public_key: "alice_key")
    @bob   = User.create!(username: "bob",   display_name: "Bob",   public_key: "bob_key")

    Friendship.create!(requester: @alice, addressee: @bob, status: "accepted")
  end

  test "local hosting dashboard renders essential widgets" do
    login_as(@alice)
    visit local_hosting_users_path

    assert_text "Hosting Status"
    assert_selector "#hosting-toggle", visible: :all
    assert_selector "#hosting-status-text", text: /Hosting/i

    within ".status-grid" do
      assert_text "Storage Allocated"
      assert_text "Storage Used"
    end
  end

  test "hosting page reports network status without browser APIs" do
    login_as(@alice)
    visit local_hosting_users_path

    assert_text "Local Hosting"
    assert_selector "#p2p-status"
  end
end
