# Development Roadmap - Vectrace Screen Marker

The architecture of **Vectrace** is based on completely decoupling the **agnostic vector rendering engine** from the **graphics window backends on Linux** (X11, XWayland, and Wayland).

---

## Implemented Crate Stack

| Component | Rust Crate | Status | Purpose |
| --- | --- | --- | --- |
| **Vector 2D Rendering** | `tiny-skia` + `fontdue` | `[x] Active` | High-fidelity pure software vector rendering, anti-aliasing, and font rasterization. |
| **X11 / XWayland Backend** | `x11rb` | `[x] Active` | Pure X11 client for 32-bit ARGB transparency, `XShape` passthrough, and `XGrabKey` global daemon shortcuts. |
| **Native Wayland Backend** | `wayland-client` + `wayland-protocols` | `[x] Active` | Native integration with `zwlr_layer_shell_v1` and `xdg_wm_base` (XDG Shell). |
| **System Tray & DBus Menu** | `ksni` + `zbus` + `tokio` | `[x] Active` | Cross-desktop Linux system tray icon (`StatusNotifierItem`) and interactive DBus context menu. |
| **Data Architecture** | `vectrace::core` | `[x] Active` | Catmull-Rom spline smoothing algorithms, Undo/Redo command stack, and shape engine. |

---

## Project Directory Structure (`src/`)

```text
vectrace/
├── Cargo.toml
├── roadmap.md
└── src/
    ├── main.rs
    ├── core/
    │   ├── mod.rs
    │   ├── canvas.rs          # Vector canvas engine, stroke smoothing, and Undo/Redo history
    │   ├── config.rs          # Monitor modes, scale factors, and configuration
    │   └── tools.rs           # Pen, highlighter, laser, spotlight, shapes (line, arrow, rect, oval)
    ├── platform/
    │   ├── mod.rs             # `PlatformBackend` trait and backend selection logic
    │   ├── tray.rs            # System tray icon (StatusNotifierItem & DBus menu integration)
    │   ├── x11/               # X11 / XWayland backend (x11rb, XShape, XGrabKey, XGrabKeyboard)
    │   └── wayland/           # Native Wayland backend (layer-shell + XDG Shell)
    └── ui/
        ├── toolbar.rs         # Compact 20% smaller floating glassmorphic toolbar with color popup & drag handle
        └── mod.rs
```

---

## Current Roadmap Phase Status

### [x] Phase 1: Vector Graphics Engine (Agnostic Canvas)
- `[x]` Vector data structures (`Stroke`, `Point`, `Color`, `BlendMode`).
- `[x]` Stroke smoothing algorithm using **Catmull-Rom** splines.
- `[x]` High-fidelity 2D rendering with `tiny-skia` on alpha-transparent pixel buffers.
- `[x]` Vector command stack for **Undo** (`U`) and **Redo** (`R`).

---

### [x] Phase 2: X11 Backend & 32-bit Transparency
- `[x]` Fullscreen overlay window creation across single and multi-monitors.
- `[x]` True 32-bit ARGB visual support for real 100% transparency.
- `[x]` **Click Passthrough**: Dynamic input shape masking via `XShape` extension (`x11rb::protocol::shape`).
- `[x]` **Offscreen 1x1 Input Region (`-32000, -32000`)**: Prevents GNOME XWayland full-screen input mask reset when minimized to System Tray.
- `[x]` Dirty sub-rectangle blitting for 1000Hz+ high-performance rendering without lag.

---

### [x] Phase 3: Native Wayland & XWayland Backends
- `[x]` Native `zwlr_layer_shell_v1` protocol integration (Sway, Hyprland, KDE).
- `[x]` Native `xdg_wm_base` protocol integration (XDG Shell for GNOME Wayland).
- `[x]` Automatic fallback to 32-bit transparent XWayland backend for universal compatibility across all Linux desktops.

---

### [x] Phase 4: Advanced Tools, Shapes & Effects
- `[x]` **Neon Laser Pointer:** Neon glow trail with automatic 1.2-second temporal decay.
- `[x]` **Spotlight / Lens Mode:** Circular spotlight cutout with dark full-screen mask toggled on-click.
- `[x]` **Real-time Geometric Shapes:** Smooth live preview without ghosting for Line, Arrow, Rectangle, and Oval.
- `[x]` **Blackboard / Whiteboard Mode:** Toggling between Transparent, Dark Blackboard (`#18181C`), and Whiteboard (`#FAFAFA`) via key `B` or toolbar button.
- `[x]` **Text Box Tool:** Interactive text input box with dedicated keyboard input.

---

### [x] Phase 5: Multi-Monitor, System Tray, Color Menu & Screen Dragging
- `[x]` **Multi-Monitor Geometry:** Automatic primary display auto-detection and toolbar centering (`mon_x + (mon_w - toolbar_w) / 2`).
- [x] **Dynamic HiDPI Scaling:** Display scale factor detection (`GDK_SCALE`, `QT_SCALE_FACTOR`, `VECTRACE_SCALE`) scaling icons, stroke widths, and borders for 4K / Retina displays.
- [x] **Global System Daemon Shortcut (`Ctrl + Alt + A`):** Native global shortcut capture via `XGrabKey` to toggle Vectrace visibility and passthrough from any app.
- [x] **Multi-Monitor Mode Selection:** Dynamic switching between *Primary Monitor* mode and *All Monitors* extended mode.
- [x] **System Tray Icon & Options Menu:** System tray icon (`ksni` StatusNotifierItem) with interactive DBus popup menu for visibility, display mode, passthrough, background mode, clear canvas, and quit.
- [x] **20% Toolbar Size Reduction:** Compact modern toolbar UI (width `660px`, height `38px`).
- [x] **Color Selection Popup Menu (🎨):** Color palette button with an interactive grid of 12 vibrant colors (Red, Orange, Yellow, Green, Cyan, Blue, Purple, Pink, White, Light Gray, Dark Gray, Black).
- [x] **60fps+ Screen-Wide Dragging:** Interactive drag grip handle (`⠿`) with motion event coalescing and dirty bounding box rendering (~0.1MB/frame) for butter-smooth dragging.

---

### [x] Phase 6: Multi-Distro Packaging & CI/CD
- `[x]` **Desktop System Integration:** Desktop entry file (`com.vectrace.Vectrace.desktop`) and SVG icon (`vectrace.svg`).
- `[x]` **DEB Package:** `cargo-deb` configuration for Debian, Ubuntu, and Linux Mint.
- `[x]` **RPM Package:** `cargo-generate-rpm` configuration for Fedora, RHEL, and openSUSE.
- `[x]` **AppImage:** Self-contained portable binary bundle.
- `[x]` **Arch Linux PKGBUILD:** Arch Linux AUR recipe package.
- `[x]` **CI/CD Pipeline:** GitHub Actions workflow (`.github/workflows/ci.yml`) for automated building, testing, and release packaging.
