#!/usr/bin/env sh
set -eu
BASE_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
export GEM_HOME="$BASE_DIR/aarch64-linux-android/lib/ruby/gems/3.2.0"
export GEM_PATH="$GEM_HOME"
export BUNDLE_GEMFILE="$BASE_DIR/app/Gemfile"
export HOME="${HOME:-$BASE_DIR/tmp}"
export TMPDIR="$BASE_DIR/tmp"
mkdir -p "$TMPDIR"
export RAILS_ENV="${RAILS_ENV:-android}"
cd "$BASE_DIR/app"

BUNDLE_BIN="$BASE_DIR/aarch64-linux-android/bin/bundle"

echo "[ruby-android] Preparing database for $RAILS_ENV"
"$BUNDLE_BIN" exec rails db:prepare

exec "$BUNDLE_BIN" exec rails server -p 3001 -b 127.0.0.1 -e "$RAILS_ENV"
