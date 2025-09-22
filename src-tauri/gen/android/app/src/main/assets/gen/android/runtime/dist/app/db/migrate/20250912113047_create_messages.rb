class CreateMessages < ActiveRecord::Migration[8.0]
  def change
    create_table :messages do |t|
      t.references :sender, null: false, foreign_key: { to_table: :users }
      t.references :recipient, null: false, foreign_key: { to_table: :users }
      t.text :content
      t.text :encrypted_content
      t.datetime :read_at

      t.timestamps
    end

    # Add indices for common queries
    add_index :messages, [ :sender_id, :recipient_id, :created_at ]
    add_index :messages, [ :recipient_id, :read_at ]
    add_index :messages, :created_at
  end
end
