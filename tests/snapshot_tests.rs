use std::fs;
use vectrace::core::canvas::{BlendMode, Color, Point, Stroke};
use vectrace::core::Canvas;
use vectrace::snapshot::{
    CapturePixelFormat, CaptureRequest, CaptureTarget, CapturedFrame, CompositionEngine,
    FrameMemory, ImageEncoder, LogicalPoint, LogicalSize, OutputId, OutputLayout, OutputTransform,
    PixelSize, ScaleFactor, SnapshotMode, SnapshotService,
};

#[test]
fn test_pixel_format_normalization() {
    let width = 2;
    let height = 2;
    let stride = 8; // 2 pixels * 4 bytes/pixel = 8 bytes stride

    // BGRA data for 2x2: (Blue, Green, Red, Alpha)
    // Pixel (0,0): Red (255, 0, 0, 255) -> BGRA = (0, 0, 255, 255)
    // Pixel (1,0): Green (0, 255, 0, 255) -> BGRA = (0, 255, 0, 255)
    // Pixel (0,1): Blue (0, 0, 255, 255) -> BGRA = (255, 0, 0, 255)
    // Pixel (1,1): White (255, 255, 255, 255) -> BGRA = (255, 255, 255, 255)
    let bgra_bytes = vec![
        0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255,
    ];

    let frame = CapturedFrame {
        output: OutputId(1),
        width,
        height,
        stride,
        format: CapturePixelFormat::Bgra8888,
        memory: FrameMemory::Owned(bgra_bytes),
        transform: OutputTransform::Normal,
        sequence: 10,
        timestamp: std::time::Duration::from_secs(1),
        damage: vec![],
    };

    let normalized = CompositionEngine::normalize_frame(&frame).unwrap();

    assert_eq!(normalized.len(), 16);
    // Pixel (0,0) RGBA
    assert_eq!(&normalized[0..4], &[255, 0, 0, 255]);
    // Pixel (1,0) RGBA
    assert_eq!(&normalized[4..8], &[0, 255, 0, 255]);
    // Pixel (0,1) RGBA
    assert_eq!(&normalized[8..12], &[0, 0, 255, 255]);
    // Pixel (1,1) RGBA
    assert_eq!(&normalized[12..16], &[255, 255, 255, 255]);
}

#[test]
fn test_document_snapshot_freezing() {
    let mut canvas = Canvas::new(800, 600);
    let mut stroke = Stroke::new(Color::new(255, 0, 0, 255), 5.0, BlendMode::Normal);
    stroke.add_point(Point::new(10.0, 10.0, 1.0, 100));
    canvas.start_stroke(stroke);
    canvas.finish_current_stroke();

    assert_eq!(canvas.strokes().len(), 1);

    // Freeze snapshot
    let snapshot = canvas.snapshot();
    assert_eq!(snapshot.strokes.len(), 1);
    assert_eq!(snapshot.revision, canvas.revision());

    // Mutate original canvas
    canvas.clear();
    assert_eq!(canvas.strokes().len(), 0);
    assert_ne!(snapshot.revision, canvas.revision());

    // Snapshot remains untouched
    assert_eq!(snapshot.strokes.len(), 1);
}

#[test]
fn test_atomic_file_writer() {
    let temp_dir = std::env::temp_dir().join("vectrace_test_atomic");
    let target_file = temp_dir.join("test_export.png");

    let pixmap = tiny_skia::Pixmap::new(100, 100).unwrap();
    ImageEncoder::save_atomically(&pixmap, &target_file).unwrap();

    assert!(target_file.exists());
    let metadata = fs::metadata(&target_file).unwrap();
    assert!(metadata.len() > 0);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_coordinate_transforms() {
    let layout = OutputLayout {
        id: OutputId(1),
        logical_origin: LogicalPoint::new(100.0, 200.0),
        logical_size: LogicalSize::new(1920.0, 1080.0),
        stream_size: PixelSize::new(1920, 1080),
        scale: ScaleFactor(1.0),
        transform: OutputTransform::Normal,
    };

    let pt = LogicalPoint::new(150.0, 250.0);
    let (sx, sy) = layout.logical_to_stream(pt);
    assert_eq!(sx, 50.0);
    assert_eq!(sy, 50.0);
}

#[test]
fn test_snapshot_service_annotation_only() {
    let temp_dir = std::env::temp_dir().join("vectrace_test_service");
    let mut service = SnapshotService::with_default_backend();

    let mut canvas = Canvas::new(400, 300);
    let mut stroke = Stroke::new(Color::new(0, 255, 0, 255), 4.0, BlendMode::Normal);
    stroke.add_point(Point::new(50.0, 50.0, 1.0, 100));
    stroke.add_point(Point::new(100.0, 100.0, 1.0, 105));
    canvas.start_stroke(stroke);
    canvas.finish_current_stroke();

    let doc = canvas.snapshot();
    let request = CaptureRequest {
        mode: SnapshotMode::AnnotationsOnly,
        target: CaptureTarget::PrimaryMonitor,
        ..Default::default()
    };

    let (export_path, metadata) = service
        .export_snapshot(&doc, request, &temp_dir)
        .unwrap();

    assert!(export_path.exists());
    assert_eq!(metadata.width, 400);
    assert_eq!(metadata.height, 300);
    assert_eq!(metadata.stroke_count, 1);

    let _ = fs::remove_dir_all(temp_dir);
}
