#!/usr/bin/env bash
# Shared helpers for nested compositor scripts.
set -euo pipefail

: "${OUT_DIR:?OUT_DIR required}"
: "${PROBE_BIN:?PROBE_BIN required}"
: "${E2E_ROOT:?E2E_ROOT required}"

run_probe_from_args_file() {
  local args_file="$OUT_DIR/probe.args"
  [[ -f "$args_file" ]] || { echo "missing $args_file" >&2; return 1; }
  local -a args=()
  while IFS= read -r -d '' a; do
    args+=("$a")
  done <"$args_file"
  if [[ -n "${NEST_SESSION_TYPE:-}" ]]; then
    export XDG_SESSION_TYPE="$NEST_SESSION_TYPE"
  fi
  "$PROBE_BIN" "${args[@]}"
}

skip_missing() {
  local bin="$1"
  if ! command -v "$bin" >/dev/null 2>&1; then
    echo "SKIP: '$bin' not installed"
    exit 2
  fi
}

wait_for() {
  local seconds="$1"
  shift
  local deadline=$((SECONDS + seconds))
  while (( SECONDS < deadline )); do
    if "$@"; then
      return 0
    fi
    sleep 0.2
  done
  return 1
}

# List Wayland server sockets in $XDG_RUNTIME_DIR (exclude *.lock).
list_wayland_sockets() {
  local f
  for f in "$XDG_RUNTIME_DIR"/wayland-*; do
    [[ -e "$f" ]] || continue
    [[ "$f" == *.lock ]] && continue
    [[ -S "$f" ]] || continue
    printf '%s\n' "$f"
  done
}

# Start PipeWire + xdg-desktop-portal + portal-wlr for ScreenCast on the
# current WAYLAND_DISPLAY. Best-effort; logs go under $OUT_DIR.
start_wayland_portals() {
  : "${WAYLAND_DISPLAY:?WAYLAND_DISPLAY required for portals}"
  export XDG_CURRENT_DESKTOP="${XDG_CURRENT_DESKTOP:-sway}"
  export XDG_SESSION_TYPE="${XDG_SESSION_TYPE:-wayland}"

  # Prefer portal-wlr for ScreenCast in nested/docker sessions.
  mkdir -p "$HOME/.config/xdg-desktop-portal"
  if [[ -f "$E2E_ROOT/nest/xdg-desktop-portal/portals.conf" ]]; then
    cp -f "$E2E_ROOT/nest/xdg-desktop-portal/portals.conf" \
      "$HOME/.config/xdg-desktop-portal/portals.conf"
  else
    printf '[preferred]\ndefault=wlr;\n' >"$HOME/.config/xdg-desktop-portal/portals.conf"
  fi

  # Unattended screencast: without this, portal-wlr tries slurp/wofi and denies.
  mkdir -p "$HOME/.config/xdg-desktop-portal-wlr"
  if [[ -f "$E2E_ROOT/nest/xdg-desktop-portal-wlr/config" ]]; then
    cp -f "$E2E_ROOT/nest/xdg-desktop-portal-wlr/config" \
      "$HOME/.config/xdg-desktop-portal-wlr/config"
  else
    cat >"$HOME/.config/xdg-desktop-portal-wlr/config" <<'EOF'
[screencast]
chooser_type=none
output_name=WL-1
EOF
  fi

  if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]] && command -v dbus-daemon >/dev/null 2>&1; then
    dbus-daemon --session --address="unix:path=$XDG_RUNTIME_DIR/bus" --fork
    export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
  fi

  if command -v pipewire >/dev/null 2>&1; then
    if ! pgrep -u "$(id -u)" -x pipewire >/dev/null 2>&1; then
      pipewire >"$OUT_DIR/pipewire.log" 2>&1 &
    fi
    if command -v wireplumber >/dev/null 2>&1; then
      if ! pgrep -u "$(id -u)" -x wireplumber >/dev/null 2>&1; then
        wireplumber >"$OUT_DIR/wireplumber.log" 2>&1 &
      fi
    fi
  fi

  local xdp xdp_wlr
  xdp="$(command -v xdg-desktop-portal || true)"
  [[ -z "$xdp" && -x /usr/libexec/xdg-desktop-portal ]] && xdp=/usr/libexec/xdg-desktop-portal
  xdp_wlr="$(command -v xdg-desktop-portal-wlr || true)"
  [[ -z "$xdp_wlr" && -x /usr/libexec/xdg-desktop-portal-wlr ]] && xdp_wlr=/usr/libexec/xdg-desktop-portal-wlr

  if [[ -n "$xdp" ]]; then
    pkill -u "$(id -u)" -f xdg-desktop-portal$ >/dev/null 2>&1 || true
    "$xdp" -r >"$OUT_DIR/xdg-desktop-portal.log" 2>&1 &
  fi
  if [[ -n "$xdp_wlr" ]]; then
    pkill -u "$(id -u)" -f xdg-desktop-portal-wlr >/dev/null 2>&1 || true
    chmod +x "$E2E_ROOT/nest/accept-output.sh" 2>/dev/null || true
    # -r replace; force line-buffered logs if stdbuf exists.
    if command -v stdbuf >/dev/null 2>&1; then
      stdbuf -oL -eL "$xdp_wlr" -r >"$OUT_DIR/xdg-desktop-portal-wlr.log" 2>&1 &
    else
      "$xdp_wlr" -r >"$OUT_DIR/xdg-desktop-portal-wlr.log" 2>&1 &
    fi
    echo "portal-wlr started (config=$HOME/.config/xdg-desktop-portal-wlr/config)"
  else
    echo "warn: xdg-desktop-portal-wlr not found; ScreenCast will fail" >&2
  fi
  sleep 1.2
}
