use vectrace::platform::detection::{CaptureBackendKind, SessionEnvironment};
use vectrace::platform::fallback::FallbackCaptureBackend;
use vectrace::platform::x11::capture::X11CaptureBackend;
use vectrace::snapshot::backend::{CaptureSessionId, ScreenCaptureBackend};
use vectrace::snapshot::capabilities::CaptureCapabilities;
use vectrace::snapshot::error::{CaptureError, CaptureErrorKind};
use vectrace::snapshot::frame::CapturedFrame;
use vectrace::snapshot::request::{CaptureRequest, SnapshotMode};
use std::time::Instant;

struct FailingBackend;

impl ScreenCaptureBackend for FailingBackend {
    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities {
            backend_name: "FailingBackend".to_string(),
            ..Default::default()
        }
    }

    fn start(&mut self, _request: CaptureRequest) -> Result<CaptureSessionId, CaptureError> {
        Err(CaptureError::new(
            CaptureErrorKind::PortalUnavailable,
            "Simulated portal failure",
        ))
    }

    fn next_frame(&mut self, _deadline: Instant) -> Result<CapturedFrame, CaptureError> {
        Err(CaptureError::new(
            CaptureErrorKind::PortalUnavailable,
            "Simulated failure",
        ))
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        Ok(())
    }
}

#[test]
fn test_session_detection_hints() {
    let env_wayland = SessionEnvironment {
        wayland_display: Some("wayland-0".to_string()),
        display: None,
        session_type: Some("wayland".to_string()),
    };

    assert!(env_wayland.is_wayland());
    assert!(!env_wayland.is_x11());
    assert_eq!(env_wayland.preferred_backend_kind(), CaptureBackendKind::WaylandPortal);

    let env_xwayland = SessionEnvironment {
        wayland_display: Some("wayland-0".to_string()),
        display: Some(":0".to_string()),
        session_type: Some("wayland".to_string()),
    };

    assert!(env_xwayland.is_xwayland());
    assert_eq!(env_xwayland.preferred_backend_kind(), CaptureBackendKind::WaylandPortal);

    let env_x11 = SessionEnvironment {
        wayland_display: None,
        display: Some(":0".to_string()),
        session_type: Some("x11".to_string()),
    };

    assert!(env_x11.is_x11());
    assert_eq!(env_x11.preferred_backend_kind(), CaptureBackendKind::X11Root);
}

#[test]
fn test_fallback_backend_chain() {
    let primary = Box::new(FailingBackend);
    let secondary = Box::new(X11CaptureBackend::new());

    let mut chained = FallbackCaptureBackend::new(primary, secondary);
    let request = CaptureRequest {
        mode: SnapshotMode::CleanComposite,
        ..Default::default()
    };

    let start_res = chained.start(request);
    assert!(start_res.is_ok());

    assert!(chained.is_using_fallback());
    assert_eq!(chained.capabilities().backend_name, "X11RootCapture");
}
