pub mod capture;

use crate::core::{Canvas, Point, Tool, StrokeType, MonitorMode};
use crate::platform::PlatformBackend;
use crate::ui::{Toolbar, ToolbarAction};

use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ColormapAlloc, CreateWindowAux, CreateGCAux, EventMask, PropMode, WindowClass,
    Rectangle, ImageFormat, ClipOrdering, ConnectionExt as _, InputFocus, Time, ModMask, GrabMode,
};
use x11rb::protocol::shape::{
    SO as ShapeOp, SK as ShapeKind, ConnectionExt as _,
};

use x11rb::wrapper::ConnectionExt as _;

use crate::platform::tray::TrayEvent;
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CropDragState {
    None,
    Creating,
    Moving {
        start_mouse: (f32, f32),
        initial_rect: (f32, f32, f32, f32),
    },
    Resizing {
        handle: CropHandle,
        initial_rect: (f32, f32, f32, f32),
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropHitResult {
    Handle(CropHandle),
    Inside,
    Outside,
}

fn hit_test_crop(
    sx: f32, sy: f32, cx: f32, cy: f32,
    px: f32, py: f32, scale: f32,
) -> (CropHitResult, (f32, f32, f32, f32)) {
    let min_x = sx.min(cx);
    let max_x = sx.max(cx);
    let min_y = sy.min(cy);
    let max_y = sy.max(cy);
    let rect = (min_x, min_y, max_x, max_y);

    let margin = 14.0 * scale;

    if (px - min_x).abs() <= margin && (py - min_y).abs() <= margin {
        return (CropHitResult::Handle(CropHandle::TopLeft), rect);
    }
    if (px - max_x).abs() <= margin && (py - min_y).abs() <= margin {
        return (CropHitResult::Handle(CropHandle::TopRight), rect);
    }
    if (px - min_x).abs() <= margin && (py - max_y).abs() <= margin {
        return (CropHitResult::Handle(CropHandle::BottomLeft), rect);
    }
    if (px - max_x).abs() <= margin && (py - max_y).abs() <= margin {
        return (CropHitResult::Handle(CropHandle::BottomRight), rect);
    }
    if (py - min_y).abs() <= margin && px >= min_x - margin && px <= max_x + margin {
        return (CropHitResult::Handle(CropHandle::Top), rect);
    }
    if (py - max_y).abs() <= margin && px >= min_x - margin && px <= max_x + margin {
        return (CropHitResult::Handle(CropHandle::Bottom), rect);
    }
    if (px - min_x).abs() <= margin && py >= min_y - margin && py <= max_y + margin {
        return (CropHitResult::Handle(CropHandle::Left), rect);
    }
    if (px - max_x).abs() <= margin && py >= min_y - margin && py <= max_y + margin {
        return (CropHitResult::Handle(CropHandle::Right), rect);
    }

    if px > min_x && px < max_x && py > min_y && py < max_y {
        return (CropHitResult::Inside, rect);
    }

    (CropHitResult::Outside, rect)
}

fn capture_desktop_background(
    conn: &impl Connection,
    win_id: u32,
    root: u32,
    w: u16,
    h: u16,
) -> Option<tiny_skia::Pixmap> {
    if w == 0 || h == 0 {
        return None;
    }

    // 1. Temporarily unmap overlay window so cyan crop box & control handles are NOT on screen
    let _ = conn.unmap_window(win_id);
    let _ = conn.flush();
    let _ = conn.get_input_focus().map(|c| c.reply());
    // Give the compositor time to composite without our overlay before portal capture.
    std::thread::sleep(std::time::Duration::from_millis(120));

    // 2. Try X11 root capture
    let reply = conn.get_image(
        ImageFormat::Z_PIXMAP,
        root,
        0,
        0,
        w,
        h,
        !0,
    ).ok().and_then(|c| c.reply().ok());

    let res = if let Some(reply) = reply {
        let data = reply.data;
        let expected_len = (w as usize) * (h as usize) * 4;

        if data.len() >= expected_len {
            if let Some(mut pixmap) = tiny_skia::Pixmap::new(w as u32, h as u32) {
                let rgba_data = pixmap.data_mut();
                for i in 0..(w as usize * h as usize) {
                    let src_idx = i * 4;
                    let b = data[src_idx];
                    let g = data[src_idx + 1];
                    let r = data[src_idx + 2];
                    let a = 255u8;

                    rgba_data[src_idx] = r;
                    rgba_data[src_idx + 1] = g;
                    rgba_data[src_idx + 2] = b;
                    rgba_data[src_idx + 3] = a;
                }
                Some(pixmap)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        // Fallback for XWayland: Request 0-flash ScreenCast PipeWire Desktop Frame!
        match crate::platform::wayland::capture::portal::PortalClient::take_screenshot() {
            Ok(desktop_pixmap) => {
                if desktop_pixmap.width() == w as u32 && desktop_pixmap.height() == h as u32 {
                    Some(desktop_pixmap)
                } else {
                    let mut scaled = tiny_skia::Pixmap::new(w as u32, h as u32)?;
                    let scale_x = w as f32 / desktop_pixmap.width() as f32;
                    let scale_y = h as f32 / desktop_pixmap.height() as f32;
                    let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);
                    let paint = tiny_skia::PixmapPaint::default();
                    scaled.draw_pixmap(0, 0, desktop_pixmap.as_ref(), &paint, transform, None);
                    Some(scaled)
                }
            }
            Err(e) => {
                println!("Desktop capture failed: {:?}", e);
                None
            }
        }
    };

    // 3. Remap overlay window IMMEDIATELY
    let _ = conn.map_window(win_id);
    let _ = conn.flush();

    res
}

fn compute_crop_dirty_rect(
    _old_rect: Option<(f32, f32, f32, f32)>,
    _new_rect: Option<(f32, f32, f32, f32)>,
    screen_w: u16,
    screen_h: u16,
    _scale: f32,
) -> Option<Rectangle> {
    // Outside dimming changes across the whole screen when the crop hole moves,
    // so partial dirty uploads look like a dark border around the selection.
    if screen_w == 0 || screen_h == 0 {
        None
    } else {
        Some(Rectangle {
            x: 0,
            y: 0,
            width: screen_w,
            height: screen_h,
        })
    }
}

pub struct X11Backend {
    width: u16,
    height: u16,
    passthrough: bool,
    active_tool: Tool,
    scale_factor: f32,
    show_settings_menu: bool,
    show_color_menu: bool,
    monitor_mode: MonitorMode,
    is_hidden: bool,
    is_dragging: bool,
    drag_offset_x: f32,
    drag_offset_y: f32,
    tray_rx: Option<Receiver<TrayEvent>>,
    
    // Persistent buffers to prevent frame allocations
    base_pixmap: Option<tiny_skia::Pixmap>,
    active_pixmap: Option<tiny_skia::Pixmap>,
    x11_pixels: Vec<u8>,
    completed_strokes_dirty: bool,
    prev_spotlight_point: Option<Point>,
    /// Previous shape AABB (min_x, min_y, max_x, max_y) for clean dirty-rect invalidation.
    prev_shape_bounds: Option<(f32, f32, f32, f32)>,
    toast_notification: Option<crate::core::canvas::ToastNotification>,
    cached_desktop: Option<tiny_skia::Pixmap>,
    crop_start: Option<(f32, f32)>,
    crop_current: Option<(f32, f32)>,
    crop_drag_state: CropDragState,
}

impl X11Backend {
    pub fn new() -> Self {
        let scale_factor = detect_scale_factor();
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
        }
    }

    pub fn new_with_tray(tray_rx: Receiver<TrayEvent>) -> Self {
        let mut backend = Self::new();
        backend.tray_rx = Some(tray_rx);
        backend
    }

    fn trigger_save_full(&mut self, conn: &impl Connection, win_id: u32, root: u32, canvas: &mut Canvas) {
        let w = self.width as u32;
        let h = self.height as u32;
        if w == 0 || h == 0 { return; }

        let bg_mode = canvas.background_mode;
        let doc = canvas.snapshot();

        if bg_mode == crate::core::BackgroundMode::Transparent {
            // Always refresh so a prior portal picker frame cannot poison the cache.
            self.cached_desktop = capture_desktop_background(conn, win_id, root, self.width, self.height);
        }
        let desktop_opt = self.cached_desktop.clone();

        if bg_mode == crate::core::BackgroundMode::Transparent && desktop_opt.is_none() {
            println!("Capture failed: desktop background unavailable (Transparent mode)");
            self.toast_notification = Some(crate::core::canvas::ToastNotification::new(
                "Capture failed (flash path disabled)".to_string(),
                3000,
            ));
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
                crate::core::canvas::render_stroke(stroke, &mut temp_pixmap);
            }

            match crate::platform::clipboard::save_and_copy_pixmap(&temp_pixmap, None) {
                Ok((path, copied)) => {
                    if copied {
                        println!("Full Screen saved and copied to clipboard: {}", path);
                    } else {
                        println!("Full Screen saved to: {} (clipboard copy failed)", path);
                    }
                }
                Err(e) => {
                    println!("Failed to save full screen: {}", e);
                }
            }
        });

        self.toast_notification = Some(crate::core::canvas::ToastNotification::new(
            "Saved + copied".to_string(),
            3000,
        ));
    }

    fn trigger_save_crop(&mut self, conn: &impl Connection, win_id: u32, root: u32, canvas: &mut Canvas, sx: f32, sy: f32, cx: f32, cy: f32) {
        let w = self.width as u32;
        let h = self.height as u32;
        if w == 0 || h == 0 { return; }

        let scale = self.scale_factor;
        let min_x = ((sx.min(cx) * scale).max(0.0)).min((w.saturating_sub(1)) as f32) as u32;
        let min_y = ((sy.min(cy) * scale).max(0.0)).min((h.saturating_sub(1)) as f32) as u32;
        let crop_w = (((sx - cx).abs() * scale) as u32).min(w - min_x);
        let crop_h = (((sy - cy).abs() * scale) as u32).min(h - min_y);

        if crop_w < 4 || crop_h < 4 {
            return;
        }

        let bg_mode = canvas.background_mode;
        let doc = canvas.snapshot();

        if self.cached_desktop.is_none() && bg_mode == crate::core::BackgroundMode::Transparent {
            self.cached_desktop = capture_desktop_background(conn, win_id, root, self.width, self.height);
        }
        let desktop_opt = self.cached_desktop.clone();

        if bg_mode == crate::core::BackgroundMode::Transparent && desktop_opt.is_none() {
            println!("Capture failed: desktop background unavailable (Transparent mode)");
            self.toast_notification = Some(crate::core::canvas::ToastNotification::new(
                "Capture failed (flash path disabled)".to_string(),
                3000,
            ));
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
                crate::core::canvas::render_stroke(stroke, &mut temp_pixmap);
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

        self.toast_notification = Some(crate::core::canvas::ToastNotification::new(
            format!("Saved Crop + copied ({}x{})", crop_w, crop_h),
            3000,
        ));
    }

    fn set_hidden(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
        hidden: bool,
    ) -> Result<(), Box<dyn Error>> {
        self.is_hidden = hidden;
        if hidden {
            println!("Hiding Vectrace overlay window to System Tray (Offscreen 1x1 input shape for XWayland)...");
            self.passthrough = true;
            let _ = conn.ungrab_keyboard(Time::CURRENT_TIME);
            let _ = conn.set_input_focus(InputFocus::POINTER_ROOT, 0u32, Time::CURRENT_TIME);
            let offscreen_rect = [Rectangle {
                x: -32000,
                y: -32000,
                width: 1,
                height: 1,
            }];
            let _ = conn.shape_rectangles(
                ShapeOp::SET,
                ShapeKind::INPUT,
                ClipOrdering::UNSORTED,
                win_id,
                0,
                0,
                &offscreen_rect,
            );
            if let Some(ref mut pixmap) = self.base_pixmap {
                pixmap.fill(tiny_skia::Color::TRANSPARENT);
            }
            if let Some(ref mut pixmap) = self.active_pixmap {
                pixmap.fill(tiny_skia::Color::TRANSPARENT);
            }
            self.x11_pixels.clear();
            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
            let _ = conn.flush();
        }

        else {
            println!("Restoring Vectrace overlay window from System Tray...");
            self.passthrough = false;
            self.completed_strokes_dirty = true;
            self.cached_desktop = capture_desktop_background(conn, win_id, root, self.width, self.height);
            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
            self.apply_passthrough(conn, win_id, root, toolbar)?;
            focus_x11_window(conn, root, win_id);
            let _ = conn.flush();
        }

        Ok(())
    }


}




fn detect_scale_factor() -> f32 {
    if let Ok(val) = std::env::var("GDK_SCALE") {
        if let Ok(scale) = val.parse::<f32>() {
            return scale.max(1.0);
        }
    }
    if let Ok(val) = std::env::var("QT_SCALE_FACTOR") {
        if let Ok(scale) = val.parse::<f32>() {
            return scale.max(1.0);
        }
    }
    if let Ok(val) = std::env::var("VECTRACE_SCALE") {
        if let Ok(scale) = val.parse::<f32>() {
            return scale.max(1.0);
        }
    }
    1.0
}

fn find_32bit_visual(screen: &x11rb::protocol::xproto::Screen) -> Option<(x11rb::protocol::xproto::Visualid, u8)> {
    for depth in &screen.allowed_depths {
        if depth.depth == 32 {
            for visual in &depth.visuals {
                return Some((visual.visual_id, 32));
            }
        }
    }
    None
}

fn get_dirty_rect(
    canvas: &Canvas,
    screen_width: u16,
    screen_height: u16,
    prev_spotlight: Option<Point>,
    prev_shape_bounds: Option<(f32, f32, f32, f32)>,
) -> Option<Rectangle> {
    if let Some(stroke) = canvas.current_stroke() {
        if stroke.stroke_type == StrokeType::Spotlight {
            if let Some(&p) = stroke.points.last() {
                let r = stroke.width + 25.0;
                let mut min_x = p.x - r;
                let mut max_x = p.x + r;
                let mut min_y = p.y - r;
                let mut max_y = p.y + r;

                if let Some(prev) = prev_spotlight {
                    min_x = f32::min(min_x, prev.x - r);
                    max_x = f32::max(max_x, prev.x + r);
                    min_y = f32::min(min_y, prev.y - r);
                    max_y = f32::max(max_y, prev.y + r);
                }

                let x1 = min_x.max(0.0).floor() as i16;
                let y1 = min_y.max(0.0).floor() as i16;
                let x2 = max_x.min(screen_width as f32).ceil() as i16;
                let y2 = max_y.min(screen_height as f32).ceil() as i16;

                let w = (x2 - x1).max(1) as u16;
                let h = (y2 - y1).max(1) as u16;

                return Some(Rectangle {
                    x: x1,
                    y: y1,
                    width: w,
                    height: h,
                });
            }
        } else if stroke.stroke_type == StrokeType::Line
            || stroke.stroke_type == StrokeType::Arrow
            || stroke.stroke_type == StrokeType::Rectangle
            || stroke.stroke_type == StrokeType::Oval
        {
            if stroke.points.len() >= 1 {
                let p1 = stroke.points[0];
                let p2 = *stroke.points.last().unwrap();
                let mut min_x = f32::min(p1.x, p2.x);
                let mut max_x = f32::max(p1.x, p2.x);
                let mut min_y = f32::min(p1.y, p2.y);
                let mut max_y = f32::max(p1.y, p2.y);

                if let Some((ox1, oy1, ox2, oy2)) = prev_shape_bounds {
                    min_x = min_x.min(ox1).min(ox2);
                    max_x = max_x.max(ox1).max(ox2);
                    min_y = min_y.min(oy1).min(oy2);
                    max_y = max_y.max(oy1).max(oy2);
                }

                let pad = stroke.width * 4.0 + 35.0;
                let x1 = (min_x - pad).max(0.0).floor() as i16;
                let y1 = (min_y - pad).max(0.0).floor() as i16;
                let x2 = (max_x + pad).min(screen_width as f32).ceil() as i16;
                let y2 = (max_y + pad).min(screen_height as f32).ceil() as i16;

                let w = (x2 - x1).max(1) as u16;
                let h = (y2 - y1).max(1) as u16;

                return Some(Rectangle {
                    x: x1,
                    y: y1,
                    width: w,
                    height: h,
                });
            }
        } else if stroke.stroke_type == StrokeType::Freehand {
            let points = &stroke.points;
            let len = points.len();
            if len >= 2 {
                let p_last = points[len - 1];
                let p_prev = points[len - 2];
                let mut min_x = f32::min(p_last.x, p_prev.x);
                let mut max_x = f32::max(p_last.x, p_prev.x);
                let mut min_y = f32::min(p_last.y, p_prev.y);
                let mut max_y = f32::max(p_last.y, p_prev.y);

                if len >= 3 {
                    let p_prev2 = points[len - 3];
                    min_x = f32::min(min_x, p_prev2.x);
                    max_x = f32::max(max_x, p_prev2.x);
                    min_y = f32::min(min_y, p_prev2.y);
                    max_y = f32::max(max_y, p_prev2.y);
                }

                let pad = stroke.width + 25.0;
                let x1 = (min_x - pad).max(0.0).floor() as i16;
                let y1 = (min_y - pad).max(0.0).floor() as i16;
                let x2 = (max_x + pad).min(screen_width as f32).ceil() as i16;
                let y2 = (max_y + pad).min(screen_height as f32).ceil() as i16;

                let w = (x2 - x1).max(1) as u16;
                let h = (y2 - y1).max(1) as u16;

                return Some(Rectangle {
                    x: x1,
                    y: y1,
                    width: w,
                    height: h,
                });
            }
        } else {
            let points = &stroke.points;
            if !points.is_empty() {
                let mut min_x = points[0].x;
                let mut max_x = points[0].x;
                let mut min_y = points[0].y;
                let mut max_y = points[0].y;

                for p in points {
                    min_x = f32::min(min_x, p.x);
                    max_x = f32::max(max_x, p.x);
                    min_y = f32::min(min_y, p.y);
                    max_y = f32::max(max_y, p.y);
                }

                let mut pad = stroke.width + 25.0;
                if stroke.stroke_type == StrokeType::Text {
                    if let Some(ref text) = stroke.text_content {
                        pad = pad.max(text.len() as f32 * stroke.font_size + 40.0);
                    }
                }

                let x1 = (min_x - pad).max(0.0).floor() as i16;
                let y1 = (min_y - pad).max(0.0).floor() as i16;
                let x2 = (max_x + pad).min(screen_width as f32).ceil() as i16;
                let y2 = (max_y + pad).min(screen_height as f32).ceil() as i16;

                let w = (x2 - x1).max(1) as u16;
                let h = (y2 - y1).max(1) as u16;

                return Some(Rectangle {
                    x: x1,
                    y: y1,
                    width: w,
                    height: h,
                });
            }
        }
    }
    None
}

fn keysym_to_char(keysym: u32) -> Option<char> {
    match keysym {
        0x0020..=0x007e | 0x00a0..=0x00ff => char::from_u32(keysym),
        0x01000000..=0x0110ffff => char::from_u32(keysym - 0x01000000),
        _ => None,
    }
}

fn grab_global_hotkeys(conn: &impl Connection, root: u32, keycode_a: u8) {
    let modifiers = [
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1), // Ctrl + Alt
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1) | u16::from(ModMask::LOCK),
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1) | u16::from(ModMask::M2),
        u16::from(ModMask::CONTROL) | u16::from(ModMask::M1) | u16::from(ModMask::LOCK) | u16::from(ModMask::M2),
    ];

    for &mod_mask in &modifiers {
        let _ = conn.grab_key(
            true,
            root,
            mod_mask.into(),
            keycode_a,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        );
    }
}

