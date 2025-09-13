class CreateP2pConnections < ActiveRecord::Migration[8.0]
  def change
    create_table :p2p_connections do |t|
      t.references :user, null: false, foreign_key: true
      t.references :friend_user, null: false, foreign_key: { to_table: :users }
      t.string :status
      t.datetime :last_seen
      t.string :connection_type

      t.timestamps
    end
  end
end
