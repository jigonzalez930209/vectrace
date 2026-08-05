# Roadmap de Desarrollo - Vectrace Screen Marker

La arquitectura de **Vectrace** se basa en desacoplar completamente el **motor de renderizado vectorial agnóstico** de los **backends de ventana del sistema gráfico Linux** (X11, XWayland y Wayland).

---

## Stack de Crates Implementado

| Componente | Crate Rust | Estado | Propósito |
| --- | --- | --- | --- |
| **Renderizado 2D Vectorial** | `tiny-skia` + `fontdue` | `[x] Activo` | Renderizado vectorial puro en software de alta fidelidad, suavizado y rasterización de fuentes. |
| **Backend X11 / XWayland** | `x11rb` | `[x] Activo` | Cliente X11 puro para manejo de transparencia 32-bit ARGB, `XShape` passthrough y atajos globales `XGrabKey`. |
| **Backend Wayland Nativo** | `wayland-client` + `wayland-protocols` | `[x] Activo` | Integración nativa con `zwlr_layer_shell_v1` y `xdg_wm_base` (XDG Shell). |
| **Estructura de Datos** | `vectrace::core` | `[x] Activo` | Algoritmos de suavizado Catmull-Rom, pilaUndo/Redo y motor de formas geométricas. |

---

## Estructura del Proyecto (`src/`)

```text
vectrace/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── core/
    │   ├── mod.rs
    │   ├── canvas.rs          # Motor de dibujo vectorial, suavizado e historial Undo/Redo
    │   └── tools.rs           # Lápiz, resaltador, láser, spotlight, formas (línea, flecha, rect, óvalo)
    ├── platform/
    │   ├── mod.rs             # Trait `PlatformBackend` y selección de backend
    │   ├── x11/               # Backend X11 / XWayland (x11rb, XShape, XGrabKey, XGrabKeyboard)
    │   └── wayland/           # Backend Wayland nativo (layer-shell + XDG Shell)
    └── ui/
        ├── toolbar.rs         # Barra de herramientas flotante glassmorphic con escalado HiDPI
        └── mod.rs
```

---

## Estado Actual de las Fases del Roadmap

### [x] Fase 1: Motor Gráfico Vectorial (Canvas Agnóstico)
- `[x]` Estructuras de datos vectoriales (`Stroke`, `Point`, `Color`, `BlendMode`).
- `[x]` Algoritmo de suavizado de trazos con splines **Catmull-Rom**.
- `[x]` Renderizado 2D de alta fidelidad con `tiny-skia` sobre buffer con transparencia alfa.
- `[x]` Historial de comandos vectoriales para **Undo** (`U`) y **Redo** (`R`).

---

### [x] Fase 2: Backend X11 & Transparencia 32-bit
- `[x]` Creación de ventana sobrepuesta a pantalla completa.
- `[x]` Visual ARGB de 32 bits para soporte de transparencia 100% real.
- `[x]` **Pass-Through de Clics**: Alternancia dinámica de la máscara de entrada con la extensión `XShape` (`x11rb::protocol::shape`).
- `[x]` Blitting de sub-rectángulos sucios (*dirty rects*) para alto rendimiento a 1000Hz+ sin lag.

---

### [x] Fase 3: Backend Wayland Nativos y XWayland
- `[x]` Integración nativa con `zwlr_layer_shell_v1` (Sway, Hyprland, KDE).
- `[x]` Integración nativa con `xdg_wm_base` (XDG Shell para GNOME Wayland).
- `[x]` Fallback automático a backend XWayland transparente de 32-bits para máxima compatibilidad con todos los escritorios Linux.

---

### [x] Fase 4: Herramientas Avanzadas y Shaders
- `[x]` **Puntero Láser Neón:** Rastro con brillo neón y decaimiento temporal automático en 1.2 segundos.
- `[x]` **Modo Spotlight / Foco (Lupa):** Icono de lupa, máscara oscura a pantalla completa con corte circular activado *on-click*.
- `[x]` **Formas Geométricas en Tiempo Real:** Vista previa fluida sin sombreado fantasma (*ghosting*) para Línea, Flecha, Rectángulo y Óvalo.
- `[x]` **Modo Pizarra / Pizarrón:** Alternancia entre fondo Transparente, Pizarra Oscura (`#18181C`) y Pizarrón Blanco (`#FAFAFA`) mediante tecla `B` o botón en barra.
- `[ ]` **[TODO] Cajón de Texto Interactivo ("T"):** Herramienta de texto con cuadro interactivo y entrada de teclado dedicada (Pospuesto para refinamiento).

---

### [x] Fase 5: Multimonitor, HiDPI y Atajos Globales
- `[x]` **Geometría Multimonitor:** Cálculo automático del bounding box del escritorio virtual completo sobre múltiples pantallas.
- `[x]` **Escalado HiDPI Dinámico:** Soporte para factores de escala de pantalla (`GDK_SCALE`, `QT_SCALE_FACTOR`, `VECTRACE_SCALE`) adaptando iconos, bordes y grosores de trazo para pantallas 4K / Retina.
- `[x]` **Atajo Global de Sistema (`Ctrl + Alt + A`):** Captura de atajo global nativo con `XGrabKey` para alternar la visibilidad y passthrough de Vectrace desde cualquier aplicación.

---

### [ ] Fase 6: Empaquetado Multidistro y CI/CD (Próxima Fase)
- `[ ]` **Integración en el Sistema:** Archivo `.desktop` (`com.vectrace.Vectrace.desktop`) e icono SVG (`vectrace.svg`).
- `[ ]` **Paquete DEB:** Configuración `cargo-deb` para Debian, Ubuntu y Linux Mint.
- `[ ]` **Paquete RPM:** Configuración `cargo-generate-rpm` para Fedora, RHEL y openSUSE.
- `[ ]` **AppImage:** Script ejecutable ejecutable portable independiente.
- `[ ]` **Receta Arch Linux:** Archivo `PKGBUILD`.
- `[ ]` **Pipeline CI/CD:** GitHub Actions (`.github/workflows/ci.yml`) para pruebas y publicación de artefactos automatizada.
