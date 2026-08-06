use crate::snapshot::capabilities::CaptureCapabilities;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::CapturedFrame;
use crate::snapshot::request::CaptureRequest;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureSessionId(pub u64);

pub trait ScreenCaptureBackend: Send {
    fn capabilities(&self) -> CaptureCapabilities;

    fn start(&mut self, request: CaptureRequest) -> Result<CaptureSessionId, CaptureError>;

    fn next_frame(&mut self, deadline: Instant) -> Result<CapturedFrame, CaptureError>;

    fn stop(&mut self) -> Result<(), CaptureError>;
}

/// Fallback backend that only supports annotation-only export.
pub struct AnnotationOnlyBackend {
    active_session: Option<CaptureSessionId>,
    next_id: u64,
}

impl AnnotationOnlyBackend {
    pub fn new() -> Self {
        Self {
            active_session: None,
            next_id: 1,
        }
    }
}

impl Default for AnnotationOnlyBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenCaptureBackend for AnnotationOnlyBackend {
    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities::default()
    }

    fn start(&mut self, _request: CaptureRequest) -> Result<CaptureSessionId, CaptureError> {
        let id = CaptureSessionId(self.next_id);
        self.next_id += 1;
        self.active_session = Some(id);
        Ok(id)
    }

    fn next_frame(&mut self, _deadline: Instant) -> Result<CapturedFrame, CaptureError> {
        Err(CaptureError::new(
            CaptureErrorKind::PortalUnavailable,
            "AnnotationOnlyBackend does not capture desktop frames",
        ))
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.active_session = None;
        Ok(())
    }
}
