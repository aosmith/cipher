#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DEPS_DIR="$ROOT/src-tauri/gen/android/runtime/deps"
mkdir -p "$DEPS_DIR"

NDK_HOME=${NDK_HOME:-$HOME/Library/Android/sdk/ndk/26.3.11579264}
TOOLCHAIN=${TOOLCHAIN:-$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64}
TARGET_TRIPLE=${TARGET_TRIPLE:-aarch64-linux-android}
API=${ANDROID_API_LEVEL:-24}
SYSROOT="$TOOLCHAIN/sysroot"

CC="$TOOLCHAIN/bin/${TARGET_TRIPLE}${API}-clang"
CXX="$TOOLCHAIN/bin/${TARGET_TRIPLE}${API}-clang++"
AR="$TOOLCHAIN/bin/llvm-ar"
RANLIB="$TOOLCHAIN/bin/llvm-ranlib"
STRIP="$TOOLCHAIN/bin/llvm-strip"

export PATH="$TOOLCHAIN/bin:$PATH"
export CC CXX AR RANLIB STRIP
export CFLAGS="-fPIC -O2 --sysroot=$SYSROOT"
export LDFLAGS="-fPIC --sysroot=$SYSROOT"
export CPPFLAGS="--sysroot=$SYSROOT"
export PKG_CONFIG_LIBDIR="$DEPS_DIR/lib/pkgconfig:$DEPS_DIR/share/pkgconfig"
export PKG_CONFIG_SYSROOT_DIR="$SYSROOT"

build_libyaml() {
  local version=0.2.5
  local url="https://github.com/yaml/libyaml/archive/refs/tags/${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  (cd "$build_dir" && ./bootstrap)
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --prefix="$DEPS_DIR" \
    --enable-static \
    --disable-shared)
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu) && make install)
}

build_libffi() {
  local version=3.4.4
  local url="https://github.com/libffi/libffi/releases/download/v${version}/libffi-${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --prefix="$DEPS_DIR" \
    --enable-static \
    --disable-shared \
    ac_cv_func_open_temp_exec_file=no)
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu) && make install)
}

build_ncurses() {
  local version=6.5
  local url="https://ftp.gnu.org/pub/gnu/ncurses/ncurses-${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --prefix="$DEPS_DIR" \
    --with-build-cc=clang \
    --without-shared \
    --with-normal \
    --enable-widec \
    --without-cxx-binding \
    --without-cxx)
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu) libs && make install.libs)
}

build_libedit() {
  local version=20240804-3.1
  local url="https://thrysoee.dk/editline/libedit-${version}.tar.gz"
  local build_dir="$(mktemp -d)"
  trap 'rm -rf "$build_dir"' RETURN

  curl -sSL "$url" | tar -xz -C "$build_dir" --strip-components=1
  (cd "$build_dir" && ./configure \
    --host=$TARGET_TRIPLE \
    --prefix="$DEPS_DIR" \
    --enable-static \
    --disable-shared \
    --with-ncurses="$DEPS_DIR")
  (cd "$build_dir" && make -j$(sysctl -n hw.ncpu) && make install)
}

build_libyaml
build_libffi
build_ncurses
build_libedit

echo "Libraries installed under $DEPS_DIR"
