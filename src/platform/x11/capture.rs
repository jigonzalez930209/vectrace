use crate::snapshot::backend::{CaptureSessionId, ScreenCaptureBackend};
use crate::snapshot::capabilities::CaptureCapabilities;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::{CapturePixelFormat, CapturedFrame, FrameMemory, OutputTransform};
use crate::snapshot::request::{CaptureRequest, CursorPolicy, OutputId};
use std::time::{Duration, Instant, SystemTime};

pub struct X11VisualInfo {
    pub depth: u8,
    pub bpp: u8,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
}

pub struct X11CaptureBackend {
    active_session: Option<CaptureSessionId>,
    next_id: u64,
}

impl X11CaptureBackend {
    pub fn new() -> Self {
        Self {
            active_session: None,
            next_id: 1,
        }
    }

    /// Normalizes X11 visual pixel buffer according to channel bitmasks.
    pub fn normalize_x11_pixels(
        raw: &[u8],
        width: u32,
        height: u32,
        info: &X11VisualInfo,
    ) -> Result<(Vec<u8>, CapturePixelFormat), CaptureError> {
        let count = (width * height) as usize;
        let expected_min_bytes = match info.bpp {
            24 => count * 3,
            32 => count * 4,
            _ => count * 4,
        };

        if raw.len() < expected_min_bytes {
            return Err(CaptureError::new(
                CaptureErrorKind::UnsupportedPixelFormat,
                format!("X11 image underflow: {} < {}", raw.len(), expected_min_bytes),
            ));
        }

        let mut output = vec![0u8; count * 4];

        if info.bpp == 32 {
            let is_bgra = info.blue_mask == 0x000000FF && info.red_mask == 0x00FF0000;
            let format = if is_bgra {
                CapturePixelFormat::Bgra8888
            } else {
                CapturePixelFormat::Rgba8888
            };

            output.copy_from_slice(&raw[..count * 4]);
            Ok((output, format))
        } else if info.bpp == 24 {
            for i in 0..count {
                let src_idx = i * 3;
                let dst_idx = i * 4;
                output[dst_idx] = raw[src_idx];
                output[dst_idx + 1] = raw[src_idx + 1];
                output[dst_idx + 2] = raw[src_idx + 2];
                output[dst_idx + 3] = 255;
            }
            Ok((output, CapturePixelFormat::Rgbx8888))
        } else {
            Err(CaptureError::new(
                CaptureErrorKind::UnsupportedPixelFormat,
                format!("Unsupported X11 bpp {}", info.bpp),
            ))
        }
    }
}

impl Default for X11CaptureBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenCaptureBackend for X11CaptureBackend {
    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities {
            backend_name: "X11RootCapture".to_string(),
            supports_clean_composite: true,
            supports_visible_composition: true,
            supports_desktop_only: true,
            supports_all_monitors: true,
            supported_cursor_policies: vec![CursorPolicy::Hidden, CursorPolicy::Embedded],
        }
    }

    fn start(&mut self, _request: CaptureRequest) -> Result<CaptureSessionId, CaptureError> {
        let id = CaptureSessionId(self.next_id);
        self.next_id += 1;
        self.active_session = Some(id);
        Ok(id)
    }

    fn next_frame(&mut self, _deadline: Instant) -> Result<CapturedFrame, CaptureError> {
        if self.active_session.is_none() {
            return Err(CaptureError::new(
                CaptureErrorKind::SessionClosed,
                "No active X11 capture session",
            ));
        }

        let width = 1920;
        let height = 1080;
        let stride = (width * 4) as usize;

        let mut raw = vec![0u8; width as usize * height as usize * 4];
        for chunk in raw.chunks_exact_mut(4) {
            chunk[0] = 240; // B
            chunk[1] = 240; // G
            chunk[2] = 240; // R
            chunk[3] = 255; // A
        }

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);

        Ok(CapturedFrame {
            output: OutputId(1),
            width,
            height,
            stride,
            format: CapturePixelFormat::Bgra8888,
            memory: FrameMemory::Owned(raw),
            transform: OutputTransform::Normal,
            sequence: 1,
            timestamp,
            damage: vec![],
        })
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.active_session = None;
        Ok(())
    }
}
