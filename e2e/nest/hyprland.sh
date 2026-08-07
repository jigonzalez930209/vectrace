#!/usr/bin/env bash
# Nested Hyprland session for e2e capture probe.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

skip_missing Hyprland

export NEST_SESSION_TYPE=wayland
unset DISPLAY

export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/vectrace-e2e-runtime-$$}"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR" 2>/dev/null || true

CONF="$E2E_ROOT/nest/hyprland.conf"
cat >"$CONF" <<'EOF'
monitor=,1280x720@60,0x0,1
misc {
  disable_hyprland_logo = true
  force_default_wallpaper = 0
}
decoration {
  rounding = 0
}
EOF

# Hyprland nested typically needs an existing Wayland or uses its own.
# Prefer running as nested client if host has WAYLAND_DISPLAY.
Hyprland -c "$CONF" >/dev/null 2>"$OUT_DIR/hyprland.log" &
HYPR_PID=$!
cleanup() {
  kill "$HYPR_PID" 2>/dev/null || true
  wait "$HYPR_PID" 2>/dev/null || true
}
trap cleanup EXIT

if ! wait_for 20 bash -c 'ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 || ls /tmp/hypr/*/  >/dev/null 2>&1'; then
  echo "Hyprland failed to start; see $OUT_DIR/hyprland.log" >&2
  exit 1
fi

SOCKET="$(ls -1t "$XDG_RUNTIME_DIR"/wayland-* 2>/dev/null | grep -v '\-lock$' | head -n1 || true)"
if [[ -n "$SOCKET" ]]; then
  export WAYLAND_DISPLAY="$(basename "$SOCKET")"
fi
echo "Nested Hyprland WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-unset}"
sleep 1
run_probe_from_args_file
