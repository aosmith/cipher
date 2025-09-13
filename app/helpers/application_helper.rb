module ApplicationHelper
  def p2p_connection_status
    return "Offline" unless current_user_session

    # Use new P2P connection tracking system
    active_connections = current_user_session.p2p_connection_count

    if active_connections > 0
      "🟢 Connected (#{active_connections} peers)"
    elsif current_user_session.friends.any?
      "🟡 Online (No active connections)"
    else
      "🔴 No friends to connect to"
    end
  end

  def p2p_status_css_class
    return "status-disconnected" unless current_user_session

    active_connections = current_user_session.p2p_connection_count

    if active_connections > 0
      "status-connected"
    elsif current_user_session.friends.any?
      "status-connecting"
    else
      "status-disconnected"
    end
  end

  private

  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end

  def stun_server_status
    # Simple server connectivity check using Ruby's built-in networking
    require 'socket'
    require 'timeout'

    begin
      Timeout::timeout(3) do
        socket = UDPSocket.new
        socket.connect('stun.l.google.com', 19302)
        socket.close
        return { indicator: '●', status: 'Google STUN servers reachable', color: '#27ae60' }
      end
    rescue => e
      Rails.logger.info "STUN server check failed: #{e.message}"
      return { indicator: '●', status: 'STUN server check failed', color: '#e74c3c' }
    end
  end
end
