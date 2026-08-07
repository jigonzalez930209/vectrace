#!/usr/bin/env bash
# Nested Weston session for e2e capture probe.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

skip_missing weston

export NEST_SESSION_TYPE=wayland
unset DISPLAY
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/vectrace-e2e-runtime-$$}"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR" 2>/dev/null || true

weston --backend=headless-backend.so --width=1280 --height=720 \
  >/dev/null 2>"$OUT_DIR/weston.log" &
WESTON_PID=$!
cleanup() {
  kill "$WESTON_PID" 2>/dev/null || true
  wait "$WESTON_PID" 2>/dev/null || true
}
trap cleanup EXIT

if ! wait_for 15 bash -c 'ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1'; then
  # Fallback: some weston builds use wayland-0 in default runtime.
  if ! wait_for 5 bash -c '[[ -n "${WAYLAND_DISPLAY:-}" ]]'; then
    echo "Weston failed to start; see $OUT_DIR/weston.log" >&2
    exit 1
  fi
fi

SOCKET="$(ls -1t "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null | grep -v '\-lock$' | head -n1 || true)"
if [[ -n "$SOCKET" ]]; then
  export WAYLAND_DISPLAY="$(basename "$SOCKET")"
fi
# Ensure probe does not treat this as XWayland.
unset DISPLAY
echo "Nested Weston WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unset}"
sleep 1
run_probe_from_args_file
