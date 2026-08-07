/// X11Backend struct definition and save/capture operations.
use crate::core::{Canvas, Point, Tool, BackgroundMode, MonitorMode, ToastNotification};
use crate::platform::tray::TrayEvent;
use crate::platform::x11::CropDragState;
use crate::ui::Toolbar;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Rectangle, ClipOrdering, ConnectionExt as _, InputFocus, Time,
};
use x11rb::protocol::shape::{SO as ShapeOp, SK as ShapeKind, ConnectionExt as _};
use std::sync::mpsc::Receiver;

pub struct X11Backend {
    pub width: u16,
    pub height: u16,
    pub passthrough: bool,
    pub active_tool: Tool,
    pub scale_factor: f32,
    pub show_settings_menu: bool,
    pub show_color_menu: bool,
    pub monitor_mode: MonitorMode,
    pub is_hidden: bool,
    pub is_dragging: bool,
    pub drag_offset_x: f32,
    pub drag_offset_y: f32,
    pub tray_rx: Option<Receiver<TrayEvent>>,

    // Persistent buffers to prevent per-frame allocations
    pub base_pixmap: Option<tiny_skia::Pixmap>,
    pub active_pixmap: Option<tiny_skia::Pixmap>,
    pub x11_pixels: Vec<u8>,
    pub completed_strokes_dirty: bool,
    pub prev_spotlight_point: Option<Point>,
    pub prev_shape_bounds: Option<(f32, f32, f32, f32)>,
    pub toast_notification: Option<ToastNotification>,
    pub cached_desktop: Option<tiny_skia::Pixmap>,
    pub crop_start: Option<(f32, f32)>,
    pub crop_current: Option<(f32, f32)>,
    pub crop_drag_state: CropDragState,
    pub hover_tooltip: Option<String>,
    pub mouse_pos: (f32, f32),
    /// Keycodes grabbed on the root window while the overlay is active (XWayland-safe).
    pub overlay_keycodes: Vec<u8>,
}

impl X11Backend {
    pub fn new() -> Self {
        let scale_factor = crate::platform::x11::window::detect_scale_factor();
        Self {
            width: 0,
            height: 0,
            passthrough: false,
            active_tool: Tool::default_pen(),
            scale_factor,
            show_settings_menu: false,
            show_color_menu: false,
            monitor_mode: MonitorMode::Primary,
            is_hidden: false,
            is_dragging: false,
            drag_offset_x: 0.0,
            drag_offset_y: 0.0,
            tray_rx: None,
            base_pixmap: None,
            active_pixmap: None,
            x11_pixels: Vec::new(),
            completed_strokes_dirty: true,
            prev_spotlight_point: None,
            prev_shape_bounds: None,
            toast_notification: None,
            cached_desktop: None,
            crop_start: None,
            crop_current: None,
            crop_drag_state: CropDragState::None,
            hover_tooltip: None,
            mouse_pos: (0.0, 0.0),
            overlay_keycodes: Vec::new(),
        }
    }

    pub fn new_with_tray(tray_rx: Receiver<TrayEvent>) -> Self {
        let mut backend = Self::new();
        backend.tray_rx = Some(tray_rx);
        backend
    }

    pub fn trigger_save_full(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        canvas: &mut Canvas,
    ) {
        let w = self.width as u32;
        let h = self.height as u32;
        if w == 0 || h == 0 { return; }

        let bg_mode = canvas.background_mode;
        let doc = canvas.snapshot();

        if bg_mode == BackgroundMode::Transparent {
            self.cached_desktop = capture_desktop_background(conn, win_id, root, self.width, self.height);
        }
        let desktop_opt = self.cached_desktop.clone();

        if bg_mode == BackgroundMode::Transparent && desktop_opt.is_none() {
            println!("Capture failed: desktop background unavailable (Transparent mode)");
            self.toast_notification = Some(ToastNotification::new("Capture failed (flash path disabled)", 3000));
            return;
        }

        std::thread::spawn(move || {
            let mut temp_pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
            if let Some(desktop_pixmap) = desktop_opt {
                temp_pixmap = desktop_pixmap;
            } else {
                doc.render_background(&mut temp_pixmap);
            }
            for stroke in &doc.strokes {
                crate::core::render::render_stroke(stroke, &mut temp_pixmap);
            }
            match crate::platform::clipboard::save_and_copy_pixmap(&temp_pixmap, None) {
                Ok((path, copied)) => {
                    if copied { println!("Full Screen saved and copied to clipboard: {}", path); }
                    else       { println!("Full Screen saved to: {} (clipboard copy failed)", path); }
                }
                Err(e) => println!("Failed to save full screen: {}", e),
            }
        });

        self.toast_notification = Some(ToastNotification::new("Saved + copied", 3000));
    }

