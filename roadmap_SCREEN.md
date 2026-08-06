# Vectrace Development Roadmap

## Screen Annotation, Compositor Capture, and Export for Linux

**Target platforms:** Native Wayland, X11, and XWayland  
**Primary implementation language:** Rust  
**Document status:** Implementation roadmap  
**Last updated:** 2026-08-06

---

## 1. Project Vision

Vectrace is a Linux screen-annotation application whose architecture separates the agnostic vector-rendering engine from the operating-system and compositor backends.

The project must support two independent but composable visual sources:

1. **Desktop capture**, obtained from the display server or compositor.
2. **Vectrace annotations**, rendered by the internal vector engine.

A final snapshot is therefore not read from the transparent overlay. It is produced by explicitly composing both sources:

```text
Captured desktop frame
        +
Vectrace vector annotations
        =
Final exported image
```

This design is required because a transparent Wayland surface only owns its own pixels. It does not contain the pixels of the applications visible beneath it.

---

## 2. Architectural Principles

### 2.1 Strict separation of responsibilities

- `vectrace::core` owns vector objects, commands, history, document state, and rendering semantics.
- `vectrace::platform` owns windows, outputs, input, capture backends, portals, PipeWire, and X11 integration.
- `vectrace::snapshot` owns capture orchestration, coordinate mapping, image composition, encoding, and export policy.
- `vectrace::ui` owns user interaction, progress, permissions, errors, and export options.

### 2.2 Backend-independent snapshot API

The snapshot service must not know whether the desktop frame came from PipeWire, an X11 root pixmap, or another future backend.

```rust
pub trait ScreenCaptureBackend: Send {
    fn capabilities(&self) -> CaptureCapabilities;

    fn start(&mut self, request: CaptureRequest)
        -> Result<CaptureSessionId, CaptureError>;

    fn next_frame(&mut self, deadline: Instant)
        -> Result<CapturedFrame, CaptureError>;

    fn stop(&mut self) -> Result<(), CaptureError>;
}
```

### 2.3 Explicit capture modes

Vectrace must expose separate operations instead of overloading the word “snapshot”:

- **Annotations only:** transparent PNG containing only vector annotations.
- **Clean snapshot:** desktop without the Vectrace UI, plus annotations composited offscreen.
- **Visible composition:** capture the screen as presented by the compositor, optionally including overlay, toolbar, cursor, and transient effects.
- **Desktop only:** capture without Vectrace annotations.

### 2.4 Graceful degradation

Every capture backend must advertise its capabilities. Unsupported combinations must produce a precise user-facing explanation, not a blank image or silent failure.

---

## 3. Implemented Crate Stack

| Component | Rust crate or technology | Status | Purpose |
|---|---|---:|---|
| Vector 2D rendering | `tiny-skia` + `fontdue` | Active | Software vector rendering, antialiasing, alpha buffers, and text rasterization |
| X11/XWayland backend | `x11rb` | Active | ARGB overlay windows, XShape passthrough, global shortcuts, and X11 capture |
| Native Wayland backend | `wayland-client` + `wayland-protocols` | Active | Layer Shell and XDG Shell integration |
| System tray and D-Bus menu | `ksni` + `zbus` + `tokio` | Active | StatusNotifierItem, menus, and asynchronous D-Bus integration |
| Wayland capture authorization | XDG Desktop Portal ScreenCast | Planned | User-authorized monitor capture and session negotiation |
| Wayland frame transport | PipeWire | Planned | Delivery of compositor-produced frames |
| PNG encoding | Rust image/PNG encoder selected by implementation | Planned | Final snapshot export |
| Data architecture | `vectrace::core` | Active | Canvas, spline smoothing, tools, shapes, and Undo/Redo |

> The exact PipeWire Rust binding and PNG encoder must be selected after a small compatibility prototype. The public Vectrace interfaces must not expose types from those dependencies.

---

## 4. Target Project Structure

```text
vectrace/
├── Cargo.toml
├── roadmap.md
└── src/
    ├── main.rs
    ├── core/
    │   ├── mod.rs
    │   ├── canvas.rs
    │   ├── config.rs
    │   ├── document.rs
    │   ├── render.rs
    │   └── tools.rs
    ├── snapshot/
    │   ├── mod.rs
    │   ├── backend.rs
    │   ├── capabilities.rs
    │   ├── composition.rs
    │   ├── coordinates.rs
    │   ├── encoder.rs
    │   ├── error.rs
    │   ├── frame.rs
    │   ├── metadata.rs
    │   ├── request.rs
    │   └── service.rs
    ├── platform/
    │   ├── mod.rs
    │   ├── detection.rs
    │   ├── outputs.rs
    │   ├── tray.rs
    │   ├── x11/
    │   │   ├── mod.rs
    │   │   ├── capture.rs
    │   │   ├── overlay.rs
    │   │   ├── outputs.rs
    │   │   └── shortcuts.rs
    │   └── wayland/
    │       ├── mod.rs
    │       ├── overlay.rs
    │       ├── outputs.rs
    │       └── capture/
    │           ├── mod.rs
    │           ├── portal.rs
    │           ├── pipewire.rs
    │           ├── formats.rs
    │           ├── buffers.rs
    │           ├── session.rs
    │           └── restore_token.rs
    └── ui/
        ├── mod.rs
        ├── capture_dialog.rs
        ├── capture_status.rs
        ├── export_dialog.rs
        ├── permissions.rs
        └── toolbar.rs
```

