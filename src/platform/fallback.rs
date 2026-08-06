use crate::snapshot::backend::{AnnotationOnlyBackend, CaptureSessionId, ScreenCaptureBackend};
use crate::snapshot::capabilities::CaptureCapabilities;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::CapturedFrame;
use crate::snapshot::request::CaptureRequest;
use std::time::Instant;

pub struct FallbackCaptureBackend {
    primary: Box<dyn ScreenCaptureBackend>,
    fallback: Box<dyn ScreenCaptureBackend>,
    using_fallback: bool,
}

impl FallbackCaptureBackend {
    pub fn new(
        primary: Box<dyn ScreenCaptureBackend>,
        fallback: Box<dyn ScreenCaptureBackend>,
    ) -> Self {
        Self {
            primary,
            fallback,
            using_fallback: false,
        }
    }

    pub fn with_annotation_fallback(primary: Box<dyn ScreenCaptureBackend>) -> Self {
        Self::new(primary, Box::new(AnnotationOnlyBackend::new()))
    }

    pub fn is_using_fallback(&self) -> bool {
        self.using_fallback
    }
}

impl ScreenCaptureBackend for FallbackCaptureBackend {
    fn capabilities(&self) -> CaptureCapabilities {
        if self.using_fallback {
            self.fallback.capabilities()
        } else {
            self.primary.capabilities()
        }
    }

    fn start(&mut self, request: CaptureRequest) -> Result<CaptureSessionId, CaptureError> {
        self.using_fallback = false;
        match self.primary.start(request.clone()) {
            Ok(id) => Ok(id),
            Err(err) => {
                if err.kind != CaptureErrorKind::UserCancelled {
                    self.using_fallback = true;
                    if let Ok(id) = self.fallback.start(request) {
                        return Ok(id);
                    }
                }
                Err(err)
            }
        }
    }

    fn next_frame(&mut self, deadline: Instant) -> Result<CapturedFrame, CaptureError> {
        if self.using_fallback {
            self.fallback.next_frame(deadline)
        } else {
            self.primary.next_frame(deadline)
        }
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        if self.using_fallback {
            self.fallback.stop()
        } else {
            self.primary.stop()
        }
    }
}
