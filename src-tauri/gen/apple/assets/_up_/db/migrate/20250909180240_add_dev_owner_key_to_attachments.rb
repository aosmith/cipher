class AddDevOwnerKeyToAttachments < ActiveRecord::Migration[8.0]
  def change
    add_column :attachments, :dev_owner_key, :text
  end
end
