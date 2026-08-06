use vectrace::platform::wayland::capture::portal::PortalClient;
use vectrace::snapshot::request::{CaptureRequest, CaptureTarget, SnapshotMode};

#[test]
#[ignore]
fn test_live_portal_call() {
    let mut client = PortalClient::new();
    println!("Is portal client available: {}", client.is_available());

    let request = CaptureRequest {
        target: CaptureTarget::PrimaryMonitor,
        mode: SnapshotMode::CleanComposite,
        ..Default::default()
    };

    let res = client.start_screencast_session(&request, None);
    println!("Live Portal Call Result: {:?}", res);
}
