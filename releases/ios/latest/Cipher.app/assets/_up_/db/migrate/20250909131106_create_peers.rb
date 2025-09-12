class CreatePeers < ActiveRecord::Migration[8.0]
  def change
    create_table :peers do |t|
      t.references :user, null: false, foreign_key: true
      t.string :address
      t.integer :port
      t.datetime :last_seen
      t.text :public_key

      t.timestamps
    end
  end
end
