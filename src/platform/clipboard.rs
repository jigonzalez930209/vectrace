use arboard::{Clipboard, ImageData};
use std::time::Duration;

pub fn copy_pixmap_to_clipboard(pixmap: &tiny_skia::Pixmap) -> Result<(), String> {
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    if width == 0 || height == 0 {
        return Err("Cannot copy empty image to clipboard".into());
    }

    let image = ImageData {
        width,
        height,
        bytes: pixmap.data().to_vec().into(),
    };

    // On Linux the clipboard offer dies when the Clipboard is dropped. Keep a
    // short-lived owner thread so GNOME's manager can see the image (~300ms).
    std::thread::Builder::new()
        .name("vectrace-clipboard".into())
        .spawn(move || {
            let Ok(mut clipboard) = Clipboard::new() else {
                return;
            };
            if clipboard.set_image(image).is_ok() {
                std::thread::sleep(Duration::from_millis(300));
            }
        })
        .map_err(|e| format!("Failed to spawn clipboard thread: {}", e))?;

    Ok(())
}

/// Prepare export, save to Pictures, and copy to clipboard.
/// Returns `(path, clipboard_ok)`.
pub fn save_and_copy_pixmap(
    pixmap: &tiny_skia::Pixmap,
    crop_rect: Option<(u32, u32, u32, u32)>,
) -> Result<(String, bool), String> {
    let export = crate::core::canvas::prepare_export_pixmap(pixmap, crop_rect)?;
    let path = crate::core::canvas::save_export_pixmap(&export, crop_rect.is_some())?;
    let copied = match copy_pixmap_to_clipboard(&export) {
        Ok(()) => true,
        Err(e) => {
            println!("Clipboard copy failed: {}", e);
            false
        }
    };
    Ok((path, copied))
}
