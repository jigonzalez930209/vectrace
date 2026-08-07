//! Structured capture probe results for the local e2e harness.

use crate::platform::detection::SessionEnvironment;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePathUsed {
    MutterScreenCast,
    XdgScreenCast,
    ScreenshotFlash,
    X11Root,
}

impl CapturePathUsed {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MutterScreenCast => "mutter_screencast",
            Self::XdgScreenCast => "xdg_screencast",
            Self::ScreenshotFlash => "screenshot_flash",
            Self::X11Root => "x11_root",
        }
    }
}

impl fmt::Display for CapturePathUsed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayHint {
    LayerShell,
    XWayland,
    X11,
    Unknown,
}

impl OverlayHint {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LayerShell => "layer_shell",
            Self::XWayland => "xwayland",
            Self::X11 => "x11",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for OverlayHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct CaptureProbeResult {
    pub path: Option<CapturePathUsed>,
    pub width: u32,
    pub height: u32,
    pub session: SessionEnvironment,
    pub overlay_hint: OverlayHint,
    pub png_path: Option<PathBuf>,
    pub error: Option<String>,
}

impl CaptureProbeResult {
    pub fn ok(
        path: CapturePathUsed,
        width: u32,
        height: u32,
        session: SessionEnvironment,
        overlay_hint: OverlayHint,
    ) -> Self {
        Self {
            path: Some(path),
            width,
            height,
            session,
            overlay_hint,
            png_path: None,
            error: None,
        }
    }

    pub fn fail(
        session: SessionEnvironment,
        overlay_hint: OverlayHint,
        error: impl Into<String>,
    ) -> Self {
        Self {
            path: None,
            width: 0,
            height: 0,
            session,
            overlay_hint,
            png_path: None,
            error: Some(error.into()),
        }
    }
}

/// Infer how Vectrace would place the overlay in this session.
pub fn detect_overlay_hint(session: &SessionEnvironment) -> OverlayHint {
    if session.is_x11() && !session.is_wayland() {
        return OverlayHint::X11;
    }
    if session.is_wayland() {
        return match probe_layer_shell_available() {
            Some(true) => OverlayHint::LayerShell,
            Some(false) => OverlayHint::XWayland,
            None => {
                if session.is_xwayland() {
                    OverlayHint::XWayland
                } else {
                    OverlayHint::Unknown
                }
            }
        };
    }
    if session.display.is_some() {
        return OverlayHint::X11;
    }
    OverlayHint::Unknown
}

fn probe_layer_shell_available() -> Option<bool> {
    use wayland_client::{protocol::wl_registry, Connection, Dispatch, QueueHandle};
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

    struct State {
        layer_shell: bool,
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name,
                interface,
                version,
            } = event
            {
                if interface == "zwlr_layer_shell_v1" {
                    let _ = registry.bind::<ZwlrLayerShellV1, _, _>(name, version.min(4), qh, ());
                    state.layer_shell = true;
                }
            }
        }
    }

    impl Dispatch<ZwlrLayerShellV1, ()> for State {
        fn event(
            _: &mut Self,
            _: &ZwlrLayerShellV1,
            _: wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    let conn = Connection::connect_to_env().ok()?;
    let display = conn.display();
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let _registry = display.get_registry(&qh, ());
    let mut state = State { layer_shell: false };
    event_queue.roundtrip(&mut state).ok()?;
    Some(state.layer_shell)
}

/// Capture the desktop and report which backend path succeeded.
pub fn capture_desktop_probe() -> Result<(tiny_skia::Pixmap, CapturePathUsed), CaptureError> {
    let session = SessionEnvironment::detect();

    // Pure X11: prefer root GetImage first.
    if session.is_x11() && !session.is_wayland() {
        match capture_x11_root() {
            Ok(pm) => return Ok((pm, CapturePathUsed::X11Root)),
            Err(e) => {
                println!("X11 root capture failed ({:?}); trying portal chain...", e);
            }
        }
    }

    // Wayland / XWayland / X11-fallback: portal chain with path tagging.
    crate::platform::wayland::capture::portal::PortalClient::take_screenshot_with_path()
}

fn capture_x11_root() -> Result<tiny_skia::Pixmap, CaptureError> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

    let (conn, screen_num) = x11rb::connect(None).map_err(|e| {
        CaptureError::new(
            CaptureErrorKind::PortalUnavailable,
            format!("X11 connect failed: {}", e),
        )
    })?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let w = screen.width_in_pixels;
    let h = screen.height_in_pixels;
    if w == 0 || h == 0 {
        return Err(CaptureError::new(
            CaptureErrorKind::Internal,
            "X11 root has zero size",
        ));
    }

    let reply = conn
        .get_image(ImageFormat::Z_PIXMAP, root, 0, 0, w, h, !0)
        .map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::PortalUnavailable,
                format!("X11 GetImage request failed: {}", e),
            )
        })?
        .reply()
        .map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::PortalUnavailable,
                format!("X11 GetImage reply failed: {}", e),
            )
        })?;

    let expected_len = (w as usize) * (h as usize) * 4;
    if reply.data.len() < expected_len {
        return Err(CaptureError::new(
            CaptureErrorKind::Internal,
            format!(
                "X11 GetImage short buffer: {} < {}",
                reply.data.len(),
                expected_len
            ),
        ));
    }

    let mut pixmap = tiny_skia::Pixmap::new(w as u32, h as u32).ok_or_else(|| {
        CaptureError::new(
            CaptureErrorKind::Internal,
            format!("Pixmap alloc {}x{}", w, h),
        )
    })?;
    let rgba = pixmap.data_mut();
    for i in 0..(w as usize * h as usize) {
        let src = i * 4;
        rgba[src] = reply.data[src + 2];
        rgba[src + 1] = reply.data[src + 1];
        rgba[src + 2] = reply.data[src];
        rgba[src + 3] = 255;
    }
    println!("Captured desktop via X11 root ({}x{})!", w, h);
    Ok(pixmap)
}

/// Run a full probe: session + overlay hint + capture (+ optional PNG write).
pub fn run_capture_probe(png_out: Option<&std::path::Path>) -> CaptureProbeResult {
    let session = SessionEnvironment::detect();
    let overlay_hint = detect_overlay_hint(&session);

    match capture_desktop_probe() {
        Ok((pixmap, path)) => {
            println!("VECTRACE_CAPTURE_PATH={}", path.as_str());
            let mut result = CaptureProbeResult::ok(
                path,
                pixmap.width(),
                pixmap.height(),
                session,
                overlay_hint,
            );
            if let Some(out) = png_out {
                if let Err(e) = pixmap.save_png(out) {
                    result.error = Some(format!("Failed to write PNG {}: {}", out.display(), e));
                } else {
                    result.png_path = Some(out.to_path_buf());
                }
            }
            result
        }
        Err(e) => {
            println!("VECTRACE_CAPTURE_PATH=failed");
            CaptureProbeResult::fail(session, overlay_hint, format!("{:?}", e))
        }
    }
}
