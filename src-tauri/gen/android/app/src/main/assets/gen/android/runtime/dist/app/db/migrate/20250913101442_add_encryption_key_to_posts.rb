class AddEncryptionKeyToPosts < ActiveRecord::Migration[8.0]
  def change
    add_column :posts, :encryption_key, :text
  end
end
