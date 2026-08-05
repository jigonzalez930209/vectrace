use crate::core::{Canvas, Point, Tool, StrokeType};
use crate::platform::PlatformBackend;
use crate::ui::{Toolbar, ToolbarAction};
use std::error::Error;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ColormapAlloc, CreateWindowAux, CreateGCAux, EventMask, PropMode, WindowClass,
    Rectangle, ImageFormat, ClipOrdering, ConnectionExt as _, InputFocus, Time,
};
use x11rb::protocol::shape::{
    SO as ShapeOp, SK as ShapeKind,
};
use x11rb::wrapper::ConnectionExt as _;

pub struct X11Backend {
    width: u16,
    height: u16,
    passthrough: bool,
    active_tool: Tool,
    
    // Persistent buffers to prevent frame allocations
    base_pixmap: Option<tiny_skia::Pixmap>,
    active_pixmap: Option<tiny_skia::Pixmap>,
    x11_pixels: Vec<u8>,
    completed_strokes_dirty: bool,
}

impl X11Backend {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            passthrough: false,
            active_tool: Tool::default_pen(),
            base_pixmap: None,
            active_pixmap: None,
            x11_pixels: Vec::new(),
            completed_strokes_dirty: true,
        }
    }
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

fn get_dirty_rect(canvas: &Canvas, screen_width: u16, screen_height: u16) -> Option<Rectangle> {
    if let Some(stroke) = canvas.current_stroke() {
        let points = &stroke.points;
        let len = points.len();
        if len >= 1 {
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

            let mut pad = stroke.width + 20.0;
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
    None
}

fn keysym_to_char(keysym: u32) -> Option<char> {
    match keysym {
        0x0020..=0x007e | 0x00a0..=0x00ff => char::from_u32(keysym),
        0x01000000..=0x0110ffff => char::from_u32(keysym - 0x01000000),
        _ => None,
    }
}

impl PlatformBackend for X11Backend {
    fn run(&mut self, canvas: &mut Canvas) -> Result<(), Box<dyn Error>> {
        // 1. Connect to X11 server
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        
        self.width = screen.width_in_pixels;
        self.height = screen.height_in_pixels;
        canvas.resize(self.width as u32, self.height as u32);

        println!("X11 Screen Dimensions: {}x{}", self.width, self.height);

        let mut toolbar = Toolbar::new(self.width as f32);

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
            );

        conn.create_window(
            depth,
            win_id,
            screen.root,
            0,
            0,
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
        
        conn.set_input_focus(InputFocus::PARENT, win_id, Time::CURRENT_TIME)?;
        conn.flush()?;

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

        const XK_ESCAPE: u32 = 0xff1b;
        const XK_SPACE: u32 = 0x0020;
        const XK_BACKSPACE: u32 = 0xff08;
        const XK_RETURN: u32 = 0xff0d;
        const XK_KP_ENTER: u32 = 0xff8d;
        const XK_U: u32 = 0x0075;
        const XK_R: u32 = 0x0072;
        const XK_C: u32 = 0x0063;

        println!("Controls:\n  [Space]  Toggle Click-Through\n  [U]      Undo last stroke\n  [R]      Redo last stroke\n  [C]      Clear canvas\n  [ESC]    Exit application\n");

        self.apply_passthrough(&conn, win_id, &toolbar)?;

        self.completed_strokes_dirty = true;

        let mut pending_events: Vec<Event> = Vec::new();

        loop {
            let event = if let Some(ev) = pending_events.pop() {
                ev
            } else {
                conn.wait_for_event()?
            };

            match event {
                Event::Expose(_) => {
                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                }
                Event::ButtonPress(e) => {
                    if e.detail == 1 {
                        let click_x = e.event_x as f32;
                        let click_y = e.event_y as f32;

                        if let Some(action) = toolbar.handle_click(click_x, click_y) {
                            // If we were typing text and clicked a toolbar button, commit active text
                            if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                                canvas.finish_current_stroke();
                                self.completed_strokes_dirty = true;
                            }

                            match action {
                                ToolbarAction::SelectTool(tool) => {
                                    self.active_tool = tool;
                                    println!("Selected tool: {:?}", tool);
                                }
                                ToolbarAction::SelectShape(kind) => {
                                    self.active_tool = Tool::default_shape(kind);
                                    println!("Selected shape: {:?}", kind);
                                }
                                ToolbarAction::SetColor(color) => {
                                    self.active_tool.set_color(color);
                                    println!("Set active tool color to: {:?}", color);
                                }
                                ToolbarAction::Clear => {
                                    canvas.clear();
                                    self.completed_strokes_dirty = true;
                                    println!("Canvas cleared");
                                }
                                ToolbarAction::TogglePassthrough => {
                                    self.passthrough = !self.passthrough;
                                    println!("Toggled Click-Through: {}", self.passthrough);
                                    self.apply_passthrough(&conn, win_id, &toolbar)?;
                                }
                                ToolbarAction::Exit => {
                                    println!("Exiting via toolbar...");
                                    break;
                                }
                            }
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                        } else if !self.passthrough {
                            // If text tool active, start or commit text stroke
                            if matches!(self.active_tool, Tool::Text { .. }) {
                                if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                                    canvas.finish_current_stroke();
                                    self.completed_strokes_dirty = true;
                                }
                                
                                if let Some(mut stroke) = self.active_tool.create_stroke() {
                                    stroke.points = vec![Point::new(click_x, click_y, 1.0, e.time as u64)];
                                    canvas.start_stroke(stroke);
                                }
                            } else {
                                if let Some(stroke) = self.active_tool.create_stroke() {
                                    canvas.start_stroke(stroke);
                                    canvas.add_point_to_current_stroke(Point::new(
                                        click_x,
                                        click_y,
                                        1.0,
                                        e.time as u64,
                                    ));
                                }
                            }

                            let pad = 30;
                            let rect = Rectangle {
                                x: (click_x - pad as f32).max(0.0) as i16,
                                y: (click_y - pad as f32).max(0.0) as i16,
                                width: (pad * 2) as u16,
                                height: (pad * 2) as u16,
                            };
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, Some(rect))?;
                        }
                    }
                }
                Event::MotionNotify(e) => {
                    if canvas.current_stroke().is_some() && !self.passthrough {
                        // Skip freehand mouse movement updates for text tool
                        if matches!(self.active_tool, Tool::Text { .. }) {
                            continue;
                        }

                        let mut last_x = e.event_x as f32;
                        let mut last_y = e.event_y as f32;
                        let mut last_time = e.time as u64;

                        while let Ok(Some(next_evt)) = conn.poll_for_event() {
                            if let Event::MotionNotify(next_e) = next_evt {
                                last_x = next_e.event_x as f32;
                                last_y = next_e.event_y as f32;
                                last_time = next_e.time as u64;
                            } else {
                                pending_events.push(next_evt);
                                break;
                            }
                        }

                        let is_shape = if let Some(stroke) = canvas.current_stroke() {
                            stroke.stroke_type != StrokeType::Freehand && stroke.stroke_type != StrokeType::Text
                        } else {
                            false
                        };

                        if is_shape {
                            if let Some(stroke) = canvas.current_stroke_mut() {
                                if stroke.points.len() >= 2 {
                                    stroke.points[1] = Point::new(last_x, last_y, 1.0, last_time);
                                } else {
                                    stroke.add_point(Point::new(last_x, last_y, 1.0, last_time));
                                }
                            }
                        } else {
                            canvas.add_point_to_current_stroke(Point::new(last_x, last_y, 1.0, last_time));
                        }

                        let dirty_rect = get_dirty_rect(canvas, self.width, self.height);
                        self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                    }
                }
                Event::ButtonRelease(e) => {
                    if e.detail == 1 && canvas.current_stroke().is_some() {
                        // Don't finish text stroke on mouse release (wait for Enter or click away)
                        if !matches!(self.active_tool, Tool::Text { .. }) {
                            canvas.finish_current_stroke();
                            self.completed_strokes_dirty = true;
                            self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, None)?;
                        }
                    }
                }
                Event::KeyPress(e) => {
                    let keysym = keycode_to_keysym(e.detail, e.state.into());

                    // Check if we are currently typing into an active text stroke
                    let is_typing_text = canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text);

                    if is_typing_text {
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
                                let dirty_rect = get_dirty_rect(canvas, self.width, self.height);
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
                                    }
                                    let dirty_rect = get_dirty_rect(canvas, self.width, self.height);
                                    self.redraw_rect(&conn, win_id, gc_id, canvas, &toolbar, dirty_rect)?;
                                }
                            }
                        }
                    } else {
                        // Global hotkeys when not typing text
                        match keysym {
                            XK_ESCAPE => {
                                println!("Exiting...");
                                break;
                            }
                            XK_SPACE => {
                                self.passthrough = !self.passthrough;
                                println!("Toggled Click-Through: {}", self.passthrough);
                                self.apply_passthrough(&conn, win_id, &toolbar)?;
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
                            XK_C => {
                                canvas.clear();
                                self.completed_strokes_dirty = true;
                                println!("Canvas cleared");
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
                        toolbar = Toolbar::new(self.width as f32);
                        
                        self.x11_pixels.clear();
                        self.completed_strokes_dirty = true;
                        
                        self.apply_passthrough(&conn, win_id, &toolbar)?;
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
    fn apply_passthrough(&self, conn: &impl Connection, win_id: u32, toolbar: &Toolbar) -> Result<(), Box<dyn Error>> {
        if self.passthrough {
            let rect = Rectangle {
                x: toolbar.x as i16,
                y: toolbar.y as i16,
                width: toolbar.width as u16,
                height: toolbar.height as u16,
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
        } else {
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

    fn redraw_rect(&mut self, conn: &impl Connection, win_id: u32, gc_id: u32, canvas: &Canvas, toolbar: &Toolbar, rect: Option<Rectangle>) -> Result<(), Box<dyn Error>> {
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
            base.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0));
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

        toolbar.draw(active, self.active_tool, self.passthrough);

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
