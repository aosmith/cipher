class WebRTCConfig
  # Free public STUN servers
  STUN_SERVERS = [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' },
    { urls: 'stun:stun2.l.google.com:19302' },
    { urls: 'stun:stun.cloudflare.com:3478' }
  ].freeze

  # You would need to configure your own TURN servers for production
  # These are placeholders - replace with real TURN servers
  TURN_SERVERS = [
    {
      urls: 'turn:your-turn-server.com:3478',
      username: 'your-username',
      credential: 'your-password'
    }
  ].freeze

  def self.stun_servers
    STUN_SERVERS
  end

  def self.turn_servers
    TURN_SERVERS
  end

  def self.ice_servers
    stun_servers + turn_servers
  end

  def self.peer_connection_config
    {
      iceServers: ice_servers,
      iceCandidatePoolSize: 10,
      bundlePolicy: 'max-bundle',
      rtcpMuxPolicy: 'require'
    }
  end
end