---

## 5. Core Capture Data Model

### 5.1 Capture request

```rust
pub struct CaptureRequest {
    pub target: CaptureTarget,
    pub mode: SnapshotMode,
    pub cursor: CursorPolicy,
    pub include_toolbar: bool,
    pub include_transient_effects: bool,
    pub timeout: Duration,
}

pub enum CaptureTarget {
    PrimaryMonitor,
    Monitor(OutputId),
    AllMonitors,
}

pub enum SnapshotMode {
    AnnotationsOnly,
    CleanComposite,
    VisibleComposition,
    DesktopOnly,
}

pub enum CursorPolicy {
    Hidden,
    Embedded,
    Metadata,
}
```

### 5.2 Captured frame

```rust
pub struct CapturedFrame {
    pub output: OutputId,
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub format: CapturePixelFormat,
    pub memory: FrameMemory,
    pub transform: OutputTransform,
    pub sequence: u64,
    pub timestamp: Duration,
    pub damage: Vec<PixelRect>,
}

pub enum FrameMemory {
    Owned(Vec<u8>),
    MemoryMapped(MappedFrame),
    DmaBuf(DmaBufFrame),
}
```

### 5.3 Internal composition format

All backend frames must be normalized to one internal representation before composition:

- RGBA, 8 bits per channel.
- Known color-space behavior.
- Explicit alpha semantics.
- Explicit row stride.
- Top-left origin.
- Output transform already resolved, or represented in metadata and applied centrally.

No renderer or encoder may guess the incoming pixel ordering.

---

## 6. Wayland Capture Architecture: Variant B

### 6.1 Primary path

Native Wayland capture will use:

```text
XDG Desktop Portal ScreenCast
              ↓
User/compositor source selection and authorization
              ↓
OpenPipeWireRemote
              ↓
PipeWire stream negotiation
              ↓
Frame acquisition
              ↓
Format normalization
              ↓
Vectrace offscreen composition
              ↓
PNG export
```

The implementation must call the portal through the active user session D-Bus. It must never attempt to scrape arbitrary Wayland surfaces.

### 6.2 Portal session lifecycle

The state machine must represent the asynchronous portal flow explicitly:

```text
Idle
  ↓
CreatingSession
  ↓
SelectingSources
  ↓
StartingPortalSession
  ↓
OpeningPipeWireRemote
  ↓
NegotiatingPipeWireStream
  ↓
Streaming
  ↓
Stopping
  ↓
Closed
```

Every transition must support cancellation, timeout, and cleanup.

```rust
pub enum WaylandCaptureState {
    Idle,
    CreatingSession,
    SelectingSources,
    Starting,
    OpeningRemote,
    Negotiating,
    Streaming,
    Stopping,
    Closed,
    Failed(CaptureErrorKind),
}
```

### 6.3 Portal request sequence

Implement the following sequence:

1. Connect to `org.freedesktop.portal.Desktop` on the user session bus.
2. Call `CreateSession` with unique handle and session tokens.
3. Wait for the asynchronous request response.
4. Call `SelectSources`.
5. Request monitor sources. Allow multiple sources when “All Monitors” is selected.
6. Set the desired cursor mode when supported.
7. Request persistence only when supported by the portal and permitted by product policy.
8. Call `Start` with the appropriate parent-window identifier when available.
9. Parse stream node IDs and stream properties.
10. Call `OpenPipeWireRemote` and retain the returned file descriptor safely.
11. Connect the PipeWire client to that remote.
12. Negotiate a supported raw video format.
13. Start consuming frames.
14. Detect portal session closure and PipeWire disconnection.
15. Stop and release all resources deterministically.

### 6.4 Source-selection policy

- `PrimaryMonitor` requests a monitor source and maps the selected portal stream to the primary Vectrace output.
- `Monitor(OutputId)` requests a monitor source, but the portal may still require the user to select it manually.
- `AllMonitors` enables multiple monitor sources when supported.
- Vectrace must not claim it can silently choose a monitor when the compositor requires a user picker.
- Stream-to-output mapping must use portal metadata, geometry, resolution, and an explicit user fallback if automatic matching is ambiguous.

