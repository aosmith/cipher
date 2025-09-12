class Peer < ApplicationRecord
  belongs_to :user

  validates :public_key, presence: true
  validates :address, presence: true
  validates :port, presence: true, numericality: { greater_than: 0, less_than: 65536 }

  scope :active, -> { where('last_seen > ?', 30.minutes.ago) }
  scope :recently_seen, -> { where('last_seen > ?', 24.hours.ago) }

  def update_last_seen!
    update!(last_seen: Time.current)
  end

  def online?
    last_seen && last_seen > 5.minutes.ago
  end

  def connection_info
    {
      peer_id: id,
      address: address,
      port: port,
      public_key: public_key,
      last_seen: last_seen
    }
  end

  def initiate_webrtc_connection
    # This will be called from the frontend to establish WebRTC connection
    {
      peer_id: id,
      stun_servers: WebRTCConfig.stun_servers,
      turn_servers: WebRTCConfig.turn_servers
    }
  end

  def verify_identity(challenge, signature)
    user.verify_signature(challenge, signature, public_key)
  end
end
