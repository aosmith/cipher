#!/usr/bin/env bash
set -euo pipefail

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_APP_PATH="$SCRIPT_DIR/../target/release/bundle/macos/Cipher.app"
APP_PATH=${1:-"$DEFAULT_APP_PATH"}

if [[ ! -d "$APP_PATH" ]]; then
  echo "Cipher.app not found at $APP_PATH. Build it with 'cargo tauri build'." >&2
  exit 1
fi

FIXTURE_ROOT=$(mktemp -d /tmp/cipher_fixture_ui.XXXX)
CIPHER_FIXTURE_ROOT="$FIXTURE_ROOT" cargo run --bin seed_fixture >/tmp/cipher_seed.log

ALICE_HOME="$FIXTURE_ROOT/alice"
BOB_HOME="$FIXTURE_ROOT/bob"

pushd "$SCRIPT_DIR" >/dev/null
swift run CipherMacUITest "--app=$APP_PATH" "--home=$ALICE_HOME" --expect="username" --expect="sign in"
swift run CipherMacUITest "--app=$APP_PATH" "--home=$BOB_HOME" --expect="username" --expect="sign in"
popd >/dev/null

rm -rf "$FIXTURE_ROOT"