fn focus_x11_window(conn: &impl Connection, root: u32, win_id: u32) {
    let _ = conn.set_input_focus(InputFocus::POINTER_ROOT, win_id, Time::CURRENT_TIME);

    if let Ok(net_active_win) = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW") {
        if let Ok(reply) = net_active_win.reply() {
            let event = x11rb::protocol::xproto::ClientMessageEvent {
                response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
                format: 32,
                sequence: 0,
                window: win_id,
                type_: reply.atom,
                data: x11rb::protocol::xproto::ClientMessageData::from([
                    1u32, // 1 = application request
                    u32::from(Time::CURRENT_TIME),
                    0u32, 0u32, 0u32
                ]),
            };
            let _ = conn.send_event(
                false,
                root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            );
        }
    }

    if let Ok(reply) = conn.grab_keyboard(
        false,
        win_id,
        Time::CURRENT_TIME,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    ) {
        if let Ok(res) = reply.reply() {
            println!("Focused & Grabbed Keyboard (Status: {:?})", res.status);
        }
    }
    let _ = conn.flush();
}

fn detect_primary_monitor(conn: &impl Connection, root: u32) -> Option<(i16, i16, u16, u16)> {
    use x11rb::protocol::randr::ConnectionExt as _;
    if let Ok(cookie) = conn.randr_get_monitors(root, true) {
        if let Ok(reply) = cookie.reply() {
            for mon in &reply.monitors {
                if mon.primary {
                    return Some((mon.x, mon.y, mon.width, mon.height));
                }
            }
            if let Some(first) = reply.monitors.first() {
                return Some((first.x, first.y, first.width, first.height));
            }
        }
    }
    None
}