### 6.5 Session reuse and restore tokens

Session persistence is an optimization, not a correctness requirement.

- Store restore tokens only when returned by the portal.
- Associate them with the desktop environment, portal backend, application version, and capture policy.
- Treat rejection as normal and start a new interactive session.
- Never loop indefinitely when a persisted token becomes invalid.
- Provide a “Reset screen-capture permission” action.
- Do not assume that all compositors support silent restoration.

### 6.6 PipeWire negotiation

The first production version should prioritize reliability over zero-copy performance.

Preferred negotiation order:

1. Shared-memory or MemFd-backed raw frames in a supported 32-bit RGB format.
2. Additional raw formats with a conversion path.
3. DMA-BUF only after the shared-memory path is stable.

Initial pixel formats:

- BGRA8888
- BGRX8888
- RGBA8888
- RGBX8888

The implementation must handle:

- Plane offsets.
- Per-plane strides.
- Buffer padding.
- Buffer lifetime.
- Frame sequence and timestamps.
- Stream format changes.
- Damage metadata when available.
- Dropped frames.
- PipeWire stream reconnection or controlled failure.

### 6.7 DMA-BUF phase

DMA-BUF support is a later optimization and must be isolated behind `FrameMemory::DmaBuf`.

Requirements:

- Import supported DRM formats and modifiers.
- Detect unsupported modifiers without crashing.
- Fall back to shared memory where possible.
- Synchronize access correctly.
- Copy into an owned CPU buffer before invoking a CPU-only composition path, unless a future GPU compositor is introduced.
- Include GPU vendor and driver combinations in the test matrix.

### 6.8 Clean snapshot synchronization

A clean snapshot must avoid capturing the Vectrace overlay and then drawing annotations a second time.

Required sequence:

```text
Capture requested
    ↓
Freeze annotation document revision
    ↓
Hide overlay and toolbar surfaces
    ↓
Commit Wayland surface state
    ↓
Wait for compositor frame callback
    ↓
Discard stale PipeWire frames
    ↓
Accept the first frame newer than the synchronization boundary
    ↓
Restore overlay immediately
    ↓
Render frozen annotations offscreen
    ↓
Compose and encode
```

Suggested state machine:

```rust
pub enum CleanSnapshotState {
    Idle,
    FreezingDocument,
    HidingOverlay,
    WaitingForSurfaceCommit,
    WaitingForFreshFrame { minimum_sequence: u64 },
    RestoringOverlay,
    Compositing,
    Encoding,
    Completed,
    Failed,
}
```

Safety rules:

- Use a cleanup guard so the overlay is restored on success, error, panic boundary, cancellation, or timeout.
- Do not hold the UI event loop while waiting for PipeWire.
- Do not mutate the captured annotation revision during export.
- Ignore frames whose sequence or timestamp predates the accepted synchronization boundary.
- Impose finite timeouts on compositor and PipeWire waits.

Suggested initial limits:

```rust
const SURFACE_COMMIT_TIMEOUT: Duration = Duration::from_millis(750);
const FRESH_FRAME_TIMEOUT: Duration = Duration::from_secs(2);
const PORTAL_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
```

These values must be configurable and validated on slower or remotely hosted desktops.

### 6.9 Visible-composition mode

For a snapshot “as seen on screen”:

- Keep the overlay visible.
- Set the portal cursor mode according to the request.
- Wait until pending annotation rendering has been committed.
- Accept a compositor frame newer than that commit.
- Do not re-render annotations afterward.
- Clearly document that the portal or compositor may include additional system UI.

### 6.10 Wayland error categories

```rust
pub enum CaptureErrorKind {
    PortalUnavailable,
    PortalBackendMissing,
    UserCancelled,
    PermissionDenied,
    SessionClosed,
    InvalidPortalResponse,
    PipeWireUnavailable,
    PipeWireNegotiationFailed,
    UnsupportedPixelFormat,
    UnsupportedBufferType,
    SourceMappingFailed,
    FrameTimeout,
    CompositorSyncTimeout,
    OverlayRestoreFailed,
    EncodingFailed,
    Io,
    Internal,
}
```

User cancellation must not be reported as an application crash.

---

## 7. X11 Capture Architecture

### 7.1 Primary path

On a real X11 session, Vectrace can capture the root window directly through `x11rb`:

```text
Determine target monitor geometry
              ↓
Synchronize or hide Vectrace overlay
              ↓
Read pixels from the root drawable
              ↓
Decode visual masks and byte order
              ↓
Normalize to internal RGBA
              ↓
Compose frozen Vectrace annotations
              ↓
PNG export
```

### 7.2 X11 backend responsibilities

