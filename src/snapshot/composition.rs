use crate::core::document::DocumentSnapshot;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::{CapturePixelFormat, CapturedFrame, FrameMemory};
use tiny_skia::Pixmap;

pub struct CompositionEngine;

impl CompositionEngine {
    /// Normalizes incoming CapturedFrame memory into a standard RGBA8888 buffer of size (width * height * 4).
    pub fn normalize_frame(frame: &CapturedFrame) -> Result<Vec<u8>, CaptureError> {
        let owned_bytes: Vec<u8>;
        let raw_bytes = match &frame.memory {
            FrameMemory::Owned(vec) => vec.as_slice(),
            FrameMemory::DmaBuf(dmabuf) => {
                let offset = dmabuf.plane_offsets.first().copied().unwrap_or(0) as u32;
                let stride = dmabuf.plane_strides.first().copied().unwrap_or(frame.stride) as u32;
                owned_bytes = crate::platform::wayland::capture::SharedMemoryBufferReader::read_frame_bytes(
                    &dmabuf.fd,
                    offset,
                    stride,
                    frame.width,
                    frame.height,
                )?;
                owned_bytes.as_slice()
            }
        };

        if frame.width > 8192 || frame.height > 8192 {
            return Err(CaptureError::new(
                CaptureErrorKind::UnsupportedPixelFormat,
                format!(
                    "Frame dimensions {}x{} exceed max 8192x8192 allocation limit",
                    frame.width, frame.height
                ),
            ));
        }

        let width = frame.width as usize;
        let height = frame.height as usize;
        let stride = frame.stride;
        let expected_min_len = (height - 1) * stride + width * 4;

        if raw_bytes.len() < expected_min_len {
            return Err(CaptureError::new(
                CaptureErrorKind::UnsupportedPixelFormat,
                format!(
                    "Frame buffer underflow: byte len {} < expected min {}",
                    raw_bytes.len(),
                    expected_min_len
                ),
            ));
        }

        let mut output = vec![0u8; width * height * 4];

        for y in 0..height {
            let src_row_start = y * stride;
            let dst_row_start = y * width * 4;

            for x in 0..width {
                let src_idx = src_row_start + x * 4;
                let dst_idx = dst_row_start + x * 4;

                let (r, g, b, a) = match frame.format {
                    CapturePixelFormat::Rgba8888 => (
                        raw_bytes[src_idx],
                        raw_bytes[src_idx + 1],
                        raw_bytes[src_idx + 2],
                        raw_bytes[src_idx + 3],
                    ),
                    CapturePixelFormat::Rgbx8888 => (
                        raw_bytes[src_idx],
                        raw_bytes[src_idx + 1],
                        raw_bytes[src_idx + 2],
                        255,
                    ),
                    CapturePixelFormat::Bgra8888 => (
                        raw_bytes[src_idx + 2],
                        raw_bytes[src_idx + 1],
                        raw_bytes[src_idx],
                        raw_bytes[src_idx + 3],
                    ),
                    CapturePixelFormat::Bgrx8888 => (
                        raw_bytes[src_idx + 2],
                        raw_bytes[src_idx + 1],
                        raw_bytes[src_idx],
                        255,
                    ),
                };

                output[dst_idx] = r;
                output[dst_idx + 1] = g;
                output[dst_idx + 2] = b;
                output[dst_idx + 3] = a;
            }
        }

        Ok(output)
    }

    /// Renders a DocumentSnapshot offscreen to a transparent or background-filled Pixmap.
    pub fn render_annotations(
        doc: &DocumentSnapshot,
        width: u32,
        height: u32,
        include_background: bool,
    ) -> Result<Pixmap, CaptureError> {
        let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
            CaptureError::new(
                CaptureErrorKind::Internal,
                format!("Failed to allocate Pixmap of size {}x{}", width, height),
            )
        })?;

        doc.render(&mut pixmap, include_background);
        Ok(pixmap)
    }

    /// Composites annotations over a normalized RGBA desktop frame.
    pub fn composite_clean_snapshot(
        desktop_rgba: &[u8],
        width: u32,
        height: u32,
        doc: &DocumentSnapshot,
    ) -> Result<Pixmap, CaptureError> {
        if desktop_rgba.len() != (width as usize * height as usize * 4) {
            return Err(CaptureError::new(
                CaptureErrorKind::Internal,
                "Desktop frame dimensions do not match RGBA buffer length",
            ));
        }

        let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
            CaptureError::new(
                CaptureErrorKind::Internal,
                format!("Failed to allocate Pixmap {}x{}", width, height),
            )
        })?;

        pixmap.data_mut().copy_from_slice(desktop_rgba);
        doc.render(&mut pixmap, false);

        Ok(pixmap)
    }
}
