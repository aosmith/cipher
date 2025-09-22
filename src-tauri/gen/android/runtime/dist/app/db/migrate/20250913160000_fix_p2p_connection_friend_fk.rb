class FixP2pConnectionFriendFk < ActiveRecord::Migration[8.0]
  def up
    remove_foreign_key :p2p_connections, :friend_users if foreign_key_exists?(:p2p_connections, :friend_users)

    unless foreign_key_exists?(:p2p_connections, column: :friend_user_id)
      add_foreign_key :p2p_connections, :users, column: :friend_user_id
    end
  end

  def down
    remove_foreign_key :p2p_connections, column: :friend_user_id if foreign_key_exists?(:p2p_connections, column: :friend_user_id)
    add_foreign_key :p2p_connections, :friend_users if table_exists?(:friend_users)
  end
end
