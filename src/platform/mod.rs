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

pub fn create_backend() -> Box<dyn PlatformBackend> {
    let (tx, rx) = std::sync::mpsc::channel();
    tray::spawn_tray(tx);

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        println!("WAYLAND_DISPLAY detected. Using native Wayland backend...");
        Box::new(wayland::WaylandBackend::new_with_tray(rx))
    } else {
        println!("Using X11 backend...");
        Box::new(x11::X11Backend::new_with_tray(rx))
    }
}



