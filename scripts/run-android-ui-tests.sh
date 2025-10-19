#!/usr/bin/env bash
set -euo pipefail

ANDROID_HOME_DEFAULT="${HOME}/Library/Android/sdk"
if [[ -z "${ANDROID_HOME:-}" ]]; then
  export ANDROID_HOME="${ANDROID_HOME_DEFAULT}"
fi

if [[ -z "${NDK_HOME:-}" ]]; then
  if [[ -d "${ANDROID_HOME}/ndk/28.0.12433566" ]]; then
    export NDK_HOME="${ANDROID_HOME}/ndk/28.0.12433566"
  else
    echo "NDK_HOME is not set and default path not found. Please export NDK_HOME." >&2
    exit 1
  fi
fi

if ! command -v adb >/dev/null 2>&1; then
  echo "adb command not found. Ensure Android platform tools are installed and on PATH." >&2
  exit 1
fi

device_count=$(adb devices | grep -w "device" | wc -l | tr -d ' ')
if [[ "${device_count}" -lt 1 ]]; then
  echo "No connected Android emulator/device detected. Start one and retry." >&2
  adb devices
  exit 1
fi

pushd gen/android >/dev/null
./gradlew :app:connectedUniversalDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=com.cipher.social.LoginScreenInstrumentedTest
popd >/dev/null
