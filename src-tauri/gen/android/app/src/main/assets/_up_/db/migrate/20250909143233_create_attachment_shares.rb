class CreateAttachmentShares < ActiveRecord::Migration[8.0]
  def change
    create_table :attachment_shares do |t|
      t.references :attachment, null: false, foreign_key: true
      t.references :user, null: false, foreign_key: true
      t.text :encrypted_key, null: false

      t.timestamps
    end
    
    add_index :attachment_shares, [:attachment_id, :user_id], unique: true
  end
end
