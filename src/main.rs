mod core;
mod platform;
mod ui;

use crate::core::Canvas;
use crate::platform::create_backend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Vectrace Screen Marker...");

    let mut canvas = Canvas::new(0, 0); // Backend will resize dynamically to screen resolution
    let mut backend = create_backend();

    backend.run(&mut canvas)?;

    println!("Vectrace exited successfully.");
    Ok(())
}
