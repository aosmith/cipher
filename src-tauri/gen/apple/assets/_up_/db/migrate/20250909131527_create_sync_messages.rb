class CreateSyncMessages < ActiveRecord::Migration[8.0]
  def change
    create_table :sync_messages do |t|
      t.references :user, null: false, foreign_key: true
      t.references :peer, null: false, foreign_key: true
      t.text :payload
      t.string :message_type
      t.string :status
      t.integer :processed_count
      t.integer :error_count

      t.timestamps
    end
  end
end
