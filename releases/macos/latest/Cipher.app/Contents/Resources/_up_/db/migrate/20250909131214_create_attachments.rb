class CreateAttachments < ActiveRecord::Migration[8.0]
  def change
    create_table :attachments do |t|
      t.references :post, null: false, foreign_key: true
      t.string :filename
      t.string :content_type
      t.integer :file_size
      t.text :data_encrypted
      t.string :checksum

      t.timestamps
    end
  end
end
