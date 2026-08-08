//! Single background worker for save/export/clipboard jobs.
//! Avoids spawning a new OS thread on every Save.

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use tiny_skia::Pixmap;

use crate::core::document::DocumentSnapshot;
use crate::core::{BackgroundMode, Stroke};

pub enum ExportJob {
    /// Compose desktop + strokes, write PNG, copy clipboard.
    SaveComposed {
        desktop: Option<Arc<Pixmap>>,
        strokes: Arc<[Stroke]>,
        width: u32,
        height: u32,
        overlay_x: i32,
        overlay_y: i32,
        bg_mode: BackgroundMode,
        crop: Option<(u32, u32, u32, u32)>,
        label: &'static str,
    },
    /// Crop save with overlay→desktop mapping after compose.
    SaveCrop {
        desktop: Option<Arc<Pixmap>>,
        strokes: Arc<[Stroke]>,
        width: u32,
        height: u32,
        overlay_x: i32,
        overlay_y: i32,
        bg_mode: BackgroundMode,
        solid_black_bg: bool,
        overlay_crop: (u32, u32, u32, u32),
    },
    /// Already-composited pixmap (rare).
    SavePixmap {
        pixmap: Pixmap,
        crop: Option<(u32, u32, u32, u32)>,
        label: &'static str,
    },
}

fn worker_tx() -> &'static Sender<ExportJob> {
    static TX: OnceLock<Sender<ExportJob>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<ExportJob>();
        thread::Builder::new()
            .name("vectrace-export".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    match job {
                        ExportJob::SaveComposed {
                            desktop,
                            strokes,
                            width,
                            height,
                            overlay_x,
                            overlay_y,
                            bg_mode,
                            crop,
                            label,
                        } => {
                            let doc = DocumentSnapshot {
                                width,
                                height,
                                scale_factor: 1.0,
                                background_mode: bg_mode,
                                strokes: strokes.to_vec(),
                                revision: 0,
                            };
                            let pixmap = crate::core::compose_desktop_with_strokes(
                                desktop.as_ref().map(|a| (**a).clone()),
                                &doc.strokes,
                                width,
                                height,
                                overlay_x,
                                overlay_y,
                                |pm| doc.render_background(pm),
                            );
                            finish_save(&pixmap, crop, label);
                        }
                        ExportJob::SaveCrop {
                            desktop,
                            strokes,
                            width,
                            height,
                            overlay_x,
                            overlay_y,
                            bg_mode,
                            solid_black_bg,
                            overlay_crop,
                        } => {
                            let doc = DocumentSnapshot {
                                width,
                                height,
                                scale_factor: 1.0,
                                background_mode: bg_mode,
                                strokes: strokes.to_vec(),
                                revision: 0,
                            };
                            let pixmap = crate::core::compose_desktop_with_strokes(
                                desktop.as_ref().map(|a| (**a).clone()),
                                &doc.strokes,
                                width,
                                height,
                                overlay_x,
                                overlay_y,
                                |pm| {
                                    if solid_black_bg {
                                        pm.fill(tiny_skia::Color::BLACK);
                                    } else {
                                        doc.render_background(pm);
                                    }
                                },
                            );
                            let crop = crate::core::map_overlay_crop_to_desktop(
                                overlay_crop,
                                width,
                                height,
                                overlay_x,
                                overlay_y,
                                pixmap.width(),
                                pixmap.height(),
                            );
                            finish_save(
                                &pixmap,
                                Some(crop),
                                "Cropped Region",
                            );
                        }
                        ExportJob::SavePixmap { pixmap, crop, label } => {
                            finish_save(&pixmap, crop, label);
                        }
                    }
                }
            })
            .expect("failed to spawn export worker");
        tx
    })
}

fn finish_save(pixmap: &Pixmap, crop: Option<(u32, u32, u32, u32)>, label: &str) {
    match crate::platform::clipboard::save_and_copy_pixmap(pixmap, crop) {
        Ok((path, copied)) => {
            if copied {
                println!(
                    "{} saved {}x{} and copied to clipboard: {}",
                    label,
                    pixmap.width(),
                    pixmap.height(),
                    path
                );
            } else {
                println!(
                    "{} saved {}x{} to: {} (clipboard copy failed)",
                    label,
                    pixmap.width(),
                    pixmap.height(),
                    path
                );
            }
        }
        Err(e) => println!("Failed to save {}: {}", label, e),
    }
}

pub fn submit(job: ExportJob) {
    let _ = worker_tx().send(job);
}

/// Submit a composed save using Arc-shared desktop + stroke list.
pub fn submit_composed(
    desktop: Option<Pixmap>,
    strokes: Vec<Stroke>,
    width: u32,
    height: u32,
    overlay_x: i32,
    overlay_y: i32,
    bg_mode: BackgroundMode,
    crop: Option<(u32, u32, u32, u32)>,
    label: &'static str,
) {
    submit(ExportJob::SaveComposed {
        desktop: desktop.map(Arc::new),
        strokes: strokes.into(),
        width,
        height,
        overlay_x,
        overlay_y,
        bg_mode,
        crop,
        label,
    });
}

/// Clipboard owner thread: keeps offers alive without per-copy spawn+sleep spam.
pub fn copy_image_bytes(width: usize, height: usize, bytes: Vec<u8>) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("Cannot copy empty image to clipboard".into());
    }
    let tx = clipboard_tx();
    tx.lock()
        .map_err(|_| "clipboard worker lock poisoned".to_string())?
        .send(ClipboardJob { width, height, bytes })
        .map_err(|e| format!("clipboard queue: {}", e))
}

struct ClipboardJob {
    width: usize,
    height: usize,
    bytes: Vec<u8>,
}

fn clipboard_tx() -> &'static Mutex<Sender<ClipboardJob>> {
    static TX: OnceLock<Mutex<Sender<ClipboardJob>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<ClipboardJob>();
        thread::Builder::new()
            .name("vectrace-clipboard".into())
            .spawn(move || {
                let Ok(mut clipboard) = arboard::Clipboard::new() else {
                    while rx.recv().is_ok() {}
                    return;
                };
                while let Ok(job) = rx.recv() {
                    let image = arboard::ImageData {
                        width: job.width,
                        height: job.height,
                        bytes: job.bytes.into(),
                    };
                    if clipboard.set_image(image).is_ok() {
                        // Keep ownership long enough for GNOME's clipboard manager.
                        thread::sleep(std::time::Duration::from_millis(1500));
                    }
                }
            })
            .expect("failed to spawn clipboard worker");
        Mutex::new(tx)
    })
}
