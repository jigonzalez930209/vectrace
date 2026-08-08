/// X11Backend struct definition and save/capture operations.
use crate::core::{Canvas, Point, Tool, BackgroundMode, MonitorMode, ToastNotification};
use crate::platform::tray::TrayEvent;
use crate::platform::x11::CropDragState;
use crate::ui::Toolbar;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    MapState, Rectangle, ClipOrdering, ConnectionExt as _,
};
use x11rb::protocol::shape::{SO as ShapeOp, SK as ShapeKind};
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
    /// Overlay position on the virtual desktop (for multi-monitor capture alignment).
    pub overlay_x: i16,
    pub overlay_y: i16,
    pub crop_start: Option<(f32, f32)>,
    pub crop_current: Option<(f32, f32)>,
    pub crop_drag_state: CropDragState,
    pub hover_tooltip: Option<String>,
    pub mouse_pos: (f32, f32),
    /// Keycodes grabbed on the root window while the overlay is active (XWayland-safe).
    pub overlay_keycodes: Vec<u8>,
    /// Escape keycode kept grabbed while mapped (incl. click-through) → tray.
    pub keycode_escape: u8,
    /// Tray "Save Region": crosshair overlay without toolbar; capture on mouse release.
    pub tray_quick_crop: bool,
    /// Cursor id for crosshair (0 = unset).
    pub crosshair_cursor: u32,
    /// Cached dim veil for fast crop dragging (avoids full-screen fill every motion).
    pub crop_veil: Option<tiny_skia::Pixmap>,
    /// `base` with dim overlay baked in — used while dragging a toolbar crop.
    pub crop_dimmed: Option<tiny_skia::Pixmap>,
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
            overlay_x: 0,
            overlay_y: 0,
            crop_start: None,
            crop_current: None,
            crop_drag_state: CropDragState::None,
            hover_tooltip: None,
            mouse_pos: (0.0, 0.0),
            overlay_keycodes: Vec::new(),
            keycode_escape: 0,
            tray_quick_crop: false,
            crosshair_cursor: 0,
            crop_veil: None,
            crop_dimmed: None,
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
        toolbar: &Toolbar,
    ) {
        let w = self.width as u32;
        let h = self.height as u32;
        if w == 0 || h == 0 { return; }

        let overlay_x = self.overlay_x as i32;
        let overlay_y = self.overlay_y as i32;
        let bg_mode = canvas.background_mode;
        let doc = canvas.snapshot();

        if bg_mode == BackgroundMode::Transparent {
            self.cached_desktop = capture_desktop_background(
                conn,
                win_id,
                root,
                self.width,
                self.height,
                self.overlay_x,
                self.overlay_y,
            );
            let _ = self.apply_passthrough(conn, win_id, root, toolbar);
        }

        if bg_mode == BackgroundMode::Transparent && self.cached_desktop.is_none() {
            println!("Capture failed: desktop background unavailable (Transparent mode)");
            self.toast_notification = Some(ToastNotification::new("Capture failed (flash path disabled)", 3000));
            return;
        }

        let desktop = self.cached_desktop.clone();
        crate::platform::export_worker::submit_composed(
            desktop,
            doc.strokes,
            w,
            h,
            overlay_x,
            overlay_y,
            bg_mode,
            None,
            "Full Screen",
        );

        self.toast_notification = Some(ToastNotification::new("Saved + copied", 3000));
    }

    pub fn trigger_save_crop(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
        sx: f32,
        sy: f32,
        cx: f32,
        cy: f32,
        include_annotations: bool,
    ) {
        let w = self.width as u32;
        let h = self.height as u32;
        if w == 0 || h == 0 { return; }

        let overlay_x = self.overlay_x as i32;
        let overlay_y = self.overlay_y as i32;
        let scale = self.scale_factor;
        let min_x = ((sx.min(cx) * scale).max(0.0)).min((w.saturating_sub(1)) as f32) as u32;
        let min_y = ((sy.min(cy) * scale).max(0.0)).min((h.saturating_sub(1)) as f32) as u32;
        let crop_w = (((sx - cx).abs() * scale) as u32).min(w - min_x);
        let crop_h = (((sy - cy).abs() * scale) as u32).min(h - min_y);

        if crop_w < 4 || crop_h < 4 { return; }

        let bg_mode = canvas.background_mode;
        let strokes = if include_annotations {
            canvas.snapshot().strokes
        } else {
            Vec::new()
        };

        let need_desktop = !include_annotations || bg_mode == BackgroundMode::Transparent;
        let desktop_opt = if need_desktop {
            let captured = capture_desktop_background(
                conn,
                win_id,
                root,
                self.width,
                self.height,
                self.overlay_x,
                self.overlay_y,
            );
            self.cached_desktop = captured.clone();
            let _ = self.apply_passthrough(conn, win_id, root, toolbar);
            captured
        } else {
            None
        };

        if need_desktop && desktop_opt.is_none() {
            println!("Capture failed: desktop background unavailable");
            self.toast_notification = Some(ToastNotification::new("Capture failed (flash path disabled)", 3000));
            return;
        }

        crate::platform::export_worker::submit(
            crate::platform::export_worker::ExportJob::SaveCrop {
                desktop: desktop_opt.map(std::sync::Arc::new),
                strokes: strokes.into(),
                width: w,
                height: h,
                overlay_x,
                overlay_y,
                bg_mode,
                solid_black_bg: !include_annotations,
                overlay_crop: (min_x, min_y, crop_w, crop_h),
            },
        );

        self.toast_notification = Some(ToastNotification::new(
            format!("Saved Crop + copied ({}x{})", crop_w, crop_h),
            3000,
        ));
    }

    /// Enter tray region-capture: map a transparent overlay with crosshair, no toolbar.
    pub fn begin_tray_quick_crop(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        toolbar: &Toolbar,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let was_hidden = self.is_hidden;
        self.tray_quick_crop = true;
        self.is_hidden = false;
        self.passthrough = false;
        self.show_settings_menu = false;
        self.show_color_menu = false;
        self.active_tool = Tool::default_select_region();
        self.crop_start = None;
        self.crop_current = None;
        self.crop_drag_state = CropDragState::None;
        self.hover_tooltip = None;

        if was_hidden {
            println!("Tray quick crop: mapping overlay (no toolbar)...");
            let _ = conn.map_window(win_id);
            let _ = conn.flush();
        }

        self.apply_passthrough(conn, win_id, root, toolbar)?;
        crate::platform::x11::window::clear_window_cursor(conn, win_id, self.crosshair_cursor);
        self.crosshair_cursor = crate::platform::x11::window::set_crosshair_cursor(conn, win_id).unwrap_or(0);
        crate::platform::x11::window::claim_keyboard_quiet(conn, root, win_id);
        println!("System Tray Action: Quick region capture (crosshair) — drag to select, release to save, Esc to cancel");
        Ok(())
    }

    /// Leave tray region-capture and return to the system tray.
    pub fn end_tray_quick_crop(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.tray_quick_crop = false;
        self.crop_start = None;
        self.crop_current = None;
        self.crop_drag_state = CropDragState::None;
        self.active_tool = Tool::default_pen();
        self.crop_veil = None;
        crate::platform::x11::window::clear_window_cursor(conn, win_id, self.crosshair_cursor);
        self.crosshair_cursor = 0;
        self.set_hidden(conn, win_id, root, gc_id, canvas, toolbar, true)?;
        Ok(())
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
            crate::platform::x11::window::ungrab_escape_key(conn, root, self.keycode_escape);
            crate::platform::x11::window::release_keyboard_focus(conn, root, win_id);
            let _ = conn.unmap_window(win_id);
            let _ = conn.flush();
        } else {
            println!("Restoring Vectrace overlay window from System Tray...");
            let _ = conn.map_window(win_id);
            self.passthrough = false;
            self.completed_strokes_dirty = true;
            // Do not capture the desktop here — that belongs to Save only.
            // A portal/ScreenCast on every tray Show freezes the desktop UX.
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
            crate::platform::x11::window::ungrab_escape_key(conn, root, self.keycode_escape);
            crate::platform::x11::window::release_keyboard_focus(conn, root, win_id);
            let offscreen_rect = [Rectangle { x: -32000, y: -32000, width: 1, height: 1 }];
            x11rb::protocol::shape::rectangles(conn, ShapeOp::SET, ShapeKind::INPUT, ClipOrdering::UNSORTED, win_id, 0, 0, &offscreen_rect)?;
            conn.flush()?;
            return Ok(());
        }

        if self.passthrough {
            // Release tool keys so the app underneath can receive typing,
            // but keep Escape grabbed so it always minimizes to tray.
            crate::platform::x11::window::ungrab_overlay_keys(conn, root, &self.overlay_keycodes);
            crate::platform::x11::window::grab_escape_key(conn, root, self.keycode_escape);
            // Give focus back to the app/dock under the pointer (not None — that
            // freezes keyboard until Alt+Tab on GNOME/XWayland).
            crate::platform::x11::window::release_keyboard_focus(conn, root, win_id);
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
                    width: (crate::ui::toolbar::SETTINGS_MENU_W * toolbar.scale_factor) as u16,
                    height: (crate::ui::toolbar::SETTINGS_MENU_H * toolbar.scale_factor) as u16,
                });
            }
            if self.show_color_menu {
                let menu_x = toolbar.x + toolbar.color_btn_logical_x() * toolbar.scale_factor;
                let menu_y = toolbar.y + toolbar.height + 6.0 * toolbar.scale_factor;
                rects.push(Rectangle {
                    x: menu_x as i16,
                    y: menu_y as i16,
                    width: (150.0 * toolbar.scale_factor) as u16,
                    height: (110.0 * toolbar.scale_factor) as u16,
                });
            }
            x11rb::protocol::shape::rectangles(conn, ShapeOp::SET, ShapeKind::INPUT, ClipOrdering::UNSORTED, win_id, 0, 0, &rects)?;
        } else {
            crate::platform::x11::window::claim_keyboard(conn, root, win_id);
            // Also grab tool keys on the root — helps on native X11; on XWayland
            // grabs may be ignored while a Wayland app holds the seat.
            crate::platform::x11::window::grab_overlay_keys(conn, root, &self.overlay_keycodes);
            crate::platform::x11::window::grab_escape_key(conn, root, self.keycode_escape);
            let rect = Rectangle { x: 0, y: 0, width: self.width, height: self.height };
            x11rb::protocol::shape::rectangles(conn, ShapeOp::SET, ShapeKind::INPUT, ClipOrdering::UNSORTED, win_id, 0, 0, &[rect])?;
        }
        conn.flush()?;
        Ok(())
    }
}

