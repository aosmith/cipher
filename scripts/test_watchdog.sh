#!/usr/bin/env bash
# shellcheck disable=SC2086
set -euo pipefail

cmd=("bundle" "exec" "bin/rails" "test")
if [[ $# -gt 0 ]]; then
  cmd=("$@")
fi

watch_interval=${TEST_WATCHDOG_INTERVAL:-30}
stall_threshold=${TEST_WATCHDOG_STALL:-120}
verbose=${TEST_WATCHDOG_VERBOSE:-}

if ! command -v "${cmd[0]}" >/dev/null 2>&1; then
  echo "[Watchdog] Command not found: ${cmd[0]}" >&2
  exit 127
fi

workdir=${TEST_WATCHDOG_WORKDIR:-"$(pwd)"}
mkdir -p "$workdir"

state_dir=$(mktemp -d "${TMPDIR:-/tmp}/test-watchdog.XXXXXX")
trap 'rm -rf "$state_dir"' EXIT

last_tick_file="$state_dir/last_tick"
printf '%s\n' "0" > "$last_tick_file"

fifo="$state_dir/output.fifo"
mkfifo "$fifo"

update_tick() {
  printf '%s\n' "$(date +%s)" > "$last_tick_file"
}

monitor() {
  local child_pid=$1
  local stall_count=0
  while kill -0 "$child_pid" >/dev/null 2>&1; do
    sleep "$watch_interval"
    local now last delta
    now=$(date +%s)
    if [[ ! -f "$last_tick_file" ]]; then
      break
    fi
    if ! last=$(<"$last_tick_file"); then
      last=0
    fi
    delta=$(( now - last ))

    if (( delta >= stall_threshold )); then
      ((stall_count++)) || true
      echo "[Watchdog] No test output for ${delta}s (last PID: $child_pid)" >&2
      if (( stall_count == 1 )); then
        kill -s USR1 "$child_pid" >/dev/null 2>&1 || true
      elif (( stall_count == 2 )); then
        kill -s QUIT "$child_pid" >/dev/null 2>&1 || true
      else
        # Escalate after repeated stalls
        kill -s KILL "$child_pid" >/dev/null 2>&1 || true
        break
      fi
    else
      (( stall_count )) && stall_count=0
      if [[ -n "$verbose" ]]; then
        echo "[Watchdog] ${delta}s since last output" >&2
      fi
    fi
  done
}

# Reader loop runs in background to update ticks and forward output
{
  while IFS= read -r line; do
    printf '%s\n' "$line"
    update_tick
  done < "$fifo"
} &
reader_pid=$!

# Reset tick before launching child so monitor has fresh reference
update_tick

# Prefer stdbuf/gstdbuf when available to keep output line buffered
runner=()
if command -v stdbuf >/dev/null 2>&1; then
  runner=(stdbuf -oL -eL)
elif command -v gstdbuf >/dev/null 2>&1; then
  runner=(gstdbuf -oL -eL)
fi

# Start command writing to FIFO
("${runner[@]}" "${cmd[@]}") > "$fifo" 2>&1 &
child_pid=$!
trap 'kill "$child_pid" 2>/dev/null || true' INT TERM

monitor "$child_pid" &
monitor_pid=$!

# Wait for child to finish
wait "$child_pid" 2>/dev/null
status=$?

# Cleanup
wait "$reader_pid" 2>/dev/null || true
kill "$monitor_pid" 2>/dev/null || true
wait "$monitor_pid" 2>/dev/null || true

if [[ -n "$verbose" ]]; then
  echo "[Watchdog] exiting with status $status" >&2
fi

exit "$status"
