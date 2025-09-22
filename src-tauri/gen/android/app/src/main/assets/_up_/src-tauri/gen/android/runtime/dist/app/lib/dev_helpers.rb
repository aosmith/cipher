# Development helper utilities for Claude Code
class DevHelpers
  def self.restart_server
    puts "🔄 Restarting Rails server..."

    # Touch the tmp/restart.txt file to trigger server restart
    FileUtils.touch(Rails.root.join("tmp", "restart.txt"))
    puts "✅ Server restart triggered"
  end

  def self.clear_cache
    puts "🧹 Clearing Rails cache..."
    Rails.cache.clear
    puts "✅ Cache cleared"
  end

  def self.reload_routes
    puts "🛤️  Reloading routes..."
    Rails.application.reload_routes!
    puts "✅ Routes reloaded"
  end
end
