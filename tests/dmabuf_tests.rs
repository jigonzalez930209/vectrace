use std::time::Instant;
use vectrace::snapshot::composition::CompositionEngine;
use vectrace::snapshot::frame::{
    CapturePixelFormat, CapturedFrame, DmaBufFrame, FrameMemory, OutputTransform,
};
use vectrace::snapshot::request::OutputId;

#[test]
fn test_dmabuf_fallback_read() {
    let dummy_fd = rustix::fs::open(
        "/dev/null",
        rustix::fs::OFlags::RDONLY,
        rustix::fs::Mode::empty(),
    )
    .unwrap();

    let dmabuf = DmaBufFrame {
        fd: dummy_fd.into(),
        drm_format: 0x34325241, // DRM_FORMAT_ARGB8888
        modifier: 0,
        plane_offsets: vec![0],
        plane_strides: vec![400],
    };

    let frame = CapturedFrame {
        output: OutputId(1),
        width: 100,
        height: 100,
        stride: 400,
        format: CapturePixelFormat::Bgra8888,
        memory: FrameMemory::DmaBuf(dmabuf),
        transform: OutputTransform::Normal,
        sequence: 1,
        timestamp: std::time::Duration::from_secs(1),
        damage: vec![],
    };

    let res = CompositionEngine::normalize_frame(&frame);
    assert!(res.is_err());
}

#[test]
fn test_composition_benchmarks() {
    let w_1080 = 1920;
    let h_1080 = 1080;
    let frame_1080 = CapturedFrame {
        output: OutputId(1),
        width: w_1080,
        height: h_1080,
        stride: (w_1080 * 4) as usize,
        format: CapturePixelFormat::Bgra8888,
        memory: FrameMemory::Owned(vec![128u8; (w_1080 * h_1080 * 4) as usize]),
        transform: OutputTransform::Normal,
        sequence: 1,
        timestamp: std::time::Duration::from_secs(1),
        damage: vec![],
    };

    let start = Instant::now();
    let norm_1080 = CompositionEngine::normalize_frame(&frame_1080).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(norm_1080.len(), (w_1080 * h_1080 * 4) as usize);
    assert!(elapsed.as_millis() < 250);

    let w_4k = 3840;
    let h_4k = 2160;
    let frame_4k = CapturedFrame {
        output: OutputId(1),
        width: w_4k,
        height: h_4k,
        stride: (w_4k * 4) as usize,
        format: CapturePixelFormat::Bgra8888,
        memory: FrameMemory::Owned(vec![128u8; (w_4k * h_4k * 4) as usize]),
        transform: OutputTransform::Normal,
        sequence: 1,
        timestamp: std::time::Duration::from_secs(1),
        damage: vec![],
    };

    let start_4k = Instant::now();
    let norm_4k = CompositionEngine::normalize_frame(&frame_4k).unwrap();
    let elapsed_4k = start_4k.elapsed();

    assert_eq!(norm_4k.len(), (w_4k * h_4k * 4) as usize);
    assert!(elapsed_4k.as_millis() < 500);
}
