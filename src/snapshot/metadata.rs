use crate::snapshot::request::{CaptureTarget, SnapshotMode};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    pub timestamp: SystemTime,
    pub target: CaptureTarget,
    pub mode: SnapshotMode,
    pub width: u32,
    pub height: u32,
    pub backend_name: String,
    pub stroke_count: usize,
}
