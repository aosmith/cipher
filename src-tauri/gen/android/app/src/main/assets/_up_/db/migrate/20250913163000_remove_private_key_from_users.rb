class RemovePrivateKeyFromUsers < ActiveRecord::Migration[8.0]
  def up
    if column_exists?(:users, :private_key)
      remove_column :users, :private_key, :text
    end
  end

  def down
    unless column_exists?(:users, :private_key)
      add_column :users, :private_key, :text
    end
  end
end
