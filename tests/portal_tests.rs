use std::fs;
use vectrace::platform::wayland::capture::{
    RestoreTokenStorage, WaylandCaptureState, WaylandPortalBackend,
};
use vectrace::snapshot::backend::ScreenCaptureBackend;

#[test]
fn test_restore_token_storage() {
    let temp_dir = std::env::temp_dir().join("vectrace_test_token");
    let token_file = temp_dir.join("token.txt");
    let storage = RestoreTokenStorage::with_path(token_file);

    assert_eq!(storage.load_token(), None);

    assert!(storage.save_token("test_token_12345"));
    assert_eq!(storage.load_token(), Some("test_token_12345".to_string()));

    storage.clear_token();
    assert_eq!(storage.load_token(), None);

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_wayland_capture_state_machine() {
    let backend = WaylandPortalBackend::new();
    assert_eq!(*backend.state(), WaylandCaptureState::Idle);

    let caps = backend.capabilities();
    assert_eq!(caps.backend_name, "WaylandPortalPipeWire");
    assert!(caps.supports_clean_composite);
    assert!(caps.supports_visible_composition);
    assert!(caps.supports_desktop_only);
}
