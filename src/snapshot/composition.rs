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
        let row_bytes = width * 4;

        // PERFORMANCE: Match format once outside the pixel loop to avoid branch mispredictions.
        // For contiguous (stride == row_bytes) Rgba/Rgbx, use a single memcpy pass.
        match frame.format {
            CapturePixelFormat::Rgba8888 if stride == row_bytes => {
                output.copy_from_slice(&raw_bytes[..width * height * 4]);
            }
            CapturePixelFormat::Rgbx8888 if stride == row_bytes => {
                output.copy_from_slice(&raw_bytes[..width * height * 4]);
                for chunk in output.chunks_exact_mut(4) { chunk[3] = 255; }
            }
            CapturePixelFormat::Rgba8888 | CapturePixelFormat::Rgbx8888 => {
                let force_opaque = frame.format == CapturePixelFormat::Rgbx8888;
                for y in 0..height {
                    let src = &raw_bytes[y * stride..y * stride + row_bytes];
                    let dst = &mut output[y * row_bytes..(y + 1) * row_bytes];
                    dst.copy_from_slice(src);
                    if force_opaque {
                        for chunk in dst.chunks_exact_mut(4) { chunk[3] = 255; }
                    }
                }
            }
            CapturePixelFormat::Bgra8888 | CapturePixelFormat::Bgrx8888 => {
                let force_opaque = frame.format == CapturePixelFormat::Bgrx8888;
                for y in 0..height {
                    let src_row = &raw_bytes[y * stride..y * stride + row_bytes];
                    let dst_row = &mut output[y * row_bytes..(y + 1) * row_bytes];
                    // Swap R<->B channels using chunks_exact for auto-vectorization
                    for (src_px, dst_px) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                        dst_px[0] = src_px[2]; // R <- B
                        dst_px[1] = src_px[1]; // G
                        dst_px[2] = src_px[0]; // B <- R
                        dst_px[3] = if force_opaque { 255 } else { src_px[3] };
                    }
                }
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