    pub fn trigger_save_crop(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        canvas: &mut Canvas,
        sx: f32,
        sy: f32,
        cx: f32,
        cy: f32,
    ) {
        let w = self.width as u32;
        let h = self.height as u32;
        if w == 0 || h == 0 { return; }

        let scale = self.scale_factor;
        let min_x = ((sx.min(cx) * scale).max(0.0)).min((w.saturating_sub(1)) as f32) as u32;
        let min_y = ((sy.min(cy) * scale).max(0.0)).min((h.saturating_sub(1)) as f32) as u32;
        let crop_w = (((sx - cx).abs() * scale) as u32).min(w - min_x);
        let crop_h = (((sy - cy).abs() * scale) as u32).min(h - min_y);

        if crop_w < 4 || crop_h < 4 { return; }

        let bg_mode = canvas.background_mode;
        let doc = canvas.snapshot();

        let desktop_opt = if bg_mode == BackgroundMode::Transparent {
            let captured = capture_desktop_background(conn, win_id, root, self.width, self.height);
            self.cached_desktop = captured.clone();
            captured
        } else {
            None
        };

        if bg_mode == BackgroundMode::Transparent && desktop_opt.is_none() {
            println!("Capture failed: desktop background unavailable (Transparent mode)");
            self.toast_notification = Some(ToastNotification::new("Capture failed (flash path disabled)", 3000));
            return;
        }

        std::thread::spawn(move || {
            let mut temp_pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
            if let Some(desktop_pixmap) = desktop_opt {
                temp_pixmap = desktop_pixmap;
            } else {
                doc.render_background(&mut temp_pixmap);
            }
            for stroke in &doc.strokes {
                crate::core::render::render_stroke(stroke, &mut temp_pixmap);
            }

            match crate::platform::clipboard::save_and_copy_pixmap(
                &temp_pixmap,
                Some((min_x, min_y, crop_w, crop_h)),
            ) {
                Ok((path, copied)) => {
                    if copied {
                        println!(
                            "Cropped Region ({}x{}) saved and copied to clipboard: {}",
                            crop_w, crop_h, path
                        );
                    } else {
                        println!(
                            "Cropped Region ({}x{}) saved to: {} (clipboard copy failed)",
                            crop_w, crop_h, path
                        );
                    }
                }
                Err(e) => {
                    println!("Failed to save crop region: {}", e);
                }
            }
        });

        self.toast_notification = Some(ToastNotification::new(
            format!("Saved Crop + copied ({}x{})", crop_w, crop_h),
            3000,
        ));
    }

    pub fn set_hidden(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
        hidden: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.is_hidden = hidden;
        if hidden {
            println!("Hiding Vectrace overlay window to System Tray...");
            self.passthrough = true;
            crate::platform::x11::window::ungrab_overlay_keys(conn, root, &self.overlay_keycodes);
            let _ = conn.ungrab_keyboard(Time::CURRENT_TIME);
            let _ = conn.set_input_focus(InputFocus::POINTER_ROOT, 0u32, Time::CURRENT_TIME);
            let _ = conn.unmap_window(win_id);
            let _ = conn.flush();
        } else {
            println!("Restoring Vectrace overlay window from System Tray...");
            let _ = conn.map_window(win_id);
            self.passthrough = false;
            self.completed_strokes_dirty = true;
            self.cached_desktop = capture_desktop_background(conn, win_id, root, self.width, self.height);
            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
            self.apply_passthrough(conn, win_id, root, toolbar)?;
            crate::platform::x11::window::focus_x11_window(conn, root, win_id);
            let _ = conn.flush();
        }
        Ok(())
    }