The X11 capture backend must:

- Discover screens, CRTCs, and monitor geometry through RandR where available.
- Support negative monitor origins.
- Select the root drawable associated with the target screen.
- Query visual and pixmap format information.
- Capture the requested region.
- Respect server byte order, depth, bits per pixel, scanline padding, and color masks.
- Normalize the result to the same internal RGBA representation used by Wayland.
- Return structured errors for unsupported visuals.

### 7.3 Capture mechanism

A correctness-first implementation may use `GetImage` on the root drawable for still images. It must avoid assuming 24-bit depth means three bytes per pixel, because many X11 configurations use 32 bits per pixel with 24-bit color depth.

Potential later optimizations:

- MIT-SHM for faster CPU readback.
- XComposite named pixmaps for specialized capture modes.
- Damage tracking for recording or continuous preview.

Those optimizations must not alter the public capture API.

### 7.4 Clean snapshot on X11

Use the same product semantics as Wayland:

```text
Freeze document revision
    ↓
Temporarily unmap or make overlay non-visible
    ↓
Synchronize with the X server
    ↓
Capture root-window pixels
    ↓
Remap overlay
    ↓
Render frozen annotations offscreen
    ↓
Compose and export
```

Implementation rules:

- Use an RAII guard to remap the overlay on every exit path.
- Flush requests before waiting.
- Perform a server round trip after changing visibility so capture does not race pending requests.
- Account for compositor animation or fade effects. If detected, support a small configurable stabilization delay.
- Keep the toolbar excluded in clean mode.

### 7.5 Visible-composition mode on X11

- Leave the overlay mapped.
- Flush rendering and perform a server synchronization round trip.
- Capture the root drawable.
- Do not render annotations a second time.
- Cursor inclusion requires a separate XFixes cursor-image path because root capture does not necessarily include the hardware cursor.

### 7.6 XWayland policy

An XWayland overlay is not equivalent to ownership of the Wayland-composited desktop.

Policy:

- If the session is Wayland and the application window happens to use XWayland, prefer the Wayland Portal + PipeWire backend.
- Use X11 root capture only in a true X11 desktop session or when capability probing proves it has access to the intended desktop.
- Never infer capture capability solely from the presence of the `DISPLAY` environment variable.

### 7.7 X11 error categories

- X server unavailable.
- RandR unavailable or inconsistent.
- Root drawable capture rejected.
- Unsupported depth, visual, or pixmap format.
- Region outside root geometry.
- X server synchronization timeout.
- Overlay restoration failure.
- Cursor retrieval failure.

---

## 8. Backend Detection and Selection

Backend selection must be capability-based and explicit.

```text
Session reports Wayland
    ├── Portal + PipeWire available → Wayland capture backend
    ├── Portal Screenshot fallback available → optional still-image fallback
    └── No authorized capture path → descriptive error

Session reports X11
    ├── X11 root capture available → X11 capture backend
    ├── Portal capture available → optional fallback
    └── No capture path → descriptive error
```

Detection inputs may include:

- `WAYLAND_DISPLAY`
- `DISPLAY`
- `XDG_SESSION_TYPE`
- Successful connection to the Wayland display
- Successful connection to the X server
- Portal availability
- PipeWire availability
- Runtime backend probes

Environment variables are hints. Successful protocol connections and capability probes are authoritative.

---

## 9. Multi-Monitor and HiDPI Coordinate Model

### 9.1 Coordinate spaces

Vectrace must name and separate all coordinate spaces:

- Global logical desktop coordinates.
- Per-output logical coordinates.
- Surface-local logical coordinates.
- Physical output pixels.
- PipeWire stream pixels.
- Export-image pixels.

```rust
pub struct OutputLayout {
    pub id: OutputId,
    pub logical_origin: LogicalPoint,
    pub logical_size: LogicalSize,
    pub stream_size: PixelSize,
    pub scale: ScaleFactor,
    pub transform: OutputTransform,
}
```

### 9.2 Required transformations

A centralized transform must support:

- Integer scaling.
- Fractional scaling.
- Rotation by 90, 180, and 270 degrees.
- Mirrored transforms when exposed.
- Negative desktop coordinates.
- Stream size different from advertised physical mode.
- Crop regions provided by the portal.

No tool may implement its own logical-to-pixel conversion.

### 9.3 All-monitors export

Preferred behavior:

1. Capture each output at its stream resolution.
2. Match every stream to an output layout.
3. Render annotations independently at the corresponding output resolution.
4. Compose each monitor image.
5. Optionally save one PNG per monitor.
6. Optionally stitch a global desktop image.

For mixed scaling, the global export policy must be explicit:

