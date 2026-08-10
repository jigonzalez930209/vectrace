pub mod x11;
pub mod wayland;
pub mod tray;
pub mod autostart;
pub mod detection;
pub mod fallback;
pub mod clipboard;
pub mod export_worker;

pub trait PlatformBackend {
    fn run(&mut self, canvas: &mut crate::core::Canvas) -> Result<(), Box<dyn std::error::Error>>;
}

pub fn create_backend(start_hidden: bool) -> Box<dyn PlatformBackend> {
    let (tx, rx) = std::sync::mpsc::channel();
    tray::spawn_tray(tx);

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        println!("WAYLAND_DISPLAY detected. Using native Wayland backend...");
        let mut backend = wayland::WaylandBackend::new_with_tray(rx);
        backend.is_hidden = start_hidden;
        Box::new(backend)
    } else {
        println!("Using X11 backend...");
        let mut backend = x11::X11Backend::new_with_tray(rx);
        backend.is_hidden = start_hidden;
        Box::new(backend)
    }
}



