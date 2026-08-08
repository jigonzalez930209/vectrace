use tiny_skia::Pixmap;

pub fn copy_pixmap_to_clipboard(pixmap: &Pixmap) -> Result<(), String> {
    let width = pixmap.width() as usize;
    let height = pixmap.height() as usize;
    if width == 0 || height == 0 {
        return Err("Cannot copy empty image to clipboard".into());
    }
    crate::platform::export_worker::copy_image_bytes(width, height, pixmap.data().to_vec())
}

/// Prepare export, save to Pictures, and copy to clipboard.
/// Returns `(path, clipboard_ok)`.
pub fn save_and_copy_pixmap(
    pixmap: &Pixmap,
    crop_rect: Option<(u32, u32, u32, u32)>,
) -> Result<(String, bool), String> {
    let export = crate::core::export::prepare_export_pixmap(pixmap, crop_rect)?;
    let path = crate::core::export::save_export_pixmap(&export, crop_rect.is_some())?;
    let copied = match copy_pixmap_to_clipboard(&export) {
        Ok(()) => true,
        Err(e) => {
            println!("Clipboard copy failed: {}", e);
            false
        }
    };
    Ok((path, copied))
}
