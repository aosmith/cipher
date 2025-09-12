class AddSyncFieldsToPosts < ActiveRecord::Migration[8.0]
  def change
    add_reference :posts, :original_user, null: true, foreign_key: { to_table: :users }
    add_reference :posts, :synced_from_user, null: true, foreign_key: { to_table: :users }
    add_column :posts, :is_synced, :boolean, default: false
    add_column :posts, :synced_at, :datetime
    add_column :posts, :content_hash, :string
    
    add_index :posts, :is_synced
    add_index :posts, :synced_at
    add_index :posts, :content_hash
  end
end
