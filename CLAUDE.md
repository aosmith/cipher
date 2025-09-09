# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rails 8.0.2 application called "Cipher" with minimal features currently implemented. The application uses modern Rails conventions with Hotwire (Turbo + Stimulus), SQLite for the database, and is configured for deployment via Kamal.

## Development Commands

### Setup
- `bin/setup` - Initial setup (installs dependencies, prepares database, starts server)
- `bundle install` - Install Ruby dependencies
- `bin/rails db:prepare` - Setup and seed database

### Running the Application
- `bin/dev` - Start development server (equivalent to `bin/rails server`)
- `bin/rails server` - Start Rails server directly

**Server Reloading**: 
- Rails automatically reloads most code changes in development mode
- For configuration, routes, or initializer changes that require restart: `bin/reload`
- The reload command triggers a graceful restart without stopping the server process

### Testing
- `bin/rails test` - Run all tests
- `bin/rails test test/path/to/specific_test.rb` - Run specific test file

### Code Quality
- `bin/rubocop` - Run RuboCop linter (uses rubocop-rails-omakase configuration)
- `bin/brakeman` - Run security analysis

### Database Operations
- `bin/rails db:create` - Create database
- `bin/rails db:migrate` - Run migrations
- `bin/rails db:seed` - Seed database
- `bin/rails db:reset` - Drop, create, migrate, and seed database
- `bin/rails dbconsole` - Open database console

### Asset Management
- `bin/rails assets:precompile` - Compile assets for production

### Deployment
- `bin/kamal deploy` - Deploy via Kamal
- `bin/kamal console` - Access production console
- `bin/kamal shell` - Access production shell
- `bin/kamal logs` - View production logs

## Architecture

### Core Components
- **Application Name**: Cipher (defined in `config/application.rb`)
- **Rails Version**: 8.0.2 with modern defaults
- **Database**: SQLite with Solid Cache, Solid Queue, and Solid Cable
- **Asset Pipeline**: Propshaft with Importmap for JavaScript
- **Frontend**: Hotwire (Turbo Rails + Stimulus)

### Key Technologies
- **Background Jobs**: Solid Queue (runs in Puma process via `SOLID_QUEUE_IN_PUMA`)
- **Caching**: Solid Cache
- **WebSockets**: Solid Cable
- **Deployment**: Kamal with Docker
- **Web Server**: Puma with Thruster for production

### File Structure
- Standard Rails 8 application structure
- Currently contains only base application classes (no custom models/controllers yet)
- Uses standard test framework (not RSpec)
- Kamal deployment configuration in `config/deploy.yml`

### Configuration Notes
- Health check endpoint available at `/up`
- PWA manifest/service worker routes are commented out but available
- RuboCop uses rails-omakase configuration
- Tests run in parallel by default
- Production uses Kamal for deployment with SSL termination