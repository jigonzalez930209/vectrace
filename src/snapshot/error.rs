use std::fmt;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct CaptureError {
    pub kind: CaptureErrorKind,
    pub message: String,
}

impl CaptureError {
    pub fn new(kind: CaptureErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.kind, self.message)
    }
}

impl std::error::Error for CaptureError {}

impl From<io::Error> for CaptureError {
    fn from(err: io::Error) -> Self {
        Self::new(CaptureErrorKind::Io, err.to_string())
    }
}
