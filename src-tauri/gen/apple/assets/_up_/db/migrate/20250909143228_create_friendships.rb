class CreateFriendships < ActiveRecord::Migration[8.0]
  def change
    create_table :friendships do |t|
      t.references :requester, null: false, foreign_key: { to_table: :users }
      t.references :addressee, null: false, foreign_key: { to_table: :users }
      t.string :status, default: 'pending'

      t.timestamps
    end
    
    add_index :friendships, [:requester_id, :addressee_id], unique: true
    add_index :friendships, [:addressee_id, :requester_id]
    add_index :friendships, :status
  end
end
