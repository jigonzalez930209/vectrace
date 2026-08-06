use crate::snapshot::frame::OutputTransform;
use crate::snapshot::request::OutputId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPoint {
    pub x: f32,
    pub y: f32,
}

impl LogicalPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalSize {
    pub width: f32,
    pub height: f32,
}

impl LogicalSize {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl PixelSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFactor(pub f32);

#[derive(Debug, Clone)]
pub struct OutputLayout {
    pub id: OutputId,
    pub logical_origin: LogicalPoint,
    pub logical_size: LogicalSize,
    pub stream_size: PixelSize,
    pub scale: ScaleFactor,
    pub transform: OutputTransform,
}

impl OutputLayout {
    pub fn logical_to_stream(&self, pt: LogicalPoint) -> (f32, f32) {
        let rel_x = pt.x - self.logical_origin.x;
        let rel_y = pt.y - self.logical_origin.y;

        let scaled_x = rel_x * self.scale.0;
        let scaled_y = rel_y * self.scale.0;

        match self.transform {
            OutputTransform::Normal => (scaled_x, scaled_y),
            OutputTransform::Rotate90 => (self.stream_size.width as f32 - scaled_y, scaled_x),
            OutputTransform::Rotate180 => (
                self.stream_size.width as f32 - scaled_x,
                self.stream_size.height as f32 - scaled_y,
            ),
            OutputTransform::Rotate270 => (scaled_y, self.stream_size.height as f32 - scaled_x),
            _ => (scaled_x, scaled_y),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DesktopLayoutGrid {
    pub layouts: Vec<OutputLayout>,
}

impl DesktopLayoutGrid {
    pub fn new(layouts: Vec<OutputLayout>) -> Self {
        Self { layouts }
    }

    pub fn compute_bounding_box(&self) -> (f32, f32, f32, f32) {
        if self.layouts.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for layout in &self.layouts {
            min_x = min_x.min(layout.logical_origin.x);
            min_y = min_y.min(layout.logical_origin.y);
            max_x = max_x.max(layout.logical_origin.x + layout.logical_size.width);
            max_y = max_y.max(layout.logical_origin.y + layout.logical_size.height);
        }

        (min_x, min_y, max_x, max_y)
    }

    pub fn total_logical_size(&self) -> LogicalSize {
        let (min_x, min_y, max_x, max_y) = self.compute_bounding_box();
        LogicalSize::new((max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
    }

    pub fn stitch_outputs(
        &self,
        frames: &[crate::snapshot::frame::CapturedFrame],
    ) -> Result<tiny_skia::Pixmap, crate::snapshot::error::CaptureError> {
        let (min_x, min_y, max_x, max_y) = self.compute_bounding_box();
        let total_width = (max_x - min_x).ceil() as u32;
        let total_height = (max_y - min_y).ceil() as u32;

        if total_width == 0 || total_height == 0 {
            return Err(crate::snapshot::error::CaptureError::new(
                crate::snapshot::error::CaptureErrorKind::SourceMappingFailed,
                "Multi-monitor layout has empty dimensions",
            ));
        }

        let mut canvas = tiny_skia::Pixmap::new(total_width, total_height).ok_or_else(|| {
            crate::snapshot::error::CaptureError::new(
                crate::snapshot::error::CaptureErrorKind::Internal,
                format!("Failed to allocate stitched pixmap {}x{}", total_width, total_height),
            )
        })?;

        for frame in frames {
            let layout = self
                .layouts
                .iter()
                .find(|l| l.id == frame.output)
                .ok_or_else(|| {
                    crate::snapshot::error::CaptureError::new(
                        crate::snapshot::error::CaptureErrorKind::SourceMappingFailed,
                        format!("No OutputLayout found matching frame output {:?}", frame.output),
                    )
                })?;

            let norm_rgba = crate::snapshot::composition::CompositionEngine::normalize_frame(frame)?;
            let dest_x = (layout.logical_origin.x - min_x).round() as u32;
            let dest_y = (layout.logical_origin.y - min_y).round() as u32;

            let frame_w = frame.width as usize;
            let frame_h = frame.height as usize;
            let canvas_w = total_width as usize;
            let canvas_data = canvas.data_mut();

            for y in 0..frame_h {
                let target_y = dest_y as usize + y;
                if target_y >= total_height as usize {
                    break;
                }

                for x in 0..frame_w {
                    let target_x = dest_x as usize + x;
                    if target_x >= canvas_w {
                        break;
                    }

                    let src_idx = (y * frame_w + x) * 4;
                    let dst_idx = (target_y * canvas_w + target_x) * 4;

                    canvas_data[dst_idx..dst_idx + 4].copy_from_slice(&norm_rgba[src_idx..src_idx + 4]);
                }
            }
        }

        Ok(canvas)
    }
}