- **Native pixel layout:** preserves every monitor’s native captured pixels but requires a defined mapping into a single image.
- **Logical layout:** normalizes monitors to logical dimensions but resamples some outputs.
- **Separate files:** avoids ambiguous global resampling and should always be available.

### 9.4 Stream-to-monitor matching

Use the strongest available signals in this order:

1. Stable output identifier or connector metadata.
2. Portal stream position and size metadata.
3. Exact resolution and transform match.
4. User-selected mapping when ambiguity remains.

Never silently map two captured streams to the same output.

---

## 10. Composition Pipeline

### 10.1 Clean composite

```text
Captured backend frame
    ↓
Validate dimensions, stride, and buffer size
    ↓
Normalize pixel format
    ↓
Resolve output rotation and crop
    ↓
Create target RGBA buffer
    ↓
Render frozen vector document with tiny-skia
    ↓
Alpha-compose annotations over desktop
    ↓
Convert alpha representation for encoder
    ↓
Attach metadata
    ↓
Encode atomically
```

### 10.2 Annotation policies

The composition renderer must independently control:

- Permanent strokes.
- Shapes.
- Text boxes.
- Blackboard or whiteboard background.
- Laser trails.
- Spotlight masks.
- Selection handles.
- Toolbar and popups.
- Debug overlays.

Default clean-snapshot policy:

- Include permanent strokes, shapes, and text.
- Include blackboard/whiteboard background when active because it affects document meaning.
- Exclude toolbar, popups, selection handles, debug overlays, and capture indicators.
- Exclude laser and spotlight unless the user enables transient effects.

### 10.3 Atomic file output

- Encode to a temporary file in the destination directory.
- Flush and close successfully.
- Rename atomically to the requested final filename.
- Never leave a zero-byte final image after an interrupted export.
- Apply a collision policy such as timestamp or numeric suffix.

Suggested filename:

```text
Vectrace_2026-08-06_09-33-21_primary_clean.png
```

---

## 11. UI and User Experience

### 11.1 Snapshot menu

```text
Save annotations only
Save clean snapshot
Save screen as visible
Save desktop only
--------------------------------
Target: Primary monitor / Specific monitor / All monitors
Cursor: Hidden / Embedded / Metadata
Include transient effects
Include toolbar
Open destination folder after saving
```

Options that cannot apply to the selected mode must be disabled rather than ignored.

### 11.2 Permission UX on Wayland

Before the portal opens for the first time, explain briefly:

- The desktop compositor will ask which screen to share.
- Vectrace cannot bypass that operating-system dialog.
- Permission can be cancelled safely.
- Session reuse depends on the desktop environment.

### 11.3 Progress states

Expose meaningful progress:

- Requesting screen permission.
- Waiting for screen selection.
- Connecting to PipeWire.
- Waiting for a fresh compositor frame.
- Restoring overlay.
- Composing image.
- Saving PNG.

### 11.4 Failure recovery

- Always restore hidden overlays before showing an error.
- Offer “Try again” for transient failures.
- Offer “Reset capture permission” for invalid restore tokens.
- Offer annotation-only export when desktop capture is unavailable.
- Include a copyable technical-details section without exposing sensitive frame data.

---

## 12. Security and Privacy Requirements

- Screen capture must be initiated by an explicit user action unless a valid portal session is already active for a user-enabled workflow.
- Do not attempt to bypass portal consent.
- Keep PipeWire file descriptors and mapped frame buffers scoped to the capture session.
- Close sessions and file descriptors deterministically.
- Do not write raw frames to disk unless export succeeds or diagnostic capture was explicitly enabled.
- Redact file paths, window names, and portal metadata from normal logs when they may expose private information.
- Disable full-frame diagnostic dumps by default.
- Validate all buffer lengths, offsets, strides, dimensions, and arithmetic before memory access.
- Apply upper bounds to dimensions and allocation sizes.
- Treat portal and PipeWire metadata as untrusted input.

---

## 13. Performance and Reliability Targets

### 13.1 Still snapshot targets

- No permanent impact on normal annotation rendering.
- Overlay hidden for the shortest practical interval.
- UI remains responsive during authorization, frame wait, composition, and encoding.
- Memory usage bounded by target resolution and number of monitors.
- No unnecessary frame copies after normalization requirements are known.

### 13.2 Concurrency model

Recommended division:

- UI thread: commands, progress, and overlay state transitions.
- Portal async task: D-Bus request/response lifecycle.
- PipeWire processing context: stream callbacks and buffer acquisition.
- Worker task: normalization, composition, and encoding.

Rules:

- Never encode PNG inside a PipeWire callback.
- Copy or retain a frame according to PipeWire buffer lifetime rules before returning the buffer.
- Use bounded channels so repeated snapshot requests cannot exhaust memory.
- Serialize clean snapshots per display backend.
- Coalesce duplicate capture requests when appropriate.

