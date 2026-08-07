# Introduction to Vectrace

**Vectrace** is an ultra-fast, pure-software vector screen marker and drawing overlay designed for Linux. It provides presenters, educators, content creators, and developers with an instant drawing canvas over their active desktop.

![Hero Banner](/images/hero-banner.png)

## Core Value Proposition

- **No GPU Overhead**: Built entirely with `tiny-skia` for predictable performance across low-end and high-end hardware.
- **Display Server Agnostic**: Operates seamlessly under X11 (via unmapped input shapes) and Wayland (via `wlr-layer-shell` or XDG protocols).
- **Non-Intrusive**: Toggle click-through mode anytime (`Ctrl+Alt+A` or `Space`) to interact with underlying windows without losing your annotations.
