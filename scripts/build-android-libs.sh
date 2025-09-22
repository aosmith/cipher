#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEPS_DIR="$ROOT/src-tauri/gen/android/runtime/deps"
mkdir -p "$DEPS_DIR"

log() {
  printf '[%s] %s\n' "$(date '+%H:%M:%S')" "$1"
}

NDK_HOME=${NDK_HOME:-$HOME/Library/Android/sdk/ndk}
if [[ -z "${TOOLCHAIN:-}" ]]; then
  TOOLCHAIN_ROOT="$NDK_HOME/toolchains/llvm/prebuilt"
  if [[ -d "$TOOLCHAIN_ROOT/darwin-arm64" ]]; then
    TOOLCHAIN="$TOOLCHAIN_ROOT/darwin-arm64"
  elif [[ -d "$TOOLCHAIN_ROOT/darwin-x86_64" ]]; then
    TOOLCHAIN="$TOOLCHAIN_ROOT/darwin-x86_64"
  else
    log "Unable to locate Apple NDK toolchain under $TOOLCHAIN_ROOT"
    exit 1
  fi
fi
TARGET_TRIPLE=${TARGET_TRIPLE:-aarch64-linux-android}
API=${ANDROID_API_LEVEL:-24}
SYSROOT="$TOOLCHAIN/sysroot"

HOST_CC=${HOST_CC:-$(xcrun -f clang 2>/dev/null || command -v clang || echo clang)}
HOST_SDK=${HOST_SDK:-$(xcrun --sdk macosx --show-sdk-path 2>/dev/null || echo '')}

CC="$TOOLCHAIN/bin/${TARGET_TRIPLE}${API}-clang"
CXX="$TOOLCHAIN/bin/${TARGET_TRIPLE}${API}-clang++"
AR="$TOOLCHAIN/bin/llvm-ar"
RANLIB="$TOOLCHAIN/bin/llvm-ranlib"
STRIP="$TOOLCHAIN/bin/llvm-strip"

export PATH="$TOOLCHAIN/bin:$PATH"
export CC CXX AR RANLIB STRIP
export CFLAGS="-fPIC -O2 --sysroot=$SYSROOT -DHAVE_OPEN_TEMP_EXEC_FILE=0 -I$DEPS_DIR/include -I$DEPS_DIR/include/ncursesw"
export LDFLAGS="-fPIC --sysroot=$SYSROOT -L$DEPS_DIR/lib"
export CPPFLAGS="--sysroot=$SYSROOT -I$DEPS_DIR/include -I$DEPS_DIR/include/ncursesw -D__STDC_ISO_10646__=201103L -DNBBY=8"
export BUILD_CC="$HOST_CC"
if [[ -n "$HOST_SDK" ]]; then
  export SDKROOT="$HOST_SDK"
  export BUILD_CFLAGS="-isysroot $HOST_SDK"
  export BUILD_CPPFLAGS="-isysroot $HOST_SDK"
  export BUILD_LDFLAGS="-isysroot $HOST_SDK"
else
  export BUILD_CFLAGS=""
  export BUILD_CPPFLAGS=""
  export BUILD_LDFLAGS=""
fi
export CC_FOR_BUILD="$BUILD_CC"
export PKG_CONFIG_LIBDIR="$DEPS_DIR/lib/pkgconfig:$DEPS_DIR/share/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"

build_libyaml() {
  log "Starting libyaml build"
  local version=0.2.5
  local url="https://github.com/yaml/libyaml/archive/refs/tags/${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  log "Downloading libyaml ${version}"
  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  (cd "$build_dir" && ./bootstrap)
  log "Configuring libyaml"
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --prefix="$DEPS_DIR" \
    --enable-static \
    --disable-shared)
  log "Compiling libyaml"
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu))
  log "Installing libyaml"
  (cd "$build_dir" && make install)
}

build_libffi() {
  log "Starting libffi build"
  local version=3.4.4
  local url="https://github.com/libffi/libffi/releases/download/v${version}/libffi-${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  log "Downloading libffi ${version}"
  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  log "Configuring libffi"
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --prefix="$DEPS_DIR" \
    --enable-static \
    --disable-shared \
    --disable-exec-static-tramp \
    ac_cv_func_open_temp_exec_file=no)
  log "Compiling libffi"
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu))
  log "Installing libffi"
  (cd "$build_dir" && make install)
}

build_ncurses() {
  log "Starting ncurses build"
  local version=6.5
  local url="https://ftp.gnu.org/pub/gnu/ncurses/ncurses-${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  log "Downloading ncurses ${version}"
  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  log "Configuring ncurses"
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --build=$(uname -m)-apple-darwin \
    --prefix="$DEPS_DIR" \
    --with-build-cc="$BUILD_CC" \
    --without-progs \
    --without-tests \
    --without-ada \
    --without-shared \
    --with-normal \
    --enable-widec \
    --without-cxx-binding \
    --without-cxx)
  log "Compiling ncurses"
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu) libs)
  log "Installing ncurses"
  (cd "$build_dir" && make install.libs)
  ln -sf libncursesw.a "$DEPS_DIR/lib/libncurses.a"
  ln -sf libncursesw.a "$DEPS_DIR/lib/libcurses.a"
  if [[ -f "$DEPS_DIR/lib/libncursesw_g.a" ]]; then
    ln -sf libncursesw_g.a "$DEPS_DIR/lib/libncurses_g.a"
    ln -sf libncursesw_g.a "$DEPS_DIR/lib/libcurses_g.a"
  fi
}

build_libedit() {
  log "Starting libedit build"
  local version=20230828-3.1
  local url="https://thrysoee.dk/editline/libedit-${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  log "Downloading libedit ${version}"
  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  log "Configuring libedit"
  local old_libs="${LIBS:-}"
  local old_ac_cv_func_setpwent="${ac_cv_func_setpwent+x}"
  local old_ac_cv_func_getpwent="${ac_cv_func_getpwent+x}"
  local old_ac_cv_func_endpwent="${ac_cv_func_endpwent+x}"
  export LIBS="-lncursesw ${LIBS:-}"
  export ac_cv_func_setpwent=no
  export ac_cv_func_getpwent=no
  export ac_cv_func_endpwent=no
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --prefix="$DEPS_DIR" \
    --enable-static \
    --disable-shared \
    --with-ncurses="$DEPS_DIR")
  export LIBS="$old_libs"
  if [[ -n "$old_ac_cv_func_setpwent" ]]; then
    export ac_cv_func_setpwent=${ac_cv_func_setpwent}
  else
    unset ac_cv_func_setpwent
  fi
  if [[ -n "$old_ac_cv_func_getpwent" ]]; then
    export ac_cv_func_getpwent=${ac_cv_func_getpwent}
  else
    unset ac_cv_func_getpwent
  fi
  if [[ -n "$old_ac_cv_func_endpwent" ]]; then
    export ac_cv_func_endpwent=${ac_cv_func_endpwent}
  else
    unset ac_cv_func_endpwent
  fi
  log "Compiling libedit"
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu))
  log "Installing libedit"
  (cd "$build_dir" && make install)
}

build_libyaml
build_libffi
build_ncurses
build_libedit

log "Libraries installed under $DEPS_DIR"