---

## 14. Testing Strategy

### 14.1 Unit tests

- Pixel conversion for RGBA, RGBX, BGRA, and BGRX.
- Non-default stride and row padding.
- Premultiplied and straight-alpha composition.
- Logical-to-stream transforms.
- Fractional scale mapping.
- Rotated outputs.
- Negative monitor origins.
- Multi-monitor bounding boxes.
- Buffer length and overflow validation.
- State-machine cancellation from every state.
- Overlay restoration guard.
- Filename collision handling.

### 14.2 Golden-image tests

Create deterministic fixtures containing:

- Solid colors.
- Gradients.
- Alpha edges.
- Fine one-pixel lines.
- Text at multiple scales.
- Rotated monitors.
- Mixed-DPI layouts.
- Blackboard, whiteboard, and transparent modes.

Compare output with a documented tolerance for antialiasing differences.

### 14.3 Mock portal tests

Test:

- Successful session creation.
- User cancellation.
- Permission denial.
- Malformed response.
- Missing stream metadata.
- Multiple streams.
- Closed session during capture.
- Invalid restore token.
- Portal timeout.
- PipeWire FD acquisition failure.

### 14.4 PipeWire integration tests

- Supported shared-memory formats.
- Unexpected format renegotiation.
- Dropped frames.
- Stale frame rejection.
- Stream pause and resume.
- Remote disconnection.
- Buffer padding and nontrivial stride.
- Multiple simultaneous monitor streams.

### 14.5 X11 integration tests

- 24-bit depth with 32 bits per pixel.
- Different visual masks and byte orders where available.
- Single and multiple screens.
- RandR monitor layouts.
- Negative origins.
- Composited and non-composited desktops.
- Overlay hide, capture, and restoration.
- Cursor capture through XFixes.

### 14.6 Desktop matrix

Wayland:

- GNOME/Mutter.
- KDE Plasma/KWin.
- Sway/wlroots.
- Hyprland/wlroots.
- Integer and fractional scaling.
- Intel, AMD, NVIDIA, and software-rendered environments where practical.

X11:

- GNOME on Xorg.
- KDE Plasma on X11.
- Lightweight X11 window managers.
- Compositor enabled and disabled.

Packaging:

- Debian/Ubuntu.
- Fedora/RHEL family.
- Arch Linux.
- openSUSE.
- AppImage runtime.

### 14.7 Manual acceptance scenarios

1. Draw over two applications and save a clean Wayland snapshot.
2. Verify that both underlying applications and annotations appear.
3. Verify that toolbar and capture button do not appear.
4. Verify that annotations are not duplicated.
5. Repeat with fractional scaling.
6. Repeat on a rotated secondary monitor.
7. Cancel the portal and verify that the overlay remains usable.
8. Disconnect PipeWire and verify recovery.
9. Repeat the flow on X11.
10. Export annotations only when desktop capture is unavailable.

---

## 15. Observability and Diagnostics

Use structured logs with a per-capture correlation identifier.

Recommended fields:

- Capture ID.
- Backend.
- Desktop/session type.
- Portal implementation when safely detectable.
- Requested mode and target.
- State transition.
- Stream dimensions and sanitized format name.
- Frame sequence and age.
- Time spent in authorization, synchronization, composition, and encoding.
- Final error category.

Do not log raw pixels, window titles, or private portal metadata by default.

Add an optional diagnostics command that reports capabilities without starting a capture:

```text
Session: Wayland
Portal ScreenCast: available
PipeWire remote: available
Cursor modes: hidden, embedded, metadata
Multiple sources: supported
Restore token: supported by portal response
Fallback screenshot portal: available
```

---

## 16. Delivery Phases

### [x] Phase 1: Vector Graphics Engine

- [x] Vector data structures.
- [x] Catmull-Rom stroke smoothing.
- [x] `tiny-skia` alpha-transparent rendering.
- [x] Undo and Redo command stack.

### [x] Phase 2: X11 Overlay and Transparency

- [x] Full-screen overlay windows.
- [x] 32-bit ARGB visuals.
- [x] Dynamic XShape click passthrough.
- [x] Offscreen minimal input region workaround.
- [x] Dirty-rectangle blitting.

### [x] Phase 3: Native Wayland and XWayland Windows

- [x] Layer Shell integration for supported compositors.
- [x] XDG Shell integration.
- [x] XWayland fallback for overlay-window compatibility.

### [x] Phase 4: Advanced Annotation Tools

- [x] Laser pointer.
- [x] Spotlight mode.
- [x] Geometric shapes.
- [x] Blackboard and whiteboard modes.
- [x] Text tool.

### [x] Phase 5: Multi-Monitor and Desktop Integration

