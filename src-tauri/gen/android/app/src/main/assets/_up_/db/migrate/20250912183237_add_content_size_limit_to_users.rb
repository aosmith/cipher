class AddContentSizeLimitToUsers < ActiveRecord::Migration[8.0]
  def change
    add_column :users, :content_size_limit, :integer, default: 10485760 # 10MB in bytes
  end
end
