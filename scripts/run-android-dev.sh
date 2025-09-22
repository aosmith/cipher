#!/bin/bash
set -euo pipefail

info() {
  printf '[android-dev] %s\n' "$1"
}

ANDROID_HOME=${ANDROID_HOME:-"$HOME/Library/Android/sdk"}
NDK_HOME=${NDK_HOME:-"$ANDROID_HOME/ndk"}
export ANDROID_HOME NDK_HOME
export PATH="$ANDROID_HOME/platform-tools:$PATH"

DEFAULT_PORT=${TAURI_DEV_PORT:-3001}
DEV_HOST=${TAURI_DEV_HOST:-0.0.0.0}

declare -a devices_list=()
while read -r serial status _; do
  if [[ $serial != "List" && $status == "device" ]]; then
    devices_list+=("$serial")
  fi
done < <(adb devices)

if [[ ${#devices_list[@]} -eq 0 ]]; then
  info "No Android devices detected."
  exit 1
fi

serial=""
label=""
for d in "${devices_list[@]}"; do
  if [[ $d == emulator-* ]]; then
    serial="$d"
    label=$(adb -s "$d" emu avd name 2>/dev/null | tr -d '\r' | head -n 1)
    break
  fi
  if [[ -z "$serial" ]]; then
    serial="$d"
    label="$d"
  fi
done

if [[ -z "$serial" ]]; then
  serial="${devices_list[0]}"
  label="$serial"
fi

if [[ -z "$label" ]]; then
  label="$serial"
fi

if [[ $serial == emulator-* ]]; then
  target_host=${ANDROID_DEV_HOST:-10.0.2.2}
else
  if [[ -n "${ANDROID_DEV_HOST:-}" ]]; then
    target_host="$ANDROID_DEV_HOST"
  else
    default_iface=$(route get default 2>/dev/null | awk '/interface:/{print $2; exit}')
    target_host=$(ipconfig getifaddr "$default_iface" 2>/dev/null || true)
    if [[ -z "$target_host" ]]; then
      info "Physical device detected; set ANDROID_DEV_HOST to this Mac's LAN IP."
      exit 1
    fi
  fi
fi

while read -r pid; do
  kill "$pid" >/dev/null 2>&1 || true
done < <(lsof -ti tcp:"$DEFAULT_PORT" 2>/dev/null || true)

echo "[android-dev] Selected device: $serial"
if [[ $label != "$serial" ]]; then
  echo "[android-dev] Launch target: $label"
fi
echo "[android-dev] Dev host for client: $target_host"

echo "[android-dev] Starting Rails dev server"
TAURI_DEV_HOST="$DEV_HOST" TAURI_DEV_PORT="$DEFAULT_PORT" bin/android_dev_server &
rails_pid=$!

cleanup() {
  if [[ -n "$rails_pid" ]]; then
    echo "[android-dev] Stopping Rails dev server (pid $rails_pid)"
    kill "$rails_pid" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

TAURI_DEV_HOST="$DEV_HOST" TAURI_DEV_PORT="$DEFAULT_PORT" \
  cargo tauri android dev --config src-tauri/tauri.android.conf.json --host "$target_host" "$label"

