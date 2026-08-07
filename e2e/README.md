# Local multi-compositor e2e harness (not run by CI)

Vectrace capture and overlay behavior depends on the desktop session. This
directory hosts a **local-only** matrix that probes detection + desktop capture
across compositors. GitHub Actions continues to run offline `cargo test` only.

## Quick start (host session)

```bash
# Build probe + list scenarios
cargo build --bin vectrace-e2e-probe
./e2e/harness.sh list

# Attach to the current session (e.g. GNOME Wayland) — this is how you got
# e2e/reports/gnome-wayland/.../capture.png
./e2e/harness.sh run gnome-wayland

# Nested compositors if installed on the host
./e2e/harness.sh run openbox-x11
./e2e/harness.sh run-all
```

## Docker: run nested matrix without installing compositors

Your host GNOME capture only covers **one** cell of the matrix. To exercise
Xephyr/Openbox (and best-effort Weston/Sway) in an isolated environment:

```bash
# Requires Docker (or Podman) + compose plugin
./e2e/docker/docker-run.sh build
./e2e/docker/docker-run.sh run          # harness run-docker inside the image
./e2e/docker/docker-run.sh run-one openbox-x11
./e2e/docker/docker-run.sh shell        # debug
```

Reports are bind-mounted to `e2e/reports/` on the host.

| Runs in Docker? | Scenarios |
|-----------------|-----------|
| **Yes (required)** | `openbox-x11`, `xfce-x11`, `i3-x11` (Xvfb → X11 `GetImage`); `sway-wayland` (nested under Weston + portal-wlr) |
| **Yes (best-effort)** | `weston-wayland` (no wlr ScreenCast portal) |
| **No — host attach** | `gnome-wayland`, `gnome-x11`, `kde-plasma-*`, Cinnamon, MATE |
| **No — not packaged** | `hyprland` (not in Debian image), river/niri (blocked) |

**Why Xephyr failed in Docker:** Xephyr nests *on top of* an existing X display.
Containers have none, so the nest script uses **Xvfb** when `E2E_PROFILE=docker`.

**Why Sway failed as root:** Sway refuses to start as UID 0. The image runs as user
`e2e` (uid 1000) and nests Sway under Weston headless when needed.

## Concepts

| Term | Meaning |
|------|---------|
| **attach** | Run the probe in your current graphical session (GNOME, KDE, …). |
| **nested** | Spawn an isolated compositor (`sway`, `weston`, `Xephyr`) and probe inside it. |
| **docker** | Same nested runners, but inside `e2e/docker` so the host need not install them. |
| **blocked** | Scenario is catalogued but not runnable yet (`blocked_reason` in `matrix.yaml`). |

Two independent axes are validated:

1. **Overlay hint** — `layer_shell` / `xwayland` / `x11` (how Vectrace would place the overlay).
2. **Capture path** — `mutter_screencast` / `xdg_screencast` / `x11_root` / `screenshot_flash`.

The probe never enables the GNOME Screenshot flash path unless you export
`VECTRACE_ALLOW_FLASH`. Matrix scenarios set `flash_forbidden: true`.

## Artifacts

Each run writes:

```text
e2e/reports/<scenario_id>/<unix_ts>/
  report.json
  capture.png
  stdout.log
  *.log          # compositor logs for nested runs
e2e/reports/<scenario_id>/latest   # pointer file
```

`report.json` fields used by compare/bless: `ok`, `capture_path`, `width`,
`height`, `overlay_hint`, `failures`.

## Prerequisites by scenario

| Scenario | Needs |
|----------|--------|
| `gnome-wayland` (attach) | Logged into GNOME Wayland; PipeWire; Mutter ScreenCast |
| `kde-plasma-*` (attach) | Logged into Plasma; portals + PipeWire for Wayland |
| Docker nested X11 | `./e2e/docker/docker-run.sh` (image includes Xephyr + openbox) |
| Host `sway` / `weston` / `Xephyr` | Install locally **or** use Docker |

Optional for nicer X11 nests on the host: `xsetroot`.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Probe + expect checks passed |
| 1 | Capture or expect failed |
| 2 | Soft skip (blocked scenario or missing nested binary) |

## CI policy

Do **not** wire `e2e/harness.sh` or Docker e2e into `.github/workflows` by default
(image build + privileged nested compositors are heavy). Keep PR CI on
unit/integration tests. Run Docker e2e locally when changing capture/overlay code.

## Extending the matrix

1. Add an entry to [`matrix.yaml`](matrix.yaml).
2. If `runner: nested`, add `e2e/nest/<name>.sh` and point `nest_script`.
3. Set `docker: true` (must pass in container) or `docker: best_effort`.
4. Run `./e2e/harness.sh run <id>` or `./e2e/docker/docker-run.sh run-one <id>`.
