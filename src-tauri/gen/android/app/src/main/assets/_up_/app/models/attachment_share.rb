class AttachmentShare < ApplicationRecord
  belongs_to :attachment
  belongs_to :user

  validates :encrypted_key, presence: true
  validates :user_id, uniqueness: { scope: :attachment_id }

  # Check if a user has access to decrypt this attachment
  def self.user_has_access?(user, attachment)
    exists?(user: user, attachment: attachment)
  end

  # Get the encrypted key for a specific user
  def self.encrypted_key_for(user, attachment)
    find_by(user: user, attachment: attachment)&.encrypted_key
  end
end
