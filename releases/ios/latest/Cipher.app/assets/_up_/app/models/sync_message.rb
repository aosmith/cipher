class SyncMessage < ApplicationRecord
  belongs_to :user
  belongs_to :peer
end
