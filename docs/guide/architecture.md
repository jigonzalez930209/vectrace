# Architecture & Rendering Engine

Vectrace is designed with a highly modular architecture where no single file exceeds 500 lines of code.

```
src/
├── core/
│   ├── canvas.rs      # Canvas state, strokes, undo/redo
│   ├── render.rs      # Optimized tiny-skia stroke rendering & Porter-Duff text compositing
│   ├── export.rs      # Pixmap crop & image export
│   └── toast.rs       # On-screen notifications
├── platform/
│   ├── x11/           # X11 backend (window, render, input handlers)
│   ├── wayland/       # Wayland backend (layer-shell & portal)
│   └── clipboard.rs   # Cross-platform clipboard provider
└── ui/
    ├── toolbar.rs      # Toolbar layout state & actions
    ├── toolbar_draw.rs # Glassmorphic toolbar rendering
    └── toolbar_icons.rs# Vector icon graphics
```
