class ChangeDefaultThemeToDeepBlue < ActiveRecord::Migration[8.0]
  def change
    change_column_default :users, :theme, 'deep-blue'
  end
end
