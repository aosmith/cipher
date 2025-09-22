class RemovePrivateKeyStorage < ActiveRecord::Migration[8.0]
  def change
    remove_column :users, :private_key_encrypted, :text
  end
end
