use crate::core::export::secs_to_datetime;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::request::{CaptureTarget, SnapshotMode};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tiny_skia::Pixmap;

pub struct ImageEncoder;

impl ImageEncoder {
    /// Encodes a Pixmap to PNG and writes it atomically to `dest_path`.
    pub fn save_atomically(pixmap: &Pixmap, dest_path: &Path) -> Result<(), CaptureError> {
        let png_bytes = pixmap.encode_png().map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::EncodingFailed,
                format!("PNG encoding failed: {}", e),
            )
        })?;

        let parent_dir = dest_path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent_dir)?;

        let filename = dest_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("snapshot.png");

        let temp_filename = format!(".tmp_{}_{}", std::process::id(), filename);
        let temp_path = parent_dir.join(temp_filename);

        let write_res = (|| -> Result<(), std::io::Error> {
            let mut file = File::create(&temp_path)?;
            file.write_all(&png_bytes)?;
            file.sync_all()?;
            Ok(())
        })();

        if let Err(e) = write_res {
            let _ = fs::remove_file(&temp_path);
            return Err(CaptureError::new(
                CaptureErrorKind::Io,
                format!("Failed to write snapshot to temp file: {}", e),
            ));
        }

        fs::rename(&temp_path, dest_path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            CaptureError::new(
                CaptureErrorKind::Io,
                format!("Failed to move temp snapshot to final destination: {}", e),
            )
        })?;

        Ok(())
    }

    /// Generates a non-colliding export path in `directory` using formatted timestamp and mode descriptors.
    pub fn generate_export_path(
        directory: &Path,
        target: &CaptureTarget,
        mode: &SnapshotMode,
    ) -> PathBuf {
        let now_secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let (year, month, day, hour, min, sec) = secs_to_datetime(now_secs);

        let target_str = match target {
            CaptureTarget::PrimaryMonitor => "primary",
            CaptureTarget::Monitor(id) => &format!("monitor_{}", id.0),
            CaptureTarget::AllMonitors => "all_monitors",
        };

        let mode_str = match mode {
            SnapshotMode::AnnotationsOnly => "annotations",
            SnapshotMode::CleanComposite => "clean",
            SnapshotMode::VisibleComposition => "visible",
            SnapshotMode::DesktopOnly => "desktop",
        };

        let base_name = format!(
            "Vectrace_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}_{}_{}",
            year, month, day, hour, min, sec, target_str, mode_str
        );

        let mut candidate = directory.join(format!("{}.png", base_name));
        let mut counter = 1;

        while candidate.exists() {
            candidate = directory.join(format!("{}_{}.png", base_name, counter));
            counter += 1;
        }

        candidate
    }
}
