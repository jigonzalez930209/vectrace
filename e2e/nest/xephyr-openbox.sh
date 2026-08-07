#!/usr/bin/env bash
# Nested virtual X11 + Openbox (proxy for openbox/xfce/i3 scenarios).
# Prefer Xephyr when a parent DISPLAY exists; otherwise use Xvfb (Docker/headless).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

WM="openbox"
if ! command -v openbox >/dev/null 2>&1; then
  if command -v twm >/dev/null 2>&1; then
    WM="twm"
  else
    echo "SKIP: need openbox or twm"
    exit 2
  fi
fi

export NEST_SESSION_TYPE=x11
unset WAYLAND_DISPLAY

# Optional parent X for Xephyr (host session). Saved before we allocate a new :N.
PARENT_DISPLAY="${HOST_DISPLAY:-}"
if [[ -z "$PARENT_DISPLAY" && -n "${DISPLAY:-}" ]]; then
  if xdpyinfo >/dev/null 2>&1; then
    PARENT_DISPLAY="$DISPLAY"
  fi
fi

DISP_NUM=77
while [[ -e "/tmp/.X${DISP_NUM}-lock" ]]; do
  DISP_NUM=$((DISP_NUM + 1))
  if (( DISP_NUM > 90 )); then
    echo "No free X display" >&2
    exit 1
  fi
done
NEST_DISPLAY=":${DISP_NUM}"

use_xvfb=0
if [[ "${E2E_PROFILE:-}" == "docker" ]]; then
  use_xvfb=1
elif [[ -z "$PARENT_DISPLAY" ]]; then
  use_xvfb=1
elif ! command -v Xephyr >/dev/null 2>&1; then
  use_xvfb=1
fi

X_PID=""
if [[ "$use_xvfb" -eq 1 ]]; then
  skip_missing Xvfb
  Xvfb "$NEST_DISPLAY" -screen 0 1280x720x24 -ac -nolisten tcp \
    >/dev/null 2>"$OUT_DIR/xserver.log" &
  X_PID=$!
  echo "Using Xvfb $NEST_DISPLAY"
else
  skip_missing Xephyr
  DISPLAY="$PARENT_DISPLAY" Xephyr "$NEST_DISPLAY" -screen 1280x720 -ac -br -reset -terminate \
    >/dev/null 2>"$OUT_DIR/xserver.log" &
  X_PID=$!
  echo "Using Xephyr $NEST_DISPLAY (parent $PARENT_DISPLAY)"
fi

export DISPLAY="$NEST_DISPLAY"

cleanup() {
  if [[ -n "${XTERM_PID:-}" ]]; then
    kill "$XTERM_PID" 2>/dev/null || true
  fi
  if [[ -n "${WM_PID:-}" ]]; then
    kill "$WM_PID" 2>/dev/null || true
  fi
  if [[ -n "${X_PID}" ]]; then
    kill "$X_PID" 2>/dev/null || true
    wait "$X_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

if ! wait_for 10 bash -c "xdpyinfo -display $DISPLAY >/dev/null 2>&1"; then
  echo "Virtual X server failed; see $OUT_DIR/xserver.log" >&2
  cat "$OUT_DIR/xserver.log" >&2 || true
  exit 1
fi

"$WM" >/dev/null 2>"$OUT_DIR/wm.log" &
WM_PID=$!
sleep 0.5

# Openbox/twm paint a black root on start — set the solid color AFTER the WM.
if command -v xsetroot >/dev/null 2>&1; then
  xsetroot -solid '#3d5a80'
fi
# Extra non-black pixels: a small xterm if available (survives WM root redraws).
XTERM_PID=""
if command -v xterm >/dev/null 2>&1; then
  xterm -bg '#e07a3d' -fg white -geometry 40x12+80+80 -e sleep 600 \
    >/dev/null 2>&1 &
  XTERM_PID=$!
  sleep 0.3
fi

echo "Nested X11 DISPLAY=$DISPLAY WM=$WM (xvfb=$use_xvfb)"
run_probe_from_args_file