/// Captures the desktop background by briefly hiding the overlay window.
///
/// If the overlay is already unmapped (e.g. minimized to tray), it is left
/// unmapped — remapping it used to leave a transparent fullscreen window that
/// ate all input ("frozen desktop").
///
/// Portal/Mutter captures are returned at **native** resolution. We never
/// stretch them to the overlay size with independent X/Y scales (that squashed
/// dual-monitor 3840×1080 into 1920×1080).
pub fn capture_desktop_background(
    conn: &impl Connection,
    win_id: u32,
    root: u32,
    w: u16,
    h: u16,
    overlay_x: i16,
    overlay_y: i16,
) -> Option<tiny_skia::Pixmap> {
    if w == 0 || h == 0 { return None; }

    let was_mapped = conn
        .get_window_attributes(win_id)
        .ok()
        .and_then(|cookie| cookie.reply().ok())
        .map(|attrs| attrs.map_state != MapState::UNMAPPED)
        .unwrap_or(true);

    if was_mapped {
        let _ = conn.unmap_window(win_id);
        let _ = conn.flush();
        let _ = conn.get_input_focus().map(|c| c.reply());
        // Brief settle so the compositor can repaint without the overlay.
        // Hard cap: ≤10ms artificial delay before grab.
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // On XWayland, skip root GetImage (unreliable) and go straight to portal/Mutter.
    let on_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let res = if on_wayland {
        match crate::platform::wayland::capture::portal::PortalClient::take_screenshot() {
            Ok(desktop_pixmap) => {
                println!(
                    "Portal/Mutter capture {}x{} (overlay {}x{}+{}+{})",
                    desktop_pixmap.width(),
                    desktop_pixmap.height(),
                    w,
                    h,
                    overlay_x,
                    overlay_y
                );
                Some(desktop_pixmap)
            }
            Err(e) => {
                println!("Desktop capture failed: {:?}", e);
                None
            }
        }
    } else if let Some(reply) = conn
        .get_image(
            x11rb::protocol::xproto::ImageFormat::Z_PIXMAP,
            root,
            overlay_x,
            overlay_y,
            w,
            h,
            !0,
        )
        .ok()
        .and_then(|c| c.reply().ok())
    {
        let data = reply.data;
        let expected_len = (w as usize) * (h as usize) * 4;
        if data.len() >= expected_len {
            tiny_skia::Pixmap::new(w as u32, h as u32).map(|mut pixmap| {
                let rgba_data = pixmap.data_mut();
                for (src_px, dst_px) in data.chunks_exact(4).zip(rgba_data.chunks_exact_mut(4)) {
                    dst_px[0] = src_px[2];
                    dst_px[1] = src_px[1];
                    dst_px[2] = src_px[0];
                    dst_px[3] = 255;
                }
                pixmap
            })
        } else {
            None
        }
    } else {
        match crate::platform::wayland::capture::portal::PortalClient::take_screenshot() {
            Ok(desktop_pixmap) => {
                println!(
                    "Portal/Mutter capture {}x{} (overlay {}x{}+{}+{})",
                    desktop_pixmap.width(),
                    desktop_pixmap.height(),
                    w,
                    h,
                    overlay_x,
                    overlay_y
                );
                Some(desktop_pixmap)
            }
            Err(e) => {
                println!("Desktop capture failed: {:?}", e);
                None
            }
        }
    };

    // Only restore mapping if we temporarily hid a visible overlay.
    if was_mapped {
        let _ = conn.map_window(win_id);
        let _ = conn.flush();
    }
    res
}
