use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct CleanSnapshotGuard {
    is_restored: Arc<AtomicBool>,
    restore_fn: Option<Box<dyn FnOnce() + Send>>,
}

impl CleanSnapshotGuard {
    pub fn new<F>(restore: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self {
            is_restored: Arc::new(AtomicBool::new(false)),
            restore_fn: Some(Box::new(restore)),
        }
    }

    pub fn restore_now(&mut self) {
        if !self.is_restored.swap(true, Ordering::SeqCst) {
            if let Some(f) = self.restore_fn.take() {
                f();
            }
        }
    }

    pub fn is_restored(&self) -> bool {
        self.is_restored.load(Ordering::SeqCst)
    }
}

impl Drop for CleanSnapshotGuard {
    fn drop(&mut self) {
        self.restore_now();
    }
}
