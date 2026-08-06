use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    WaylandPortal,
    X11Root,
    AnnotationOnly,
}

#[derive(Debug, Clone)]
pub struct SessionEnvironment {
    pub wayland_display: Option<String>,
    pub display: Option<String>,
    pub session_type: Option<String>,
}

impl SessionEnvironment {
    pub fn detect() -> Self {
        Self {
            wayland_display: env::var("WAYLAND_DISPLAY").ok(),
            display: env::var("DISPLAY").ok(),
            session_type: env::var("XDG_SESSION_TYPE").ok(),
        }
    }

    pub fn is_wayland(&self) -> bool {
        self.wayland_display.is_some()
            || self
                .session_type
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case("wayland"))
                .unwrap_or(false)
    }

    pub fn is_x11(&self) -> bool {
        self.display.is_some()
            && self
                .session_type
                .as_deref()
                .map(|s| s.eq_ignore_ascii_case("x11"))
                .unwrap_or(false)
    }

    pub fn is_xwayland(&self) -> bool {
        self.wayland_display.is_some() && self.display.is_some()
    }

    pub fn preferred_backend_kind(&self) -> CaptureBackendKind {
        if self.is_wayland() || self.is_xwayland() {
            CaptureBackendKind::WaylandPortal
        } else if self.is_x11() {
            CaptureBackendKind::X11Root
        } else {
            CaptureBackendKind::AnnotationOnly
        }
    }
}
