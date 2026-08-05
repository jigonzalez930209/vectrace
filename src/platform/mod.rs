pub mod x11;
pub mod wayland;

pub trait PlatformBackend {
    fn run(&mut self, canvas: &mut crate::core::Canvas) -> Result<(), Box<dyn std::error::Error>>;
}

pub fn create_backend() -> Box<dyn PlatformBackend> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        println!("WAYLAND_DISPLAY detected. Attempting Wayland backend...");
        Box::new(wayland::WaylandBackend::new())
    } else {
        println!("Using X11 backend...");
        Box::new(x11::X11Backend::new())
    }
}
