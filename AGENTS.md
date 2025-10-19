# Repository Guidelines

> For architecture context, skim `CLAUDE.md`.

## Project Structure & Module Organization
- `src/` holds the Rust core; `src/app` handles database, crypto, and libp2p commands, while `src/js` ships helper scripts for the webview.
- Tauri metadata lives in `src-tauri/` and `tauri*.json`; automation scripts stay in `scripts/`, generated assets in `gen/`, and platform bundles in `releases/`.
- UI assets sit alongside the app: HTML/CSS at the root and branding artwork in `icons/`. Tests reside in `tests/` with mobile journeys under `tests/e2e/`.

## Build, Test, and Development Commands
- `cargo tauri dev` runs the desktop app with hot reload; `cargo run` exercises the backend only.
- `cargo build --release` produces optimized binaries; use `./scripts/build-all.sh` when you need every platform artifact.
- `cargo fmt` and `cargo clippy --all-targets --all-features` must pass before you open a PR.
- `cargo test` runs the Rust suite; filter with names like `cargo test functional_tests`. For Appium flows run `npm install && npm test --prefix tests/e2e` and set `TEST_PLATFORM` to pick a simulator.

## Coding Style & Naming Conventions
- Four-space indentation, `snake_case` modules/functions, `CamelCase` types, SCREAMING_SNAKE_CASE constants.
- Keep wrappers in `src/app/commands` thin and move domain logic to modules with explicit error propagation instead of `unwrap`.
- Add focused `///` docs to public APIs when behavior is not obvious.

## Testing Guidelines
- Cover crypto, database, and networking seams with integration tests in `tests/`; reuse builders to isolate fixtures.
- Name cases descriptively (`test_user_can_send_encrypted_message`) and add `-- --test-threads=1` when diagnosing shared-state flakiness.
- Capture UI or mobile regressions with the Appium suite and attach screenshots for tricky flows.

## Commit & Pull Request Guidelines
- Start commit subjects with an imperative verb (`Add`, `Fix`, `Update`); release bumps follow `Release vX.Y.Z: …`.
- Keep commits focused, reference issue IDs when relevant, and note any new binaries staged in `releases/`.
- PRs should summarize scope, affected platforms, verification (tests/screenshots), and highlight configuration changes.

## Security & Configuration Tips
- Persist user data via `app.path().app_data_dir()` and avoid logging secrets from the SQLite store.
- Store credentials in Tauri config or OS keychains; never commit secrets.
- After `cargo tauri android init`, run `./scripts/setup-android-icons.sh` to restore branded Android assets.
