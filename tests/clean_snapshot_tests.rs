use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use vectrace::platform::wayland::capture::CleanSnapshotGuard;

#[test]
fn test_clean_snapshot_guard_automatic_drop() {
    let restored = Arc::new(AtomicBool::new(false));
    let restored_clone = Arc::clone(&restored);

    {
        let _guard = CleanSnapshotGuard::new(move || {
            restored_clone.store(true, Ordering::SeqCst);
        });
        assert!(!restored.load(Ordering::SeqCst));
    }

    assert!(restored.load(Ordering::SeqCst));
}

#[test]
fn test_clean_snapshot_guard_explicit_restore() {
    let call_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let count_clone = Arc::clone(&call_count);

    {
        let mut guard = CleanSnapshotGuard::new(move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        guard.restore_now();
        guard.restore_now();
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}
