use vectrace::snapshot::capabilities::CaptureCapabilities;
use vectrace::snapshot::composition::CompositionEngine;
use vectrace::snapshot::diagnostics::DiagnosticsReport;
use vectrace::snapshot::frame::{
    CapturePixelFormat, CapturedFrame, FrameMemory, OutputTransform,
};
use vectrace::snapshot::request::OutputId;

#[test]
fn test_memory_allocation_limits() {
    let frame_oversized = CapturedFrame {
        output: OutputId(1),
        width: 10000,
        height: 10000,
        stride: 40000,
        format: CapturePixelFormat::Rgba8888,
        memory: FrameMemory::Owned(vec![]),
        transform: OutputTransform::Normal,
        sequence: 1,
        timestamp: std::time::Duration::from_secs(1),
        damage: vec![],
    };

    let res = CompositionEngine::normalize_frame(&frame_oversized);
    assert!(res.is_err());
}

#[test]
fn test_telemetry_free_diagnostics_report() {
    let caps = CaptureCapabilities {
        backend_name: "TestBackend".to_string(),
        supports_clean_composite: true,
        supports_visible_composition: true,
        supports_desktop_only: true,
        supports_all_monitors: true,
        supported_cursor_policies: vec![],
    };

    let report = DiagnosticsReport::generate_report(&caps);

    assert!(report.contains("Vectrace Screen Capture Diagnostics"));
    assert!(report.contains("Active Capture Backend: TestBackend"));
    assert!(report.contains("Max Allocation Bounds: 8192x8192"));
}
