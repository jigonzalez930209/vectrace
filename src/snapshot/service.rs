use crate::core::document::DocumentSnapshot;
use crate::snapshot::backend::{AnnotationOnlyBackend, ScreenCaptureBackend};
use crate::snapshot::capabilities::CaptureCapabilities;
use crate::snapshot::composition::CompositionEngine;
use crate::snapshot::encoder::ImageEncoder;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::metadata::SnapshotMetadata;
use crate::snapshot::request::{CaptureRequest, SnapshotMode};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

pub struct SnapshotService {
    backend: Box<dyn ScreenCaptureBackend>,
}

impl SnapshotService {
    pub fn new(backend: Box<dyn ScreenCaptureBackend>) -> Self {
        Self { backend }
    }

    pub fn with_default_backend() -> Self {
        Self::new(Box::new(AnnotationOnlyBackend::new()))
    }

    pub fn capabilities(&self) -> CaptureCapabilities {
        self.backend.capabilities()
    }

    pub fn export_snapshot(
        &mut self,
        document: &DocumentSnapshot,
        request: CaptureRequest,
        export_dir: &Path,
    ) -> Result<(PathBuf, SnapshotMetadata), CaptureError> {
        let export_path = ImageEncoder::generate_export_path(export_dir, &request.target, &request.mode);

        match request.mode {
            SnapshotMode::AnnotationsOnly => {
                let include_bg = document.background_mode != crate::core::canvas::BackgroundMode::Transparent;
                let pixmap = CompositionEngine::render_annotations(
                    document,
                    document.width,
                    document.height,
                    include_bg,
                )?;

                ImageEncoder::save_atomically(&pixmap, &export_path)?;

                let metadata = SnapshotMetadata {
                    timestamp: SystemTime::now(),
                    target: request.target,
                    mode: request.mode,
                    width: document.width,
                    height: document.height,
                    backend_name: self.backend.capabilities().backend_name,
                    stroke_count: document.strokes.len(),
                };

                Ok((export_path, metadata))
            }
            SnapshotMode::CleanComposite | SnapshotMode::VisibleComposition | SnapshotMode::DesktopOnly => {
                let caps = self.backend.capabilities();
                let is_supported = match request.mode {
                    SnapshotMode::CleanComposite => caps.supports_clean_composite,
                    SnapshotMode::VisibleComposition => caps.supports_visible_composition,
                    SnapshotMode::DesktopOnly => caps.supports_desktop_only,
                    _ => false,
                };

                if !is_supported {
                    return Err(CaptureError::new(
                        CaptureErrorKind::PortalUnavailable,
                        format!(
                            "Desktop capture mode {:?} is not supported by backend '{}'",
                            request.mode, caps.backend_name
                        ),
                    ));
                }

                let _session_id = self.backend.start(request.clone())?;
                let deadline = Instant::now() + request.timeout;
                let frame_res = self.backend.next_frame(deadline);
                let _ = self.backend.stop();

                let frame = frame_res?;
                let normalized_rgba = CompositionEngine::normalize_frame(&frame)?;

                let pixmap = if request.mode == SnapshotMode::DesktopOnly {
                    let mut p = tiny_skia::Pixmap::new(frame.width, frame.height).ok_or_else(|| {
                        CaptureError::new(CaptureErrorKind::Internal, "Pixmap allocation failed")
                    })?;
                    p.data_mut().copy_from_slice(&normalized_rgba);
                    p
                } else {
                    CompositionEngine::composite_clean_snapshot(
                        &normalized_rgba,
                        frame.width,
                        frame.height,
                        document,
                    )?
                };

                ImageEncoder::save_atomically(&pixmap, &export_path)?;

                let metadata = SnapshotMetadata {
                    timestamp: SystemTime::now(),
                    target: request.target,
                    mode: request.mode,
                    width: frame.width,
                    height: frame.height,
                    backend_name: caps.backend_name,
                    stroke_count: if request.mode == SnapshotMode::DesktopOnly { 0 } else { document.strokes.len() },
                };

                Ok((export_path, metadata))
            }
        }
    }
}
