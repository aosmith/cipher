class P2pConnection < ApplicationRecord
  belongs_to :user
  belongs_to :friend_user, class_name: 'User'

  validates :status, presence: true, inclusion: { in: %w[connecting connected disconnected failed] }
  validates :connection_type, inclusion: { in: %w[webrtc datachannel direct] }

  scope :active, -> { where(status: 'connected').where('last_seen > ?', 5.minutes.ago) }
  scope :recent, -> { where('last_seen > ?', 1.hour.ago) }

  def self.establish_connection(user, friend_user, connection_type = 'webrtc')
    connection = find_or_initialize_by(user: user, friend_user: friend_user)
    connection.status = 'connecting'
    connection.connection_type = connection_type
    connection.last_seen = Time.current
    connection.save!
    connection
  end

  def mark_connected!
    update!(status: 'connected', last_seen: Time.current)
  end

  def mark_disconnected!
    update!(status: 'disconnected', last_seen: Time.current)
  end

  def active?
    status == 'connected' && last_seen > 5.minutes.ago
  end
end