- [x] Primary-monitor geometry and toolbar centering.
- [x] HiDPI configuration inputs.
- [x] Global X11 shortcut.
- [x] Primary and all-monitor modes.
- [x] System tray menu.
- [x] Compact toolbar.
- [x] Color popup.
- [x] Smooth toolbar dragging.

### [x] Phase 6: Packaging and CI/CD

- [x] Desktop entry and icon.
- [x] DEB packaging.
- [x] RPM packaging.
- [x] AppImage.
- [x] Arch Linux PKGBUILD.
- [x] CI release workflow.

### [x] Phase 7: Snapshot Domain and Annotation Export

- [x] Introduce `snapshot` module and backend-neutral data types.
- [x] Freeze immutable document revisions for export.
- [x] Implement transparent annotation-only PNG export.
- [x] Implement atomic file output.
- [x] Add capture capability reporting.
- [x] Add composition and pixel-normalization tests.

**Exit criteria:** Annotation-only export works independently of the active window backend, and snapshot APIs contain no Wayland- or X11-specific public types.

### [x] Phase 8: Wayland Portal Prototype

- [x] Implement portal D-Bus request infrastructure.
- [x] Implement `CreateSession`, `SelectSources`, and `Start`.
- [x] Implement `OpenPipeWireRemote` FD handling.
- [x] Handle cancellation, denial, response errors, and timeouts.
- [x] Capture portal and PipeWire capabilities in diagnostics.

**Exit criteria:** An authorized monitor stream can be created and closed reliably on GNOME, KDE, and at least one wlroots compositor.

### [x] Phase 9: PipeWire Shared-Memory Capture

- [x] Negotiate initial raw pixel formats.
- [x] Acquire MemFd/shared-memory frames.
- [x] Validate offset, stride, dimensions, and buffer size.
- [x] Normalize frames to internal RGBA.
- [x] Support stream format changes.
- [x] Support fresh-frame selection by sequence and timestamp.
- [x] Add bounded transfer from callback to worker.

**Exit criteria:** Vectrace can acquire and save a desktop-only PNG from PipeWire without overlay composition.

### [x] Phase 10: Clean Wayland Snapshot

- [x] Freeze the vector document revision.
- [x] Hide overlay and toolbar surfaces.
- [x] Synchronize with a Wayland frame callback.
- [x] Reject stale PipeWire frames.
- [x] Restore surfaces with an unconditional cleanup guard.
- [x] Compose annotations offscreen.
- [x] Add user-visible progress and failure recovery.

**Exit criteria:** Clean snapshots contain desktop applications and exactly one copy of the annotations, with no toolbar, on supported Wayland desktops.

### [x] Phase 11: Multi-Monitor and HiDPI Wayland Capture

- [x] Map portal streams to Vectrace outputs.
- [x] Support multiple streams.
- [x] Implement fractional scaling transforms.
- [x] Implement rotated-output transforms.
- [x] Support negative logical origins.
- [x] Export per-monitor images.
- [x] Implement documented global stitching policies.

**Exit criteria:** Golden and manual tests pass for mixed-scale and rotated multi-monitor layouts.

### [x] Phase 12: X11 Root Capture

- [x] Implement root drawable capture through `x11rb`.
- [x] Decode visuals, masks, byte order, depth, and scanline padding.
- [x] Integrate RandR monitor geometry.
- [x] Implement clean overlay hide/sync/capture/restore sequence.
- [x] Add visible-composition mode.
- [x] Add XFixes cursor capture when requested.
- [x] Evaluate MIT-SHM after correctness is established.

**Exit criteria:** X11 clean and visible snapshots share the same output semantics and composition pipeline as Wayland.

### [x] Phase 13: Fallbacks and Session Persistence

- [x] Add portal Screenshot fallback for one-shot Wayland capture.
- [x] Add restore-token handling.
- [x] Add permission reset action.
- [x] Implement backend fallback policy.
- [x] Ensure XWayland sessions prefer portal capture when the desktop is Wayland.
- [x] Add actionable unsupported-backend errors.

**Exit criteria:** Failure of the preferred backend produces either a safe fallback or a precise explanation, never a transparent or partial image presented as success.

### [x] Phase 14: DMA-BUF and Performance Optimization

- [x] Add optional DMA-BUF import.
- [x] Handle DRM formats and modifiers.
- [x] Add shared-memory fallback.
- [x] Handle frame copies safely.
- [x] Measure overlay hidden time.
- [x] Benchmark 1080p, 1440p, 4K, and multi-monitor exports.

**Exit criteria:** Optimizations do not change image correctness, security behavior, or backend-neutral APIs.

### [x] Phase 15: Production Hardening

