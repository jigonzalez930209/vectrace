use crate::platform::detection::SessionEnvironment;
use crate::platform::wayland::capture::RestoreTokenStorage;
use crate::snapshot::capabilities::CaptureCapabilities;

pub struct DiagnosticsReport;

impl DiagnosticsReport {
    pub fn generate_report(caps: &CaptureCapabilities) -> String {
        let env = SessionEnvironment::detect();
        let restore_storage = RestoreTokenStorage::new();
        let has_token = restore_storage.load_token().is_some();

        let session_str = if env.is_xwayland() {
            "Wayland (XWayland active)"
        } else if env.is_wayland() {
            "Native Wayland"
        } else if env.is_x11() {
            "X11"
        } else {
            "Unknown"
        };

        format!(
            "--- Vectrace Screen Capture Diagnostics ---\n\
             Session Type: {}\n\
             WAYLAND_DISPLAY: {}\n\
             DISPLAY: {}\n\
             XDG_SESSION_TYPE: {}\n\
             Active Capture Backend: {}\n\
             Clean Composition Supported: {}\n\
             Visible Composition Supported: {}\n\
             Desktop Only Supported: {}\n\
             All Monitors Supported: {}\n\
             Restore Token Stored: {}\n\
             Max Allocation Bounds: 8192x8192 (256 MB)\n\
             ------------------------------------------",
            session_str,
            env.wayland_display.as_deref().unwrap_or("none"),
            env.display.as_deref().unwrap_or("none"),
            env.session_type.as_deref().unwrap_or("none"),
            caps.backend_name,
            caps.supports_clean_composite,
            caps.supports_visible_composition,
            caps.supports_desktop_only,
            caps.supports_all_monitors,
            has_token,
        )
    }
}
