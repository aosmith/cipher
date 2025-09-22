class CreateUsers < ActiveRecord::Migration[8.0]
  def change
    create_table :users do |t|
      t.text :public_key
      t.text :private_key_encrypted
      t.string :username
      t.string :display_name

      t.timestamps
    end
  end
end
