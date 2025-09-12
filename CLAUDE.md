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

### Desktop Application
- `bin/desktop` - Start desktop app in development mode (macOS/Linux)
- `bin/desktop.bat` - Start desktop app in development mode (Windows)
- `./scripts/build-desktop.sh` - Build macOS desktop app
- `./scripts/build-desktop-windows.ps1` - Build Windows desktop app
- `npm run tauri:build:linux` - Build Linux desktop app

### Release Management
- `./scripts/prepare-release.sh [version]` - Build all platforms and prepare release files
- `./scripts/update-release-links.sh [version]` - Update all version numbers and download links
- **Releases Directory**: `releases/` contains pre-compiled desktop apps for Windows, macOS, and Linux
- **Current Version**: v0.6.1
- See `releases/README.md` for installation instructions and download links

#### Version Management
- **Version numbers must be sequential** - Always increment from the last tagged version
- **Keep Rails and Tauri versions in sync** - Both should use the same version number
- **Update all version references**: CLAUDE.md, src-tauri/Cargo.toml, and git tags
- Check existing tags with: `git tag | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | sort -V | tail -5`

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
- **Desktop App**: Tauri (Rust + WebView) for cross-platform native applications

## Security & Cryptography

### Local-First Architecture

**Local Deployment Model**
- Application runs entirely on user's local machine (Tauri desktop app or `rails server`)
- Database is local SQLite file owned by the user
- No remote server concerns - user controls their own data
- Private keys can be stored locally since it's the user's own device
- Encryption protects data at rest and in peer-to-peer communication

**Encryption for Privacy & P2P Communication**
- Messages: Encrypted with recipient's public key, signed with sender's private key
- Posts: Can be encrypted for privacy or stored plaintext (user's choice)
- Attachments: Encrypted with symmetric keys for sharing
- Private keys derived from username/password for deterministic key generation
- Use NaCl (libsodium) for all cryptographic operations

**Key Management**
- Username + Password → Private Key (via PBKDF2/Argon2 key derivation)
- Private Key → Public Key (via NaCl crypto_sign_keypair_seed)
- Private keys stored encrypted in local database (AES with password-derived key)
- Public keys shared for secure peer-to-peer communication
- Session unlocks private key for encryption/decryption operations
- **Public keys are the primary identifier** for P2P interactions (prevent username collisions)
- Usernames are local display names, public keys ensure global uniqueness

**Peer-to-Peer Security**
- Messages encrypted end-to-end between users
- Public key cryptography enables secure communication without shared secrets
- Digital signatures verify message authenticity
- Local data encrypted to prevent unauthorized access to user's device

**CRITICAL: Private Key Protection**
- ❌ **NEVER transmit private keys over P2P network channels**
- ❌ **NEVER expose other users' private keys through any API**  
- ❌ **NEVER log private keys in application logs**
- ✅ **Users can access their OWN private key via localhost API**
- ✅ **Private keys stored on local server (user's machine)**
- ✅ **Browser ↔ Server communication is secure (localhost)**
- ✅ **Only public keys transmitted over P2P channels**
- ✅ **Context-aware serialization prevents cross-user private key access**

**Security Model:**
- **Localhost Communication**: Browser ↔ Server communication is secure (localhost)
- **Own Private Key**: User can access their own private key via localhost API
- **Others' Private Keys**: Never accessible through any API or serialization
- **P2P Network**: Only public keys are transmitted between users over P2P channels
- **Local Storage**: Private keys stored on local server, never transmitted over P2P
- **Logging**: All private key parameters filtered from application logs
- **Trust Boundary**: Server is trusted (user's machine), P2P network is not trusted

**Security Safeguards Implemented:**
- Parameter filtering in `config/application.rb` and `config/initializers/security.rb`
- Context-aware User model serialization (own vs others' private keys)
- `serialize_user_safely` helper method for controllers
- API endpoints use explicit field selection for other users
- Thread-local context tracking for safe serialization

### File Structure
- Standard Rails 8 application structure
- Currently contains only base application classes (no custom models/controllers yet)
- Uses standard test framework (not RSpec)
- Kamal deployment configuration in `config/deploy.yml`
- **Desktop App Files**:
  - `src-tauri/` - Tauri application source code
  - `scripts/` - Build and release automation scripts
  - `releases/` - Pre-compiled desktop applications
  - `bin/desktop*` - Development launchers
  - `config/environments/desktop.rb` - Desktop-specific Rails configuration

### Configuration Notes
- Health check endpoint available at `/up`
- PWA manifest/service worker routes are commented out but available
- RuboCop uses rails-omakase configuration
- Tests run in parallel by default
- Production uses Kamal for deployment with SSL termination

## Release Workflow for Desktop Apps

When preparing a new release:

1. **Update Version Numbers**:
   ```bash
   ./scripts/update-release-links.sh [version]
   ```
   This updates all version references across the project.

2. **Build All Platforms**:
   ```bash
   ./scripts/prepare-release.sh [version]
   ```
   This builds desktop apps for all platforms and copies them to `releases/`.

3. **Test Builds**:
   - Test each platform's installer/package
   - Verify functionality on target operating systems
   - Check that all features work in desktop mode

4. **Update Release Files**:
   - The `releases/` directory contains pre-compiled versions
   - Update SHA256 checksums in `releases/*/latest/checksums.txt`
   - Update `releases/*/latest/CHANGELOG.md` with release notes

5. **Create GitHub Release**:
   - Tag the version: `git tag v[version]`
   - Push tags: `git push origin --tags`
   - Create GitHub release with files from `releases/*/latest/`

6. **Update Documentation**:
   - Verify download links work
   - Update any version-specific documentation
   - Test installation instructions

**Note**: The `releases/` directory structure allows users to download pre-compiled desktop applications without building from source. The `.gitignore` is configured to ignore the actual binary files while preserving the directory structure and documentation.

# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
- memorize fill in the sha 256 on the realeases readme after you compile them.
- memorize if you start a server also stop it
- use `bin/reload` to restart the server after CSS/layout changes
- memorize clear the cache after changes.
- memorize use stylesheets instead of inline styles.
- memorize compile any binaries we can and push tags to git for any new version.
- memorize when we version bump it should only be our apps and not dependancies unless they have been updated as well.  Ask before updating deps like Tauri
- memorize ALWAYS recompile binaries after version bumps using ./scripts/prepare-release.sh [version] - Don't just bump version numbers without rebuilding!
- memorize the update-release-links.sh script now only updates package versions, not Tauri dependency versions - this prevents build breaks
- memorize use as little javascript as possible, the client and the server are the same machine so latency is not an issue
- memorize We are user a local server to power a p2p encrypted social network, private keys can be on the server but they should never leave the users machine over p2p channels.  We can assume the browser to server connection is secure because it is localhost.