# Changelog

All notable changes to the Vectrace project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Automated release management script integration with CHANGELOG validation.

---

## [0.2.3] - 2026-08-10

### Fixed
- Fixed autostart session freeze and infinite logout loop on Ubuntu 26.04 (GNOME Shell/Wayland/XWayland).
- Added `--start-in-tray`, `--hidden`, and `--minimized` command-line flags to launch Vectrace in the system tray without taking window focus or exclusive keyboard grabs at boot.
- Updated XDG autostart desktop entry (`com.vectrace.Vectrace.desktop`) to execute `vectrace --start-in-tray` automatically.

---

## [0.2.2] - 2026-08-05

### Added
- Full screen region crop selection tool (`Save Region`) triggered from the toolbar and system tray.
- Multi-monitor bounds detection for primary vs all-monitor overlay modes.
- Neon laser stroke decay and spotlight tool mode.

### Improved
- Spline rendering performance and smooth stroke interpolation.
- Keyboard focus proxy mechanism under XWayland to release seat focus cleanly.

---

## [0.2.1] - 2026-08-01

### Added
- System tray status notifier (`ksni`) integration.
- Floating glassmorphism UI toolbar with tool selection, color picker, and settings menu.

---

## [0.1.1] - 2026-07-20

### Added
- Initial release of Vectrace screen marker for Linux (X11 & Wayland backend support).
