class AddEmailVerificationToUsers < ActiveRecord::Migration[8.0]
  def change
    add_column :users, :email, :string
    add_column :users, :email_verified_at, :datetime
    add_column :users, :verification_code, :string
    add_column :users, :verification_code_expires_at, :datetime

    add_index :users, :email, unique: true
    add_index :users, :verification_code
  end
end
