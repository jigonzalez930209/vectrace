use vectrace::core::Canvas;
use vectrace::platform::create_backend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        // Signal 2 = SIGINT (Ctrl+C), Signal 15 = SIGTERM (kill)
        let _ = signal_hook_registry::register(2, || {
            println!("\nReceived SIGINT (Ctrl+C). Terminating Vectrace...");
            std::process::exit(0);
        });
        let _ = signal_hook_registry::register(15, || {
            println!("\nReceived SIGTERM. Terminating Vectrace...");
            std::process::exit(0);
        });
    }

    println!("Starting Vectrace Screen Marker...");

    let mut start_hidden = false;
    for arg in std::env::args().skip(1) {
        if arg == "--start-in-tray" || arg == "--hidden" || arg == "--minimized" {
            start_hidden = true;
        }
    }

    let mut canvas = Canvas::new(0, 0); // Backend will resize dynamically to screen resolution
    let mut backend = create_backend(start_hidden);

    backend.run(&mut canvas)?;

    println!("Vectrace exited successfully.");
    Ok(())
}

