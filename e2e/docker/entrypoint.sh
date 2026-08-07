#!/usr/bin/env bash
# Container entrypoint: fix volume perms, drop to e2e, start dbus, run harness.
set -euo pipefail

ROOT="${ROOT:-/src}"
E2E="$ROOT/e2e"

# When launched as root (sudo docker / default image user), reclaim bind mounts
# and re-exec as the non-root e2e user (Sway refuses UID 0).
if [[ "$(id -u)" -eq 0 ]]; then
  mkdir -p "$E2E/reports" "$E2E/goldens" /tmp/runtime-vectrace /home/e2e
  chown -R e2e:e2e "$E2E/reports" "$E2E/goldens" /tmp/runtime-vectrace /home/e2e 2>/dev/null || true
  # Optional: install grim if the image predates the Dockerfile change.
  if ! command -v grim >/dev/null 2>&1; then
    echo "Installing grim for docker Wayland capture fallback..."
    apt-get update -qq >/tmp/apt-grim.log 2>&1 \
      && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq grim >>/tmp/apt-grim.log 2>&1 \
      || echo "warn: could not install grim (see /tmp/apt-grim.log)"
  fi
  # Re-exec this script as e2e with the same args.
  exec runuser -u e2e -- env \
    HOME=/home/e2e \
    USER=e2e \
    E2E_PROFILE="${E2E_PROFILE:-docker}" \
    PROBE_BIN="${PROBE_BIN:-/usr/local/bin/vectrace-e2e-probe}" \
    XDG_RUNTIME_DIR=/tmp/runtime-vectrace \
    "$0" "$@"
fi

export PROBE_BIN="${PROBE_BIN:-/usr/local/bin/vectrace-e2e-probe}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/runtime-vectrace}"
export HOME="${HOME:-/home/e2e}"
export E2E_PROFILE="${E2E_PROFILE:-docker}"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

# Private session bus for portals / PipeWire clients.
if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then
  if command -v dbus-daemon >/dev/null 2>&1; then
    dbus-daemon --session --address="unix:path=$XDG_RUNTIME_DIR/bus" --fork
    export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
  fi
fi

# Best-effort PipeWire (needed if a Wayland nest tries ScreenCast).
if command -v pipewire >/dev/null 2>&1; then
  pipewire >/tmp/pipewire.log 2>&1 &
  if command -v wireplumber >/dev/null 2>&1; then
    wireplumber >/tmp/wireplumber.log 2>&1 &
  fi
fi

cmd="${1:-run-docker}"
shift || true

case "$cmd" in
  run-docker)
    exec "$E2E/harness.sh" run-docker "$@"
    ;;
  run)
    exec "$E2E/harness.sh" run "$@"
    ;;
  list)
    exec "$E2E/harness.sh" list
    ;;
  run-all)
    exec "$E2E/harness.sh" run-all "$@"
    ;;
  shell)
    exec bash "$@"
    ;;
  *)
    exec "$E2E/harness.sh" "$cmd" "$@"
    ;;
esac