impl PlatformBackend for X11Backend {
    fn run(&mut self, canvas: &mut Canvas) -> Result<(), Box<dyn Error>> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        
        let root_w = screen.width_in_pixels;
        let root_h = screen.height_in_pixels;

        let primary_mon = detect_primary_monitor(&conn, screen.root);
        let (mon_x, mon_y, mon_w, mon_h) = primary_mon.unwrap_or((0, 0, root_w, root_h));

        println!("Detected Primary Monitor: {}x{}+{}+{}", mon_w, mon_h, mon_x, mon_y);
        println!("Virtual Desktop: {}x{}", root_w, root_h);

        let (win_x, win_y, win_w, win_h) = match self.monitor_mode {
            MonitorMode::Primary => (mon_x, mon_y, mon_w, mon_h),
            MonitorMode::All => (0, 0, root_w, root_h),
        };

        self.width = win_w;
        self.height = win_h;
        canvas.resize(self.width as u32, self.height as u32);
        canvas.set_scale_factor(self.scale_factor);

        let mut toolbar = Toolbar::new_with_scale(mon_w as f32, self.scale_factor);
        if self.monitor_mode == MonitorMode::All {
            toolbar.x += mon_x as f32;
            toolbar.y += mon_y as f32;
        }

        let (visual_id, depth) = find_32bit_visual(screen)
            .unwrap_or((screen.root_visual, screen.root_depth));
        
