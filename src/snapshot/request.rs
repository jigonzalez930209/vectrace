use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureTarget {
    PrimaryMonitor,
    Monitor(OutputId),
    AllMonitors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotMode {
    AnnotationsOnly,
    CleanComposite,
    VisibleComposition,
    DesktopOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorPolicy {
    Hidden,
    Embedded,
    Metadata,
}

#[derive(Debug, Clone)]
pub struct CaptureRequest {
    pub target: CaptureTarget,
    pub mode: SnapshotMode,
    pub cursor: CursorPolicy,
    pub include_toolbar: bool,
    pub include_transient_effects: bool,
    pub timeout: Duration,
}

impl Default for CaptureRequest {
    fn default() -> Self {
        Self {
            target: CaptureTarget::PrimaryMonitor,
            mode: SnapshotMode::CleanComposite,
            cursor: CursorPolicy::Hidden,
            include_toolbar: false,
            include_transient_effects: false,
            timeout: Duration::from_secs(5),
        }
    }
}