    pub fn apply_passthrough(
        &self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        toolbar: &Toolbar,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_hidden {
            crate::platform::x11::window::ungrab_overlay_keys(conn, root, &self.overlay_keycodes);
            let _ = conn.ungrab_keyboard(Time::CURRENT_TIME);
            let _ = conn.set_input_focus(InputFocus::POINTER_ROOT, 0u32, Time::CURRENT_TIME);
            let offscreen_rect = [Rectangle { x: -32000, y: -32000, width: 1, height: 1 }];
            x11rb::protocol::shape::rectangles(conn, ShapeOp::SET, ShapeKind::INPUT, ClipOrdering::UNSORTED, win_id, 0, 0, &offscreen_rect)?;
            conn.flush()?;
            return Ok(());
        }

        if self.passthrough {
            // Release overlay key grabs so keystrokes go to the app underneath.
            crate::platform::x11::window::ungrab_overlay_keys(conn, root, &self.overlay_keycodes);
            let _ = conn.ungrab_keyboard(Time::CURRENT_TIME);
            let _ = conn.set_input_focus(InputFocus::POINTER_ROOT, 0u32, Time::CURRENT_TIME);
            let mut rects = vec![Rectangle {
                x: toolbar.x as i16,
                y: toolbar.y as i16,
                width: toolbar.width as u16,
                height: toolbar.height as u16,
            }];
            if self.show_settings_menu {
                let menu_x = toolbar.x + toolbar.settings_btn_logical_x() * toolbar.scale_factor;
                let menu_y = toolbar.y + toolbar.height + 8.0 * toolbar.scale_factor;
                rects.push(Rectangle {
                    x: menu_x as i16,
                    y: menu_y as i16,
                    width: (260.0 * toolbar.scale_factor) as u16,
                    height: (130.0 * toolbar.scale_factor) as u16,
                });
            }
            x11rb::protocol::shape::rectangles(conn, ShapeOp::SET, ShapeKind::INPUT, ClipOrdering::UNSORTED, win_id, 0, 0, &rects)?;
        } else {
            // Root XGrabKey is reliable on XWayland/GNOME; window focus/grab is not.
            crate::platform::x11::window::focus_x11_window(conn, root, win_id);
            crate::platform::x11::window::grab_overlay_keys(conn, root, &self.overlay_keycodes);
            let rect = Rectangle { x: 0, y: 0, width: self.width, height: self.height };
            x11rb::protocol::shape::rectangles(conn, ShapeOp::SET, ShapeKind::INPUT, ClipOrdering::UNSORTED, win_id, 0, 0, &[rect])?;
        }
        conn.flush()?;
        Ok(())
    }
}

/// Captures the desktop background by briefly hiding the overlay window.
/// PERFORMANCE: Sleep reduced to 80ms from 120ms while still giving the compositor
/// enough time to repaint without the overlay.
pub fn capture_desktop_background(
    conn: &impl Connection,
    win_id: u32,
    root: u32,
    w: u16,
    h: u16,
) -> Option<tiny_skia::Pixmap> {
    if w == 0 || h == 0 { return None; }

    let _ = conn.unmap_window(win_id);
    let _ = conn.flush();
    let _ = conn.get_input_focus().map(|c| c.reply());
    // PERFORMANCE: Reduced from 120ms to 80ms — sufficient for most compositors.
    std::thread::sleep(std::time::Duration::from_millis(80));

    let reply = conn.get_image(
        x11rb::protocol::xproto::ImageFormat::Z_PIXMAP,
        root,
        0, 0, w, h, !0,
    ).ok().and_then(|c| c.reply().ok());

    let res = if let Some(reply) = reply {
        let data = reply.data;
        let expected_len = (w as usize) * (h as usize) * 4;
        if data.len() >= expected_len {
            if let Some(mut pixmap) = tiny_skia::Pixmap::new(w as u32, h as u32) {
                let rgba_data = pixmap.data_mut();
                // BGRA -> RGBA conversion using chunks_exact for auto-vectorization
                for (src_px, dst_px) in data.chunks_exact(4).zip(rgba_data.chunks_exact_mut(4)) {
                    dst_px[0] = src_px[2]; // R <- B
                    dst_px[1] = src_px[1]; // G
                    dst_px[2] = src_px[0]; // B <- R
                    dst_px[3] = 255;
                }
                Some(pixmap)
            } else { None }
        } else { None }
    } else {
        // Fallback for XWayland: ScreenCast portal capture
        match crate::platform::wayland::capture::portal::PortalClient::take_screenshot() {
            Ok(desktop_pixmap) => {
                if desktop_pixmap.width() == w as u32 && desktop_pixmap.height() == h as u32 {
                    Some(desktop_pixmap)
                } else {
                    let mut scaled = tiny_skia::Pixmap::new(w as u32, h as u32)?;
                    let scale_x = w as f32 / desktop_pixmap.width() as f32;
                    let scale_y = h as f32 / desktop_pixmap.height() as f32;
                    let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);
                    scaled.draw_pixmap(0, 0, desktop_pixmap.as_ref(), &tiny_skia::PixmapPaint::default(), transform, None);
                    Some(scaled)
                }
            }
            Err(e) => { println!("Desktop capture failed: {:?}", e); None }
        }
    };

    let _ = conn.map_window(win_id);
    let _ = conn.flush();
    res
}
