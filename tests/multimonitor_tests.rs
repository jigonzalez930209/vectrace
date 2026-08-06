use vectrace::snapshot::{
    CapturePixelFormat, CapturedFrame, DesktopLayoutGrid, FrameMemory, LogicalPoint, LogicalSize,
    OutputId, OutputLayout, OutputTransform, PixelSize, ScaleFactor,
};

#[test]
fn test_desktop_grid_bounding_box_negative_origins() {
    let layout1 = OutputLayout {
        id: OutputId(1),
        logical_origin: LogicalPoint::new(0.0, 0.0),
        logical_size: LogicalSize::new(1920.0, 1080.0),
        stream_size: PixelSize::new(1920, 1080),
        scale: ScaleFactor(1.0),
        transform: OutputTransform::Normal,
    };

    let layout2 = OutputLayout {
        id: OutputId(2),
        logical_origin: LogicalPoint::new(-1920.0, 0.0),
        logical_size: LogicalSize::new(1920.0, 1080.0),
        stream_size: PixelSize::new(1920, 1080),
        scale: ScaleFactor(1.0),
        transform: OutputTransform::Normal,
    };

    let grid = DesktopLayoutGrid::new(vec![layout1, layout2]);
    let (min_x, min_y, max_x, max_y) = grid.compute_bounding_box();

    assert_eq!(min_x, -1920.0);
    assert_eq!(min_y, 0.0);
    assert_eq!(max_x, 1920.0);
    assert_eq!(max_y, 1080.0);

    let total = grid.total_logical_size();
    assert_eq!(total.width, 3840.0);
    assert_eq!(total.height, 1080.0);
}

#[test]
fn test_multi_monitor_stitching() {
    let layout_left = OutputLayout {
        id: OutputId(1),
        logical_origin: LogicalPoint::new(-2.0, 0.0),
        logical_size: LogicalSize::new(2.0, 2.0),
        stream_size: PixelSize::new(2, 2),
        scale: ScaleFactor(1.0),
        transform: OutputTransform::Normal,
    };

    let layout_right = OutputLayout {
        id: OutputId(2),
        logical_origin: LogicalPoint::new(0.0, 0.0),
        logical_size: LogicalSize::new(2.0, 2.0),
        stream_size: PixelSize::new(2, 2),
        scale: ScaleFactor(1.0),
        transform: OutputTransform::Normal,
    };

    let frame_left = CapturedFrame {
        output: OutputId(1),
        width: 2,
        height: 2,
        stride: 8,
        format: CapturePixelFormat::Rgba8888,
        memory: FrameMemory::Owned(vec![
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ]),
        transform: OutputTransform::Normal,
        sequence: 1,
        timestamp: std::time::Duration::from_secs(1),
        damage: vec![],
    };

    let frame_right = CapturedFrame {
        output: OutputId(2),
        width: 2,
        height: 2,
        stride: 8,
        format: CapturePixelFormat::Rgba8888,
        memory: FrameMemory::Owned(vec![
            0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255,
        ]),
        transform: OutputTransform::Normal,
        sequence: 1,
        timestamp: std::time::Duration::from_secs(1),
        damage: vec![],
    };

    let grid = DesktopLayoutGrid::new(vec![layout_left, layout_right]);
    let stitched = grid.stitch_outputs(&[frame_left, frame_right]).unwrap();

    assert_eq!(stitched.width(), 4);
    assert_eq!(stitched.height(), 2);

    let data = stitched.data();
    assert_eq!(&data[0..4], &[255, 0, 0, 255]);
    assert_eq!(&data[8..12], &[0, 0, 255, 255]);
}
