class RemoveThemeFromUsers < ActiveRecord::Migration[8.0]
  def change
    remove_column :users, :theme, :string
  end
end
