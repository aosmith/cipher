# Cipher

A Rails 8.0.2 application built with modern Rails conventions, featuring Hotwire for interactive frontend experiences and configured for seamless deployment via Kamal.

## Quick Start

Get up and running in one command:

```bash
bin/setup
```

This will install dependencies, prepare the database, and start the development server.

## Development

### Starting the Server

```bash
bin/dev        # Recommended - starts development server
bin/rails server  # Alternative direct approach
```

### Reloading Changes

Rails automatically reloads most code changes. For configuration, routes, or initializer changes:

```bash
bin/reload     # Graceful restart without stopping the server
```

## Architecture

- **Rails**: 8.0.2 with modern defaults
- **Database**: SQLite with Solid Cache, Solid Queue, and Solid Cable
- **Frontend**: Hotwire (Turbo Rails + Stimulus)
- **Assets**: Propshaft with Importmap for JavaScript
- **Background Jobs**: Solid Queue (integrated with Puma)
- **Deployment**: Kamal with Docker

## Development Commands

### Database

```bash
bin/rails db:prepare    # Setup and seed database
bin/rails db:migrate    # Run migrations
bin/rails db:seed       # Seed database
bin/rails db:reset      # Full database reset
bin/rails dbconsole     # Database console
```

### Testing

```bash
bin/rails test                              # Run all tests
bin/rails test test/path/to/specific_test.rb # Run specific test
```

### Code Quality

```bash
bin/rubocop     # Lint with RuboCop (rails-omakase config)
bin/brakeman    # Security analysis
```

### Assets

```bash
bin/rails assets:precompile    # Compile for production
```

## Deployment

Deploy to production using Kamal:

```bash
bin/kamal deploy    # Deploy application
bin/kamal console   # Access production console
bin/kamal shell     # Access production shell
bin/kamal logs      # View production logs
```

## Health Check

The application includes a health check endpoint at `/up` for monitoring and deployment verification.

## Configuration

- **Web Server**: Puma with Thruster for production
- **SSL**: Handled by Kamal deployment
- **Caching**: Solid Cache for high-performance caching
- **WebSockets**: Solid Cable for real-time features
