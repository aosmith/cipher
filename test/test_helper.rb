ENV["RAILS_ENV"] ||= "test"
require_relative "../config/environment"
require "rails/test_help"

module ActiveSupport
  class TestCase
    # Run tests sequentially by default to avoid reliance on DRb sockets, which
    # are blocked in our sandbox. Opt back in to parallel runs via PARALLEL_WORKERS.
    workers = ENV.fetch("PARALLEL_WORKERS", "1").to_i
    parallelize(workers:) if workers > 1

    # Setup all fixtures in test/fixtures/*.yml for all tests in alphabetical order.
    fixtures :all

    # Add more helper methods to be used by all tests here...
    
    # Helper method for integration tests to simulate user login
    def login_as(user)
      # For ActionDispatch::IntegrationTest
      if respond_to?(:get) && respond_to?(:session)
        # Set session directly in integration test
        get '/users' # Make a request to establish session
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
