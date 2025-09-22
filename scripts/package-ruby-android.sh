#!/usr/bin/env bash
# Bundle MRI Ruby + gems for the Android target.
# This script expects the Android NDK toolchain and ruby-build to be available.
# It assembles a portable Ruby runtime under src-tauri/gen/android/runtime.

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
RUNTIME_DIR="$ROOT/src-tauri/gen/android/runtime"
BUILD_DIR="${RUNTIME_DIR}/build"
DIST_DIR="${RUNTIME_DIR}/dist"

ANDROID_API_LEVEL=${ANDROID_API_LEVEL:-24}
ANDROID_ARCH=${ANDROID_ARCH:-arm64-v8a}
RUBY_VERSION=${RUBY_VERSION:-3.2.1}
TARGET_TRIPLE=${TARGET_TRIPLE:-aarch64-linux-android}

# Toolchain defaults
NDK_HOME=${NDK_HOME:-$HOME/Library/Android/sdk/ndk}
if [[ -z "${TOOLCHAIN:-}" ]]; then
  TOOLCHAIN_ROOT="$NDK_HOME/toolchains/llvm/prebuilt"
  if [[ -d "$TOOLCHAIN_ROOT/darwin-arm64" ]]; then
    TOOLCHAIN="$TOOLCHAIN_ROOT/darwin-arm64"
  elif [[ -d "$TOOLCHAIN_ROOT/darwin-x86_64" ]]; then
    TOOLCHAIN="$TOOLCHAIN_ROOT/darwin-x86_64"
  else
    echo "[ruby-android] Unable to locate Apple toolchain under $TOOLCHAIN_ROOT" >&2
    exit 1
  fi
fi
RUBY_BUILD=${RUBY_BUILD:-$(command -v ruby-build || true)}

TARGET_CC="$TOOLCHAIN/bin/${TARGET_TRIPLE}${ANDROID_API_LEVEL}-clang"
TARGET_CXX="$TOOLCHAIN/bin/${TARGET_TRIPLE}${ANDROID_API_LEVEL}-clang++"

if [[ ! -x "$TARGET_CC" ]]; then
  echo "[ruby-android] Missing NDK toolchain at $TOOLCHAIN" >&2
  exit 1
fi

if [[ -z "$RUBY_BUILD" ]]; then
  echo "[ruby-android] ruby-build not found (brew install ruby-build)" >&2
  exit 1
fi

mkdir -p "$BUILD_DIR" "$DIST_DIR"

cat <<"CONFIG" > "$BUILD_DIR/config.site"
ac_cv_func_gethostbyname=yes
ac_cv_func_gethostent=yes
ac_cv_func_endpwent=no
ac_cv_header_pwd_h=no
aix_cv_endpwent_works=no
rb_cv_have_endpwent=no
aix_cv_c_stack=65536
rb_cv_type_deprecated=no
CONFIG

export CC="$TARGET_CC"
export CXX="$TARGET_CXX"
export AR="$TOOLCHAIN/bin/llvm-ar"
export LD="$TOOLCHAIN/bin/ld"
export RANLIB="$TOOLCHAIN/bin/llvm-ranlib"
export READELF="$TOOLCHAIN/bin/llvm-readelf"
export STRIP="$TOOLCHAIN/bin/llvm-strip"
export CFLAGS="-fPIC -O2"
export LDFLAGS="-fPIC"
export RUBY_CONFIGURE_OPTS="--with-static-linked-ext --disable-install-doc --without-gmp --host=${TARGET_TRIPLE} --target=${TARGET_TRIPLE} --with-out-ext=dbm,openssl,zlib"
export ac_cv_func_nl_langinfo=no
export ac_cv_header_langinfo_h=no
export ac_cv_func_endpwent=no
export CONFIG_SITE="$BUILD_DIR/config.site"
export ANDROID_NDK_ROOT="$NDK_HOME"
export PATH="$TOOLCHAIN/bin:$PATH"

PREFIX="$DIST_DIR/${TARGET_TRIPLE}"
mkdir -p "$PREFIX"

if [[ ! -d "$PREFIX/lib/ruby" ]]; then
  echo "[ruby-android] building Ruby $RUBY_VERSION for Android" >&2
  "$RUBY_BUILD" "$RUBY_VERSION" "$PREFIX" --verbose
else
  echo "[ruby-android] Ruby runtime already present at $PREFIX" >&2
fi

# Install bundle into vendor directory
GEM_HOME="$PREFIX/lib/ruby/gems/3.2.0"
export GEM_HOME
export GEM_PATH="$GEM_HOME"
export BUNDLE_PATH="$GEM_HOME"

BUNDLE_BIN="$PREFIX/bin/bundle"
if [[ ! -x "$BUNDLE_BIN" ]]; then
  echo "[ruby-android] Installing bundler" >&2
  "$PREFIX/bin/gem" install bundler --no-document
fi

pushd "$ROOT" >/dev/null
RAILS_ENV=android "$PREFIX/bin/bundle" install --deployment --path "$GEM_HOME" --without development test --binstubs "$PREFIX/bin"
popd >/dev/null

# Copy application files
APP_DIR="$DIST_DIR/app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR"
rsync -a --exclude 'tmp/' --exclude 'log/' --exclude 'node_modules/' \
  "$ROOT/app" "$ROOT/config" "$ROOT/db" "$ROOT/lib" "$ROOT/Gemfile" "$ROOT/Gemfile.lock" "$ROOT/bin" "$ROOT/config.ru" "$APP_DIR"

# Prepare bootstrap script
cat <<'BOOT' > "$DIST_DIR/run_rails.sh"
#!/usr/bin/env sh
set -eu
BASE_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
export GEM_HOME="$BASE_DIR/aarch64-linux-android/lib/ruby/gems/3.2.0"
export GEM_PATH="$GEM_HOME"
export BUNDLE_GEMFILE="$BASE_DIR/app/Gemfile"
export HOME="${HOME:-$BASE_DIR/tmp}"
export TMPDIR="$BASE_DIR/tmp"
mkdir -p "$TMPDIR"
cd "$BASE_DIR/app"
exec "$BASE_DIR/aarch64-linux-android/bin/bundle" exec rails server -p 3001 -b 127.0.0.1 -e android
BOOT
chmod +x "$DIST_DIR/run_rails.sh"

echo "[ruby-android] Bundled runtime at $DIST_DIR"
