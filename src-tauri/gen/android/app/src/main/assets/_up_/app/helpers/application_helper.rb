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

  def mobile_browser?
    return false unless request.user_agent
    request.user_agent.match?(/Mobile|Android|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini/i)
  end

  def android_device?
    return false unless request.user_agent
    request.user_agent.match?(/Android/i)
  end

  def ios_device?
    return false unless request.user_agent
    request.user_agent.match?(/iPhone|iPad|iPod/i)
  end

  def should_show_mobile_install_banner?
    return false if Rails.env.android? || Rails.env.ios? || Rails.env.desktop?
    return false if cookies[:hide_mobile_banner] == 'true'
    mobile_browser?
  end

  private

  def current_user_session
    return unless session[:user_id]
    @current_user_session ||= User.find_by(id: session[:user_id])
  end
end
