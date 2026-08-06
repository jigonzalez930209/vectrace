use vectrace::platform::x11::capture::{X11CaptureBackend, X11VisualInfo};
use vectrace::snapshot::backend::ScreenCaptureBackend;
use vectrace::snapshot::frame::CapturePixelFormat;

#[test]
fn test_x11_visual_pixel_normalization() {
    let info_bgra = X11VisualInfo {
        depth: 24,
        bpp: 32,
        red_mask: 0x00FF0000,
        green_mask: 0x0000FF00,
        blue_mask: 0x000000FF,
    };

    let raw_bgra = vec![255, 0, 0, 255, 0, 255, 0, 255];
    let (norm, format) = X11CaptureBackend::normalize_x11_pixels(&raw_bgra, 2, 1, &info_bgra).unwrap();

    assert_eq!(format, CapturePixelFormat::Bgra8888);
    assert_eq!(norm.len(), 8);
    assert_eq!(&norm[0..4], &[255, 0, 0, 255]);

    let info_24bpp = X11VisualInfo {
        depth: 24,
        bpp: 24,
        red_mask: 0x00FF0000,
        green_mask: 0x0000FF00,
        blue_mask: 0x000000FF,
    };

    let raw_24 = vec![10, 20, 30, 40, 50, 60];
    let (norm_24, format_24) = X11CaptureBackend::normalize_x11_pixels(&raw_24, 2, 1, &info_24bpp).unwrap();

    assert_eq!(format_24, CapturePixelFormat::Rgbx8888);
    assert_eq!(norm_24.len(), 8);
    assert_eq!(&norm_24[0..4], &[10, 20, 30, 255]);
}

#[test]
fn test_x11_backend_capabilities() {
    let backend = X11CaptureBackend::new();
    let caps = backend.capabilities();

    assert_eq!(caps.backend_name, "X11RootCapture");
    assert!(caps.supports_clean_composite);
    assert!(caps.supports_visible_composition);
    assert!(caps.supports_desktop_only);
    assert!(caps.supports_all_monitors);
}
