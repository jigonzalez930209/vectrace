pub mod x11;
pub mod wayland;
pub mod tray;
pub mod detection;
pub mod fallback;

pub trait PlatformBackend {
    fn run(&mut self, canvas: &mut crate::core::Canvas) -> Result<(), Box<dyn std::error::Error>>;
}

pub fn disable_gnome_screenshot_flash() {
    let _ = std::process::Command::new("gsettings")
        .args(["set", "org.gnome.desktop.wm.preferences", "visual-bell", "false"])
        .status();
}

pub fn create_backend() -> Box<dyn PlatformBackend> {
    disable_gnome_screenshot_flash();
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