        let has_transparency = depth == 32;
        if has_transparency {
            println!("X11 Transparency enabled (32-bit visual found).");
        } else {
            println!("WARNING: X11 32-bit visual not found. Falling back to default screen visual (no transparency).");
        }

        let colormap = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual_id)?;

        let win_id = conn.generate_id()?;
        let win_aux = CreateWindowAux::new()
            .colormap(colormap)
            .border_pixel(0)
            .background_pixel(0)
            .override_redirect(1)
            .event_mask(
                EventMask::EXPOSURE
                | EventMask::BUTTON_PRESS
                | EventMask::BUTTON_RELEASE
                | EventMask::POINTER_MOTION
                | EventMask::KEY_PRESS
                | EventMask::STRUCTURE_NOTIFY
                | EventMask::FOCUS_CHANGE
            );

        conn.create_window(
            depth,
            win_id,
            screen.root,
            win_x,
            win_y,
            self.width,
            self.height,
            0,
            WindowClass::INPUT_OUTPUT,
            visual_id,
            &win_aux
        )?;


        let wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let wm_state_above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
        let wm_state_skip_taskbar = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?.reply()?.atom;
        let wm_state_skip_pager = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_PAGER")?.reply()?.atom;
        let wm_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
        let wm_type_dock = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_DOCK")?.reply()?.atom;

        conn.change_property32(
            PropMode::REPLACE,
            win_id,
            wm_type,
            AtomEnum::ATOM,
            &[wm_type_dock]
        )?;

        conn.change_property32(
            PropMode::REPLACE,
            win_id,
            wm_state,
            AtomEnum::ATOM,
            &[wm_state_above, wm_state_skip_taskbar, wm_state_skip_pager]
        )?;

        let gc_id = conn.generate_id()?;
        conn.create_gc(gc_id, win_id, &CreateGCAux::new())?;

        conn.map_window(win_id)?;
        
        focus_x11_window(&conn, screen.root, win_id);

        let min_keycode = conn.setup().min_keycode;
        let max_keycode = conn.setup().max_keycode;
        let keyboard_mapping = conn.get_keyboard_mapping(min_keycode, max_keycode - min_keycode + 1)?.reply()?;
        let keysyms_per_keycode = keyboard_mapping.keysyms_per_keycode as usize;

        let keycode_to_keysym = |keycode: u8, state: u16| -> u32 {
            if keycode < min_keycode || keycode > max_keycode {
                return 0;
            }
            let base_idx = ((keycode - min_keycode) as usize) * keysyms_per_keycode;
            let is_shift = (state & 0x0001) != 0;
            let offset = if is_shift && keysyms_per_keycode > 1 { 1 } else { 0 };
            let idx = base_idx + offset;
            if idx < keyboard_mapping.keysyms.len() {
                keyboard_mapping.keysyms[idx]
            } else {
                0
            }
        };

        const XK_A_LOWER: u32 = 0x0061;
        const XK_A_UPPER: u32 = 0x0041;
        let mut keycode_a = 0u8;

        for kc in min_keycode..=max_keycode {
            let ks = keycode_to_keysym(kc, 0);
            if ks == XK_A_LOWER || ks == XK_A_UPPER {
                keycode_a = kc;
                break;
            }
        }

        if keycode_a > 0 {
            grab_global_hotkeys(&conn, screen.root, keycode_a);
            println!("Registered Global Daemon Shortcut: [Ctrl+Alt+A]");
        }

        const XK_ESCAPE: u32 = 0xff1b;
        const XK_SPACE: u32 = 0x0020;
        const XK_BACKSPACE: u32 = 0xff08;
        const XK_RETURN: u32 = 0xff0d;
        const XK_KP_ENTER: u32 = 0xff8d;
        const XK_U: u32 = 0x0075;
        const XK_R: u32 = 0x0072;
        const XK_C_LOWER: u32 = 0x0063;
        const XK_C_UPPER: u32 = 0x0043;
        const XK_B: u32 = 0x0062;
        const XK_S_LOWER: u32 = 0x0073;
        const XK_S_UPPER: u32 = 0x0053;

        println!("Controls:\n  [Ctrl+Alt+A] Global Toggle Active/Passthrough\n  [Space]      Toggle Click-Through\n  [U]          Undo last stroke\n  [R]          Redo last stroke\n  [C]          Clear canvas\n  [B]          Toggle Blackboard/Whiteboard\n  [ESC]        Exit application\n");

        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;

        self.completed_strokes_dirty = true;

        let mut pending_events: Vec<Event> = Vec::new();

        loop {
            // Process any incoming TrayEvent messages from the system tray menu
            let mut tray_events = Vec::new();
            if let Some(ref rx) = self.tray_rx {
                while let Ok(tray_event) = rx.try_recv() {
                    tray_events.push(tray_event);
                }
            }

            for tray_event in tray_events {
                match tray_event {
                    TrayEvent::ToggleVisibility => {
                        let target_hidden = !self.is_hidden;
                        self.set_hidden(&conn, win_id, screen.root, gc_id, canvas, &toolbar, target_hidden)?;
                    }

                    TrayEvent::ToggleSettingsMenu => {
                        self.show_settings_menu = !self.show_settings_menu;
                        println!("System Tray Action: Toggle Settings Menu = {}", self.show_settings_menu);
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                    }
                    TrayEvent::ToggleMonitorMode => {
                        self.monitor_mode = self.monitor_mode.toggle();
                        println!("System Tray Action: Toggle Monitor Mode = {:?}", self.monitor_mode);
                        let (win_x, win_y, win_w, win_h) = match self.monitor_mode {
                            MonitorMode::Primary => (mon_x, mon_y, mon_w, mon_h),
                            MonitorMode::All => (0, 0, root_w, root_h),
                        };
                        self.width = win_w;
                        self.height = win_h;
                        canvas.resize(self.width as u32, self.height as u32);
                        self.x11_pixels.clear();
                        self.completed_strokes_dirty = true;

                        let _ = conn.configure_window(
                            win_id,
                            &x11rb::protocol::xproto::ConfigureWindowAux::new()
                                .x(win_x as i32)
                                .y(win_y as i32)
                                .width(win_w as u32)
                                .height(win_h as u32),
                        );

                        toolbar = Toolbar::new_with_scale(mon_w as f32, self.scale_factor);
                        if self.monitor_mode == MonitorMode::All {
                            toolbar.x += mon_x as f32;
                            toolbar.y += mon_y as f32;
                        }
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                    }
                    TrayEvent::TogglePassthrough => {
                        self.passthrough = !self.passthrough;
                        println!("System Tray Action: Toggle Passthrough = {}", self.passthrough);
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                    }
                    TrayEvent::CycleBackground => {
                        let mode = canvas.cycle_background_mode();
                        println!("System Tray Action: Cycle Background = {:?}", mode);
                    }
                    TrayEvent::ClearCanvas => {
                        canvas.clear();
                        println!("System Tray Action: Clear Canvas");
                    }
                    TrayEvent::SaveFull => {
                        self.trigger_save_full(&conn, win_id, screen.root, canvas);
                    }
                    TrayEvent::SaveRegion => {
                        self.active_tool = Tool::default_select_region();
                        println!("System Tray Action: Select Region Crop Tool");
                    }
                    TrayEvent::Exit => {
                        println!("System Tray Action: Exit application");
                        return Ok(());
                    }
                }
                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
            }


            let event = if let Some(ev) = pending_events.pop() {
                Some(ev)
            } else if let Ok(Some(ev)) = conn.poll_for_event() {
                Some(ev)
            } else {
                None
            };

            if event.is_none() {
                std::thread::sleep(std::time::Duration::from_millis(16));
                if let Some(ref s) = canvas.current_stroke() {
                    if s.stroke_type == StrokeType::Laser && !s.points.is_empty() {
                        let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, None);
                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                    }
                }
                continue;
            }

            let event = event.unwrap();


            match event {
                Event::Expose(_) => {
                    focus_x11_window(&conn, screen.root, win_id);
                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                }
                Event::ButtonPress(e) => {
                    if e.detail == 1 {
                        let click_x = e.event_x as f32;
                        let click_y = e.event_y as f32;

                        focus_x11_window(&conn, screen.root, win_id);

                        let has_crop = self.crop_start.is_some() && self.crop_current.is_some();

                        if let Some(action) = toolbar.handle_click(click_x, click_y, self.show_settings_menu, self.show_color_menu, has_crop) {
                            if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                                canvas.finish_current_stroke();
                            }

                            canvas.cancel_current_stroke();
                            self.completed_strokes_dirty = true;

                            match action {
                                ToolbarAction::StartDrag => {
                                    self.is_dragging = true;
                                    self.drag_offset_x = click_x - toolbar.x;
                                    self.drag_offset_y = click_y - toolbar.y;
                                    println!("Started dragging toolbar from ({:.0}, {:.0})", toolbar.x, toolbar.y);
                                }
                                ToolbarAction::SelectTool(tool) => {
                                    if !matches!(tool, Tool::SelectRegion) {
                                        self.crop_start = None;
                                        self.crop_current = None;
                                        self.crop_drag_state = CropDragState::None;
                                    }
                                    self.active_tool = tool;
                                    self.show_color_menu = false;
                                    if matches!(tool, Tool::Text { .. }) {
                                        focus_x11_window(&conn, screen.root, win_id);
                                    }
                                    println!("Selected tool: {:?}", tool);
                                }
                                ToolbarAction::SelectShape(kind) => {
                                    self.crop_start = None;
                                    self.crop_current = None;
                                    self.crop_drag_state = CropDragState::None;
                                    self.active_tool = Tool::default_shape(kind);
                                    self.show_color_menu = false;
                                    println!("Selected shape: {:?}", kind);
                                }
                                ToolbarAction::SetColor(color) => {
                                    self.active_tool.set_color(color);
                                    self.show_color_menu = false;
                                    println!("Set active tool color to: {:?}", color);
                                    self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                                }
                                ToolbarAction::ToggleColorMenu => {
                                    self.show_color_menu = !self.show_color_menu;
                                    self.show_settings_menu = false;
                                    println!("Toggled Color Menu: {}", self.show_color_menu);
                                    self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                                }
                                ToolbarAction::ToggleBackgroundMode => {
                                    let mode = canvas.cycle_background_mode();
                                    println!("Switched background mode to: {:?}", mode);
                                }
                                ToolbarAction::Clear => {
                                    canvas.clear();
                                    self.crop_start = None;
                                    self.crop_current = None;
                                    self.crop_drag_state = CropDragState::None;
                                    println!("Canvas cleared");
                                }
                                ToolbarAction::SaveFull => {
                                    self.trigger_save_full(&conn, win_id, screen.root, canvas);
                                }
                                ToolbarAction::ConfirmCrop => {
                                    if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start.take(), self.crop_current.take()) {
                                        self.trigger_save_crop(&conn, win_id, screen.root, canvas, sx, sy, cx, cy);
                                    }
                                    self.active_tool = Tool::default_pen();
                                    self.crop_drag_state = CropDragState::None;
                                    println!("Confirmed and saved crop selection.");
                                }
                                ToolbarAction::TogglePassthrough => {
                                    self.passthrough = !self.passthrough;
                                    println!("Toggled Click-Through: {}", self.passthrough);
                                    self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                                }
                                ToolbarAction::ToggleSettingsMenu => {
                                    self.show_settings_menu = !self.show_settings_menu;
                                    self.show_color_menu = false;
                                    println!("Toggled Settings Menu: {}", self.show_settings_menu);
                                    self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                                }
                                ToolbarAction::ToggleMonitorMode => {
                                    self.monitor_mode = self.monitor_mode.toggle();
                                    println!("Switched Monitor Mode: {:?}", self.monitor_mode);

                                    let (win_x, win_y, win_w, win_h) = match self.monitor_mode {
                                        MonitorMode::Primary => (mon_x, mon_y, mon_w, mon_h),
                                        MonitorMode::All => (0, 0, root_w, root_h),
                                    };

                                    self.width = win_w;
                                    self.height = win_h;
                                    canvas.resize(self.width as u32, self.height as u32);
                                    self.x11_pixels.clear();
                                    self.completed_strokes_dirty = true;

                                    let _ = conn.configure_window(
                                        win_id,
                                        &x11rb::protocol::xproto::ConfigureWindowAux::new()
                                            .x(win_x as i32)
                                            .y(win_y as i32)
                                            .width(win_w as u32)
                                            .height(win_h as u32),
                                    );

                                    toolbar = Toolbar::new_with_scale(mon_w as f32, self.scale_factor);
                                    if self.monitor_mode == MonitorMode::All {
                                        toolbar.x += mon_x as f32;
                                        toolbar.y += mon_y as f32;
                                    }
                                    self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                                }

                                ToolbarAction::MinimizeToTray => {
                                    self.set_hidden(&conn, win_id, screen.root, gc_id, canvas, &toolbar, true)?;
                                }

                                ToolbarAction::Exit => {
                                    println!("Exiting via toolbar...");
                                    break;
                                }

                            }
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                        } else if !self.passthrough {
                            let now_ms = crate::core::canvas::current_time_ms();

                            if matches!(self.active_tool, Tool::SelectRegion) {
                                if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
                                    let (hit, (min_x, min_y, max_x, max_y)) = hit_test_crop(sx, sy, cx, cy, click_x, click_y, self.scale_factor);
                                    match hit {
                                        CropHitResult::Handle(h) => {
                                            self.crop_drag_state = CropDragState::Resizing { handle: h, initial_rect: (min_x, min_y, max_x, max_y) };
                                        }
                                        CropHitResult::Inside => {
                                            self.crop_drag_state = CropDragState::Moving {
                                                start_mouse: (click_x, click_y),
                                                initial_rect: (min_x, min_y, max_x, max_y),
                                            };
                                        }
                                        CropHitResult::Outside => {
                                            self.crop_start = Some((click_x, click_y));
                                            self.crop_current = Some((click_x, click_y));
                                            self.crop_drag_state = CropDragState::Creating;
                                        }
                                    }
                                } else {
                                    self.crop_start = Some((click_x, click_y));
                                    self.crop_current = Some((click_x, click_y));
                                    self.crop_drag_state = CropDragState::Creating;
                                }
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            } else if matches!(self.active_tool, Tool::Text { .. }) {
                                if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                                    canvas.finish_current_stroke();
                                    self.completed_strokes_dirty = true;
                                }
                                
                                if let Some(mut stroke) = self.active_tool.create_stroke() {
                                    stroke.points = vec![Point::new(click_x, click_y, 1.0, now_ms)];
                                    canvas.start_stroke(stroke);
                                    println!("Started Text input at ({:.0}, {:.0})", click_x, click_y);
                                }
                            } else {
                                if let Some(stroke) = self.active_tool.create_stroke() {
                                    canvas.start_stroke(stroke);
                                    canvas.add_point_to_current_stroke(Point::new(click_x, click_y, 1.0, now_ms));
                                }
                            }

                            if matches!(self.active_tool, Tool::Spotlight { .. }) {
                                self.completed_strokes_dirty = true;
                                self.prev_spotlight_point = Some(Point::new(click_x, click_y, 1.0, now_ms));
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            } else if !matches!(self.active_tool, Tool::SelectRegion) {
                                let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, None);
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                            }
                        }
                    }
                }
                Event::MotionNotify(e) => {
                    let mut move_x = e.event_x as f32;
                    let mut move_y = e.event_y as f32;

                    while let Ok(Some(next_evt)) = conn.poll_for_event() {
                        if let Event::MotionNotify(next_e) = next_evt {
                            move_x = next_e.event_x as f32;
                            move_y = next_e.event_y as f32;
                        } else {
                            pending_events.push(next_evt);
                            break;
                        }
                    }

                    if self.is_dragging {
                        let old_x = toolbar.x;
                        let old_y = toolbar.y;

                        let new_x = (move_x - self.drag_offset_x).max(0.0).min(self.width as f32 - toolbar.width);
                        let new_y = (move_y - self.drag_offset_y).max(0.0).min(self.height as f32 - toolbar.height);
                        toolbar.x = new_x;
                        toolbar.y = new_y;

                        let dirty_x = (old_x.min(toolbar.x) - 10.0).max(0.0) as u16;
                        let dirty_y = (old_y.min(toolbar.y) - 10.0).max(0.0) as u16;
                        let dirty_w = (old_x.max(toolbar.x) + toolbar.width + 10.0 - dirty_x as f32).min(self.width as f32) as u16;
                        let dirty_h = (old_y.max(toolbar.y) + toolbar.height + 150.0 - dirty_y as f32).min(self.height as f32) as u16;

                        let dirty_rect = Rectangle {
                            x: dirty_x as i16,
                            y: dirty_y as i16,
                            width: dirty_w,
                            height: dirty_h,
                        };

                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, Some(dirty_rect))?;
                        continue;
                    }

                    if matches!(self.active_tool, Tool::SelectRegion) && self.crop_drag_state != CropDragState::None {
                        let old_crop = if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
                            Some((sx, sy, cx, cy))
                        } else {
                            None
                        };

                        match self.crop_drag_state {
                            CropDragState::Creating => {
                                self.crop_current = Some((move_x, move_y));
                            }
                            CropDragState::Moving { start_mouse: (mx0, my0), initial_rect: (rx1, ry1, rx2, ry2) } => {
                                let dx = move_x - mx0;
                                let dy = move_y - my0;
                                let rw = rx2 - rx1;
                                let rh = ry2 - ry1;
                                let new_min_x = (rx1 + dx).max(0.0).min(self.width as f32 - rw);
                                let new_min_y = (ry1 + dy).max(0.0).min(self.height as f32 - rh);
                                self.crop_start = Some((new_min_x, new_min_y));
                                self.crop_current = Some((new_min_x + rw, new_min_y + rh));
                            }
                            CropDragState::Resizing { handle, initial_rect: (rx1, ry1, rx2, ry2) } => {
                                let (mut min_x, mut min_y, mut max_x, mut max_y) = (rx1, ry1, rx2, ry2);
                                match handle {
                                    CropHandle::TopLeft => { min_x = move_x; min_y = move_y; }
                                    CropHandle::TopRight => { max_x = move_x; min_y = move_y; }
                                    CropHandle::BottomLeft => { min_x = move_x; max_y = move_y; }
                                    CropHandle::BottomRight => { max_x = move_x; max_y = move_y; }
                                    CropHandle::Top => { min_y = move_y; }
                                    CropHandle::Bottom => { max_y = move_y; }
                                    CropHandle::Left => { min_x = move_x; }
                                    CropHandle::Right => { max_x = move_x; }
                                }
                                self.crop_start = Some((min_x, min_y));
                                self.crop_current = Some((max_x, max_y));
                            }
                            CropDragState::None => {}
                        }

                        let new_crop = if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
                            Some((sx, sy, cx, cy))
                        } else {
                            None
                        };

                        let dirty_rect = compute_crop_dirty_rect(old_crop, new_crop, self.width, self.height, self.scale_factor);
                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                        continue;
                    }

                    if canvas.current_stroke().is_some() && !self.passthrough {
                        if matches!(self.active_tool, Tool::Text { .. }) {
                            continue;
                        }

                        let last_x = move_x;
                        let last_y = move_y;

                        let now_ms = crate::core::canvas::current_time_ms();

                        let is_shape = if let Some(stroke) = canvas.current_stroke() {
                            stroke.stroke_type != StrokeType::Freehand
                                && stroke.stroke_type != StrokeType::Text
                                && stroke.stroke_type != StrokeType::Laser
                                && stroke.stroke_type != StrokeType::Spotlight
                        } else {
                            false
                        };

                        let is_spotlight = if let Some(stroke) = canvas.current_stroke() {
                            stroke.stroke_type == StrokeType::Spotlight
                        } else {
                            false
                        };

                        if is_spotlight {
                            let old_pt = self.prev_spotlight_point;
                            let new_pt = Point::new(last_x, last_y, 1.0, now_ms);
                            if let Some(stroke) = canvas.current_stroke_mut() {
                                stroke.points = vec![new_pt];
                            }
                            self.prev_spotlight_point = Some(new_pt);

                            let dirty_rect = get_dirty_rect(canvas, self.width, self.height, old_pt, None);
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                        } else if is_shape {
                            let old_bounds = self.prev_shape_bounds;
                            let new_shape_pt = Point::new(last_x, last_y, 1.0, now_ms);

                            if let Some(stroke) = canvas.current_stroke_mut() {
                                if stroke.points.len() >= 2 {
                                    stroke.points[1] = new_shape_pt;
                                } else {
                                    stroke.add_point(new_shape_pt);
                                }
                                let p1 = stroke.points[0];
                                let p2 = *stroke.points.last().unwrap();
                                self.prev_shape_bounds = Some((
                                    p1.x.min(p2.x),
                                    p1.y.min(p2.y),
                                    p1.x.max(p2.x),
                                    p1.y.max(p2.y),
                                ));
                            }

                            let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, old_bounds);
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                        } else {
                            canvas.add_point_to_current_stroke(Point::new(last_x, last_y, 1.0, now_ms));
                            let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, None);
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                        }
                    }
                }
                Event::ButtonRelease(e) => {
                    if e.detail == 1 {
                        if matches!(self.active_tool, Tool::SelectRegion) && self.crop_drag_state != CropDragState::None {
                            self.crop_drag_state = CropDragState::None;
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                        } else {
                            if self.is_dragging {
                                self.is_dragging = false;
                                self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            }
                            if canvas.current_stroke().is_some() {
                                self.prev_shape_bounds = None;
                                if !matches!(self.active_tool, Tool::Text { .. }) && !matches!(self.active_tool, Tool::Spotlight { .. }) {
                                    canvas.finish_current_stroke();
                                    self.completed_strokes_dirty = true;
                                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                                }
                            }
                        }
                    }
                }


                Event::KeyPress(e) => {
                    let keysym = keycode_to_keysym(e.detail, e.state.into());

                    // Check for Global Daemon Shortcut (Ctrl+Alt+A ONLY)
                    let is_ctrl = (u16::from(e.state) & u16::from(ModMask::CONTROL)) != 0;
                    let is_alt = (u16::from(e.state) & u16::from(ModMask::M1)) != 0;

                    if keycode_a > 0 && e.detail == keycode_a && is_ctrl && is_alt {
                        self.passthrough = !self.passthrough;
                        println!("Global Shortcut Triggered (Ctrl+Alt+A): Passthrough={}", self.passthrough);
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                        continue;
                    }

                    let is_typing_text = matches!(self.active_tool, Tool::Text { .. });

                    if is_typing_text {
                        if canvas.current_stroke().is_none() {
                            let now_ms = crate::core::canvas::current_time_ms();
                            if let Some(mut stroke) = self.active_tool.create_stroke() {
                                stroke.points = vec![Point::new(self.width as f32 / 2.0, self.height as f32 / 2.0, 1.0, now_ms)];
                                canvas.start_stroke(stroke);
                            }
                        }

                        match keysym {
                            XK_RETURN | XK_KP_ENTER => {
                                canvas.finish_current_stroke();
                                self.completed_strokes_dirty = true;
                                println!("Committed text stroke");
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            }
                            XK_BACKSPACE => {
                                if let Some(stroke) = canvas.current_stroke_mut() {
                                    if let Some(ref mut text) = stroke.text_content {
                                        text.pop();
                                    }
                                }
                                let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, None);
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                            }
                            XK_ESCAPE => {
                                canvas.cancel_current_stroke();
                                println!("Cancelled text stroke");
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            }
                            _ => {
                                if let Some(ch) = keysym_to_char(keysym) {
                                    if let Some(stroke) = canvas.current_stroke_mut() {
                                        let text = stroke.text_content.get_or_insert_with(String::new);
                                        text.push(ch);
                                        println!("Typed char in text box: {:?}", ch);
                                    }
                                    let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, None);
                                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                                }
                            }
                        }
                    } else {
                        match keysym {
                            XK_ESCAPE => {
                                if self.crop_start.is_some() || matches!(self.active_tool, Tool::SelectRegion) {
                                    self.crop_start = None;
                                    self.crop_current = None;
                                    self.active_tool = Tool::default_pen();
                                    println!("Cancelled Crop Selection");
                                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                                } else {
                                    println!("Exiting...");
                                    break;
                                }
                            }
                            XK_S_LOWER | XK_S_UPPER => {
                                let is_shift = (u16::from(e.state) & u16::from(ModMask::SHIFT)) != 0;
                                let is_ctrl = (u16::from(e.state) & u16::from(ModMask::CONTROL)) != 0;
                                if is_ctrl && is_shift {
                                    self.active_tool = Tool::default_select_region();
                                    println!("Activated Crop Selection Tool via Ctrl+Shift+S");
                                } else {
                                    self.trigger_save_full(&conn, win_id, screen.root, canvas);
                                }
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            }
                            XK_SPACE => {
                                self.passthrough = !self.passthrough;
                                println!("Toggled Click-Through: {}", self.passthrough);
                                self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            }
                            XK_B => {
                                let mode = canvas.cycle_background_mode();
                                self.completed_strokes_dirty = true;
                                println!("Switched background mode to: {:?}", mode);
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            }
                            XK_U => {
                                if canvas.undo() {
                                    println!("Undo stroke");
                                    self.completed_strokes_dirty = true;
                                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                                }
                            }
                            XK_R => {
                                if canvas.redo() {
                                    println!("Redo stroke");
                                    self.completed_strokes_dirty = true;
                                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                                }
                            }
                            XK_C_LOWER | XK_C_UPPER => {
                                let is_ctrl = (u16::from(e.state) & u16::from(ModMask::CONTROL)) != 0;
                                if is_ctrl {
                                    self.active_tool = Tool::default_select_region();
                                    println!("Activated Crop Selection Tool via Ctrl+C");
                                } else {
                                    canvas.clear();
                                    self.completed_strokes_dirty = true;
                                    println!("Canvas cleared");
                                }
                                self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                            }
                            _ => {}
                        }
                    }
                }
                Event::ConfigureNotify(e) => {
                    if e.width != self.width || e.height != self.height {
                        self.width = e.width;
                        self.height = e.height;
                        canvas.resize(self.width as u32, self.height as u32);
                        toolbar = Toolbar::new_with_scale(self.width as f32, self.scale_factor);
                        
                        self.x11_pixels.clear();
                        self.completed_strokes_dirty = true;
                        
                        self.apply_passthrough(&conn, win_id, screen.root, &toolbar)?;
                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

impl X11Backend {
    fn apply_passthrough(&self, conn: &impl Connection, win_id: u32, root: u32, toolbar: &Toolbar) -> Result<(), Box<dyn Error>> {


        if self.is_hidden {
            let _ = conn.ungrab_keyboard(Time::CURRENT_TIME);
            let _ = conn.set_input_focus(InputFocus::POINTER_ROOT, 0u32, Time::CURRENT_TIME);
            let offscreen_rect = [Rectangle {
                x: -32000,
                y: -32000,
                width: 1,
                height: 1,
            }];
            x11rb::protocol::shape::rectangles(
                conn,
                ShapeOp::SET,
                ShapeKind::INPUT,
                ClipOrdering::UNSORTED,
                win_id,
                0,
                0,
                &offscreen_rect,
            )?;
            conn.flush()?;
            return Ok(());
        }

        if self.passthrough {
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
                let menu_w = 250.0 * toolbar.scale_factor;
                let menu_h = 135.0 * toolbar.scale_factor;
                rects.push(Rectangle {
                    x: menu_x as i16,
                    y: menu_y as i16,
                    width: menu_w as u16,
                    height: menu_h as u16,
                });
            }
            x11rb::protocol::shape::rectangles(
                conn,
                ShapeOp::SET,
                ShapeKind::INPUT,
                ClipOrdering::UNSORTED,
                win_id,
                0,
                0,
                &rects,
            )?;
        } else {
            focus_x11_window(conn, root, win_id);
            let rect = Rectangle {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            };
            x11rb::protocol::shape::rectangles(
                conn,
                ShapeOp::SET,
                ShapeKind::INPUT,
                ClipOrdering::UNSORTED,
                win_id,
                0,
                0,
                &[rect],
            )?;
        }
        conn.flush()?;
        Ok(())
    }

    fn redraw_rect(&mut self, conn: &impl Connection, win_id: u32, gc_id: u32, canvas: &mut Canvas, toolbar: &Toolbar, rect: Option<Rectangle>) -> Result<(), Box<dyn Error>> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let w = self.width as u32;
        let h = self.height as u32;

        let expected_len = (w * h * 4) as usize;
        if self.x11_pixels.len() != expected_len {
            self.x11_pixels = vec![0u8; expected_len];
            self.base_pixmap = Some(tiny_skia::Pixmap::new(w, h).unwrap());
            self.active_pixmap = Some(tiny_skia::Pixmap::new(w, h).unwrap());
            self.completed_strokes_dirty = true;
        }

        let base = self.base_pixmap.as_mut().unwrap();
        let active = self.active_pixmap.as_mut().unwrap();

        let blit_rect = rect.unwrap_or(Rectangle {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        });

        let rx = (blit_rect.x as u32).min(w - 1);
        let ry = (blit_rect.y as u32).min(h - 1);
        let rw = (blit_rect.width as u32).min(w - rx);
        let rh = (blit_rect.height as u32).min(h - ry);

        if rw == 0 || rh == 0 {
            return Ok(());
        }

        if self.completed_strokes_dirty {
            canvas.render_background(base);
            canvas.render_completed_strokes(base);
            self.completed_strokes_dirty = false;
            active.data_mut().copy_from_slice(base.data());
        } else {
            for row in 0..rh {
                let src_row_start = ((ry + row) * w + rx) as usize * 4;
                let len = rw as usize * 4;
                active.data_mut()[src_row_start..src_row_start + len]
                    .copy_from_slice(&base.data()[src_row_start..src_row_start + len]);
            }
        }

        // Render current active stroke
        if let Some(stroke) = canvas.current_stroke() {
            if stroke.stroke_type == StrokeType::Text {
                let cur_text = stroke.text_content.as_deref().unwrap_or("");
                let mut temp_stroke = stroke.clone();
                temp_stroke.text_content = Some(format!("{}|", cur_text));
                
                let mut temp_canvas = Canvas::new(w, h);
                temp_canvas.start_stroke(temp_stroke);
                temp_canvas.render_current_stroke(active);
            } else {
                canvas.render_current_stroke(active);
            }
        }

        let has_crop_selection = self.crop_start.is_some() && self.crop_current.is_some();
        if !self.is_hidden {
            toolbar.draw(
                active,
                self.active_tool,
                self.passthrough,
                canvas.background_mode,
                self.show_settings_menu,
                self.show_color_menu,
                self.monitor_mode,
                has_crop_selection,
            );
        } else {
            active.fill(tiny_skia::Color::TRANSPARENT);
        }

        if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
            crate::core::canvas::render_crop_selection(active, sx, sy, cx - sx, cy - sy, self.scale_factor);
        }

        if let Some(ref toast) = self.toast_notification {
            if !toast.is_expired() {
                toast.draw(active, self.width as f32, self.scale_factor);
            } else {
                self.toast_notification = None;
            }
        }



        let src = active.data();
        let mut sub_pixels = vec![0u8; (rw * rh * 4) as usize];

        for row in 0..rh {
            let src_row_start = ((ry + row) * w + rx) as usize * 4;
            let dst_row_start = (row * rw) as usize * 4;
            
            let src_slice = &src[src_row_start..src_row_start + (rw as usize * 4)];
            let dst_slice = &mut sub_pixels[dst_row_start..dst_row_start + (rw as usize * 4)];

            for p in 0..rw as usize {
                let s = p * 4;
                let d = p * 4;
                dst_slice[d] = src_slice[s + 2];     // B
                dst_slice[d + 1] = src_slice[s + 1]; // G
                dst_slice[d + 2] = src_slice[s];     // R
                dst_slice[d + 3] = src_slice[s + 3]; // A
            }
        }

        conn.put_image(
            ImageFormat::Z_PIXMAP,
            win_id,
            gc_id,
            rw as u16,
            rh as u16,
            rx as i16,
            ry as i16,
            0,
            32, // depth
            &sub_pixels,
        )?;
        conn.flush()?;
        Ok(())
    }
}