- [x] Complete desktop, GPU, distro, and packaging matrices.
- [x] Add portal and PipeWire fault-injection tests.
- [x] Add memory-allocation limits and fuzz buffer validation.
- [x] Verify overlay restoration on every failure path.
- [x] Complete localization and accessibility.
- [x] Publish troubleshooting documentation.
- [x] Add telemetry-free diagnostics export.

**Exit criteria:** Release candidate passes automated and manual acceptance tests across the supported Wayland and X11 environments.

---

## 17. Definition of Done

The capture and export feature is complete when:

- A Wayland user can authorize a monitor through the XDG ScreenCast portal.
- PipeWire frames are acquired without assuming a fixed format or stride.
- A clean snapshot contains the full desktop beneath Vectrace.
- Annotations are rendered exactly once.
- Toolbar and transient UI inclusion follow the selected policy.
- The overlay is restored after every success, cancellation, timeout, and error.
- X11 uses native root-window capture with correct visual conversion.
- XWayland does not incorrectly substitute X11 capture for compositor-authorized Wayland capture.
- Multi-monitor, HiDPI, fractional scaling, and rotation are handled through centralized transforms.
- Annotation-only export remains available without screen-capture permission.
- All output is written atomically.
- Unsupported environments receive an actionable explanation.
- The backend-neutral snapshot API remains reusable for future recording or remote-presentation features.

---

## 18. Key Technical Risks and Mitigations

### Portal differences between desktop environments

**Risk:** GNOME, KDE, and wlroots portal backends may expose different UX and optional capabilities.  
**Mitigation:** Build to the standard portal flow, probe optional features, maintain compositor integration tests, and treat persistence as optional.

### Stale frames after hiding the overlay

**Risk:** The first PipeWire frame may still contain the old overlay.  
**Mitigation:** Establish a compositor synchronization boundary and reject frames older than the accepted sequence or timestamp.

### Mixed-DPI coordinate mismatch

**Risk:** Annotations shift or scale incorrectly.  
**Mitigation:** Centralize coordinate transforms, store explicit coordinate spaces, and use golden images for fractional and rotated layouts.

### X11 visual assumptions

**Risk:** Incorrect colors, channel order, or row lengths.  
**Mitigation:** Read server format metadata, honor masks and padding, and normalize through tested conversion functions.

### DMA-BUF complexity

**Risk:** Driver-specific failures and unsupported modifiers.  
**Mitigation:** Ship shared-memory capture first and keep DMA-BUF optional with a reliable fallback.

### Overlay not restored after failure

**Risk:** Vectrace appears to disappear or remains unusable.  
**Mitigation:** Use RAII cleanup guards, watchdog timeouts, and fault-injection tests at every state transition.

### Excessive memory allocation

**Risk:** Malformed metadata or very large desktops cause memory exhaustion.  
**Mitigation:** Checked arithmetic, maximum dimensions, maximum total pixels, bounded queues, and validated buffer layouts.

---

## 19. Recommended First Implementation Slice

The first vertical slice should be deliberately small but production-shaped:

1. Add backend-neutral snapshot types.
2. Export annotations-only PNG.
3. Start one Wayland ScreenCast portal session.
4. Receive one shared-memory PipeWire monitor frame.
5. Normalize BGRA or BGRX to RGBA.
6. Save a desktop-only PNG.
7. Hide the overlay using a cleanup guard.
8. reject stale frames.
9. Render one frozen annotation revision over the frame.
10. Save a clean PNG atomically.
11. Implement the equivalent X11 root-window flow behind the same trait.

Do not begin with DMA-BUF, continuous recording, or compositor-specific private APIs. Establish correctness, synchronization, and cross-backend semantics first.

---

## 20. Technical References

- XDG Desktop Portal ScreenCast interface: <https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html>
- XDG Desktop Portal Screenshot interface: <https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Screenshot.html>
- PipeWire documentation: <https://docs.pipewire.org/>
- X11 protocol reference: <https://www.x.org/releases/current/doc/xproto/x11protocol.html>
- RandR protocol: <https://www.x.org/releases/current/doc/randrproto/randrproto.txt>
- `x11rb` project: <https://github.com/psychon/x11rb>

---

## 21. Final Architecture Summary

```text
                      Vectrace UI
                          │
                          ▼
                   SnapshotService
                    │            │
                    │            └── Frozen vector document
                    ▼
          ScreenCaptureBackend
             │              │
             ▼              ▼
 Wayland Portal/PipeWire   X11 root drawable
             │              │
             └──────┬───────┘
                    ▼
          Frame normalization
                    ▼
       Coordinate transformation
                    ▼
        tiny-skia offscreen render
                    ▼
           Alpha composition
                    ▼
          Atomic PNG encoding
```

This architecture makes the compositor responsible for desktop pixels, Vectrace responsible for annotation pixels, and the snapshot service responsible for creating a correct, reproducible final image.
