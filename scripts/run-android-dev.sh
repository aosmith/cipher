#!/bin/bash
set -euo pipefail

export ANDROID_HOME=${ANDROID_HOME:-"$HOME/Library/Android/sdk"}
export NDK_HOME=${NDK_HOME:-"$ANDROID_HOME/ndk"}
export PATH="$ANDROID_HOME/platform-tools:$PATH"
export TAURI_DEV_HOST=0.0.0.0

lsof -ti tcp:3001 | xargs -r kill

cargo tauri android dev --config src-tauri/tauri.android.conf.json
