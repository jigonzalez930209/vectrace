#!/usr/bin/env bash
# Nested Sway for e2e. In Docker/headless, nest under Weston (no DRM/root seat).
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

skip_missing sway

export NEST_SESSION_TYPE=wayland
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/vectrace-e2e-runtime-$$}"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR" 2>/dev/null || true

CFG="$E2E_ROOT/nest/sway.config"
PARENT_PID=""
SWAY_PID=""
PARENT_SOCK=""

cleanup() {
  if [[ -n "${SWAY_PID}" ]]; then
    kill "$SWAY_PID" 2>/dev/null || true
    wait "$SWAY_PID" 2>/dev/null || true
  fi
  if [[ -n "${PARENT_PID}" ]]; then
    kill "$PARENT_PID" 2>/dev/null || true
    wait "$PARENT_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# Prefer nesting under Weston headless when there is no usable DRM seat.
need_parent=0
if [[ "${E2E_PROFILE:-}" == "docker" ]]; then
  need_parent=1
elif [[ "$(id -u)" -eq 0 ]]; then
  need_parent=1
elif [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
  need_parent=1
fi

if [[ "$need_parent" -eq 1 ]]; then
  skip_missing weston
  unset DISPLAY
  PARENT_SOCK="wayland-parent-$$"
  weston --backend=headless-backend.so --socket="$PARENT_SOCK" --width=1280 --height=720 \
    >/dev/null 2>"$OUT_DIR/weston-parent.log" &
  PARENT_PID=$!
  if ! wait_for 15 bash -c "[[ -S \"$XDG_RUNTIME_DIR/$PARENT_SOCK\" ]]"; then
    echo "Weston parent failed; see $OUT_DIR/weston-parent.log" >&2
    cat "$OUT_DIR/weston-parent.log" >&2 || true
    exit 1
  fi
  export WAYLAND_DISPLAY="$PARENT_SOCK"
  export WLR_BACKENDS=wayland
  echo "Nesting Sway under Weston parent=$WAYLAND_DISPLAY"
fi

# Snapshot sockets before Sway so we can detect the new compositor socket.
mapfile -t BEFORE_SOCKS < <(list_wayland_sockets | xargs -n1 basename 2>/dev/null || true)

unset DISPLAY
sway -c "$CFG" --unsupported-gpu >/dev/null 2>"$OUT_DIR/sway.log" &
SWAY_PID=$!

# Wait for a NEW wayland socket that is not the Weston parent.
find_sway_socket() {
  local f base
  while IFS= read -r f; do
    base="$(basename "$f")"
    if [[ -n "$PARENT_SOCK" && "$base" == "$PARENT_SOCK" ]]; then
      continue
    fi
    local seen=0
    local b
    for b in "${BEFORE_SOCKS[@]:-}"; do
      [[ "$b" == "$base" ]] && seen=1 && break
    done
    if [[ "$seen" -eq 0 ]]; then
      printf '%s\n' "$f"
      return 0
    fi
  done < <(list_wayland_sockets)
  return 1
}

if ! wait_for 25 find_sway_socket; then
  echo "Sway failed to create its own Wayland socket; see $OUT_DIR/sway.log" >&2
  echo "sockets now:" >&2
  list_wayland_sockets >&2 || true
  cat "$OUT_DIR/sway.log" >&2 || true
  exit 1
fi

SOCKET="$(find_sway_socket)"
export WAYLAND_DISPLAY="$(basename "$SOCKET")"
unset WLR_BACKENDS
echo "Nested Sway WAYLAND_DISPLAY=$WAYLAND_DISPLAY (pid=$SWAY_PID)"

# ScreenCast via portal-wlr (auto-picks the single nested output).
start_wayland_portals

sleep 0.5
set +e
run_probe_from_args_file
probe_rc=$?
set -e

if [[ "$probe_rc" -eq 0 ]]; then
  exit 0
fi

# Docker/nested: portal-wlr often denies Start ("Operation not permitted") even
# with chooser_type=none. grim uses the same wlr-screencopy path and is enough
# to prove the Sway nest is capturable; matrix accepts capture_path=grim.
if [[ "${E2E_PROFILE:-}" == "docker" ]] && command -v grim >/dev/null 2>&1; then
  echo "Portal probe failed (rc=$probe_rc); trying grim fallback..."
  if grim "$OUT_DIR/capture.png" 2>"$OUT_DIR/grim.log"; then
    python3 - "$OUT_DIR" <<'PY'
import json, struct, sys
from pathlib import Path
out = Path(sys.argv[1])
png = out / "capture.png"
data = png.read_bytes()
if data[:8] != b"\x89PNG\r\n\x1a\n":
    raise SystemExit("grim produced non-PNG")
w, h = struct.unpack(">II", data[16:24])
report = {
    "scenario_id": "sway-wayland",
    "ok": True,
    "capture_path": "grim",
    "width": w,
    "height": h,
    "overlay_hint": "layer_shell",
    "session": {
        "wayland_display": None,
        "display": None,
        "session_type": "wayland",
    },
    "png_path": str(png),
    "error": None,
    "failures": [],
    "note": "docker grim fallback after portal-wlr Start denial",
}
(out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
(out / "stdout.log").write_text(
    f"capture_path=grim\nsize={w}x{h}\noverlay_hint=layer_shell\n"
)
print(f"grim fallback OK {w}x{h}")
if w < 1280 or h < 720:
    raise SystemExit(f"grim size {w}x{h} below min 1280x720")
PY
    exit 0
  fi
  echo "grim fallback failed; see $OUT_DIR/grim.log" >&2
fi

exit "$probe_rc"
