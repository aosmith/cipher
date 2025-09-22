ENV["RAILS_ENV"] ||= "test"
require_relative "../config/environment"
require "rails/test_help"

module ActiveSupport
  class TestCase
    # Run tests sequentially when the run is large or known to be memory-heavy.
    # Developers can override with PARALLEL_WORKERS to opt back into concurrency.
    configured_workers = ENV["PARALLEL_WORKERS"].presence

    requested_paths = ARGV.take_while { |arg| arg != "--" }
                         .select { |arg| arg.start_with?("test/") || arg.end_with?("_test.rb") }

    running_full_suite = requested_paths.empty?
    memory_heavy_paths = requested_paths.any? do |path|
      path.start_with?("test/system", "test/integration", "test/services")
    end

    workers = if configured_workers
                configured_workers.to_i
    elsif running_full_suite || memory_heavy_paths
                1
    else
                2
    end

    parallelize(workers: [ workers, 1 ].max)

    Minitest.after_run do
      next if ENV["KEEP_TEST_LOGS"].present?

      log_path = Rails.root.join("log", "test.log")
      File.truncate(log_path, 0) if File.exist?(log_path)
    rescue Errno::EACCES => e
      warn "Could not truncate test.log: #{e.message}"
    end

    # Setup all fixtures in test/fixtures/*.yml for all tests in alphabetical order.
    fixtures :all

    # Add more helper methods to be used by all tests here...

    # Helper method for integration tests to simulate user login
    def login_as(user)
      # For ActionDispatch::IntegrationTest
      if respond_to?(:get) && respond_to?(:session)
        # Set session directly in integration test
        get "/users" # Make a request to establish session
        session[:user_id] = user.id
      elsif defined?(@request) && @request
        # Controller test - set session directly
        @request.session[:user_id] = user.id
      end

      # Make user available as instance variable
      @current_user = user
    end
  end
end

module TestHeartbeat
  class << self
    attr_accessor :last_progress_at, :tests_seen, :current_test
  end

  self.last_progress_at = Time.now
  self.tests_seen = 0

  module Instrumentation
    def before_setup
      TestHeartbeat.tests_seen += 1
      TestHeartbeat.last_progress_at = Time.now
      TestHeartbeat.current_test = "#{self.class}##{name}"
      super
    end
  end

  def self.start
    interval = ENV.fetch("TEST_HEARTBEAT_SEC", "30").to_i
    return if interval <= 0

    timeout = ENV.fetch("TEST_HEARTBEAT_TIMEOUT_SEC", (interval * 2).to_s).to_i
    timeout = interval if timeout <= 0

    running = true
    thread = Thread.new do
      Thread.current.name = "test-heartbeat"
      Thread.current.report_on_exception = false if Thread.current.respond_to?(:report_on_exception=)

      while running
        sleep interval

        now = Time.now
        since_last = (now - last_progress_at).round
        status = if tests_seen.positive?
                   "#{tests_seen} tests observed; last progress #{since_last}s ago"
        else
                   "initializing test suite"
        end

        if since_last >= timeout
          warn "[Heartbeat] No test progress for #{since_last}s (last: #{current_test || "unknown"})"
        else
          warn "[Heartbeat] #{status}" if ENV["TEST_HEARTBEAT_VERBOSE"].present?
        end

        $stdout.flush
        $stderr.flush
      end
    end

    Minitest.after_run do
      running = false
      thread.wakeup rescue ThreadError
      thread.join(1)
    end
  end
end

ActiveSupport::TestCase.prepend(TestHeartbeat::Instrumentation)
TestHeartbeat.start
