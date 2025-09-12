# Cipher

## ⚠️ BETA SOFTWARE WARNING

**This software is in BETA and should not be used for production or sensitive data.** While Cipher implements end-to-end encryption and zero-knowledge architecture, it has not undergone independent security audits. Use at your own risk and only with non-sensitive information until security has been independently verified.

Key security considerations:
- 🔒 End-to-end encryption is implemented but not audited
- 🌐 P2P networking features are experimental 
- 🔑 Key management system needs security review
- 📡 WebRTC connections may expose metadata
- 🛡️ Friend-based data syncing is new and untested at scale

**For security-critical applications, wait for independent security audit completion.**

---

A decentralized social network built with Rails 8.0.2, featuring end-to-end encryption, peer-to-peer data syncing, and zero-knowledge architecture. Users run local servers that connect via WebRTC for secure, private communication.

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
