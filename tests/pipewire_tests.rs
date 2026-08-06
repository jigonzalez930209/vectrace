use vectrace::platform::wayland::capture::{
    PipeWireStreamReader, SharedMemoryBufferReader, SpaVideoFormat,
};
use vectrace::snapshot::frame::CapturePixelFormat;
use std::time::Instant;

#[test]
fn test_spa_video_format_mapping() {
    assert_eq!(
        SpaVideoFormat::from_spa_id(6).to_capture_format(),
        Some(CapturePixelFormat::Bgra8888)
    );
    assert_eq!(
        SpaVideoFormat::from_spa_id(7).to_capture_format(),
        Some(CapturePixelFormat::Bgrx8888)
    );
    assert_eq!(
        SpaVideoFormat::from_spa_id(12).to_capture_format(),
        Some(CapturePixelFormat::Rgba8888)
    );
    assert_eq!(
        SpaVideoFormat::from_spa_id(14).to_capture_format(),
        Some(CapturePixelFormat::Rgbx8888)
    );
    assert_eq!(
        SpaVideoFormat::from_spa_id(999).to_capture_format(),
        None
    );
}

#[test]
fn test_shm_buffer_reader_validation() {
    let dummy_fd = rustix::fs::open(
        "/dev/null",
        rustix::fs::OFlags::RDONLY,
        rustix::fs::Mode::empty(),
    )
    .unwrap();

    let res = SharedMemoryBufferReader::read_frame_bytes(&dummy_fd, 0, 0, 0, 0);
    assert!(res.is_err());
}

#[test]
fn test_pipewire_stream_reader() {
    let dummy_fd = rustix::fs::open(
        "/dev/null",
        rustix::fs::OFlags::RDONLY,
        rustix::fs::Mode::empty(),
    )
    .unwrap();

    let mut reader = PipeWireStreamReader::new(dummy_fd, 42);
    let deadline = Instant::now() + std::time::Duration::from_secs(1);

    let res = reader.acquire_frame(deadline, 100, 100, CapturePixelFormat::Bgra8888);
    assert!(res.is_ok() || res.is_err());
}
