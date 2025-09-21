# Repository Guidelines

## Project Structure & Module Organization
Cipher pairs Rails 8 with a Rust/Tauri desktop shell. Rails code sits in `app/` and `lib/`; desktop config lives in `src-tauri/`. Use `bin/` for Rails wrappers, `scripts/` for builds, `test/` for suites, and `docs/` for references. Each installation runs as its own server, so default to server-rendered flows and Ruby services before reaching for browser logic. Browser-side JavaScript is disabled; build interactive behavior into controllers and background jobs instead.

## Build, Test, and Development Commands
Run `bin/setup` once for Ruby, Node, and database prep. Daily work runs through `bin/dev`; use `bin/reload` after config tweaks. Call `bin/rails db:prepare`, `db:migrate`, or `db:reset` for database work, `bin/rubocop` + `bin/brakeman` for checks, and `bin/rails test` for suites.

## Desktop Build Playbook
Prerequisites: Node 20+, Rust stable, Ruby 3.3+, Bundler, Tauri CLI. Install Rust targets up front: `rustup target add x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-msvc x86_64-unknown-linux-gnu`. Build matrix:

| Platform | Command | Notes |
| --- | --- | --- |
| macOS (Intel/ARM) | `scripts/build-desktop.sh` | Needs Xcode CLT; outputs `.app`/`.dmg` to `src-tauri/target/release/bundle/macos/`. |
| Windows | `scripts\\build-desktop-windows.bat` | Run in Developer Command Prompt; emits `.msi`. |
| Linux | `npm run tauri:build:linux` | Needs GTK/WebKit packages; outputs `.AppImage` + `.deb`. |

Workflow: (1) `npm install && bundle install`, (2) `RAILS_ENV=desktop bundle exec rails db:prepare && assets:precompile`, (3) run the platform script. For debugging use `npm run tauri:dev`. If builds fail, confirm icons in `src-tauri/icons/`, targets installed, and identifiers in `tauri.conf.json` unchanged.

## Coding Style & Naming Conventions
Ruby follows Rails Omakase—two-space indent, snake_case methods, CamelCase classes, `_path`/`_url` helpers. Minitest files live under `test/` as `SomethingTest`. Favor HTML/CSS/Ruby solutions; do not ship new browser JavaScript. Run `cargo fmt && cargo clippy` before committing Rust work.

## Testing Guidelines
Target `bin/rails test` before pushing; narrow runs with file paths while iterating. Use system tests for user flows, model tests for crypto logic, and add smoke tests around WebRTC or the Tauri bridge. Keep fixtures in `test/fixtures` lean and anonymized.

## Commit & Pull Request Guidelines
Use imperative commit subjects (`Add P2P reconnect guard`), keep related changes together, and amend instead of piling fixups. Verify `bin/rails test`, `bin/rubocop`, and relevant `npm run tauri:*` builds, then note results in the PR body. Link issues with `Closes #123`, flag risk, and attach screenshots or screencasts for UI tweaks.

## Security & Configuration Notes
Store secrets with Rails credentials; never commit `.env`. Document new ports or capabilities in `docs/` when altering P2P behavior. Desktop releases must retain bundle identifiers in `tauri.conf.json` to preserve auto-update trust.
