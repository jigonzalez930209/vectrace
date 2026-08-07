/// X11 event handlers extracted from the main run() loop.
/// Each function handles a specific X11 event type.
use crate::core::{Canvas, Point, StrokeType};
use crate::platform::x11::backend::X11Backend;
use crate::platform::x11::render::{get_dirty_rect, compute_crop_dirty_rect};
use crate::platform::x11::window::{focus_x11_window, keysym_to_char};
use crate::platform::x11::{CropDragState, CropHandle, CropHitResult, hit_test_crop, Tool};
use crate::ui::{Toolbar, ToolbarAction};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ModMask, Rectangle};

impl X11Backend {
    pub fn handle_button_press(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
        click_x: f32,
        click_y: f32,
        now_ms: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        focus_x11_window(conn, root, win_id);

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
                        focus_x11_window(conn, root, win_id);
                    }
                }
                ToolbarAction::SelectShape(kind) => {
                    self.crop_start = None;
                    self.crop_current = None;
                    self.crop_drag_state = CropDragState::None;
                    self.active_tool = Tool::default_shape(kind);
                    self.show_color_menu = false;
                }
                ToolbarAction::SetColor(color) => {
                    self.active_tool.set_color(color);
                    self.show_color_menu = false;
                    self.apply_passthrough(conn, win_id, root, toolbar)?;
                }
                ToolbarAction::ToggleColorMenu => {
                    self.show_color_menu = !self.show_color_menu;
                    self.show_settings_menu = false;
                    self.apply_passthrough(conn, win_id, root, toolbar)?;
                }
                ToolbarAction::ToggleBackgroundMode => {
                    canvas.cycle_background_mode();
                }
                ToolbarAction::Clear => {
                    canvas.clear();
                    self.crop_start = None;
                    self.crop_current = None;
                    self.crop_drag_state = CropDragState::None;
                }
                ToolbarAction::SaveFull => {
                    self.trigger_save_full(conn, win_id, root, canvas);
                }
                ToolbarAction::ConfirmCrop => {
                    if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start.take(), self.crop_current.take()) {
                        self.trigger_save_crop(conn, win_id, root, canvas, sx, sy, cx, cy);
                    }
                    self.active_tool = Tool::default_pen();
                    self.crop_drag_state = CropDragState::None;
                }
                ToolbarAction::TogglePassthrough => {
                    self.passthrough = !self.passthrough;
                    self.apply_passthrough(conn, win_id, root, toolbar)?;
                }
                ToolbarAction::ToggleSettingsMenu => {
                    self.show_settings_menu = !self.show_settings_menu;
                    self.show_color_menu = false;
                    self.apply_passthrough(conn, win_id, root, toolbar)?;
                }
                ToolbarAction::ToggleMonitorMode => { /* handled in run() */ }
                ToolbarAction::MinimizeToTray => {
                    self.set_hidden(conn, win_id, root, gc_id, canvas, toolbar, true)?;
                }
                ToolbarAction::Exit => { /* handled in run() */ }
            }
            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
        } else if !self.passthrough {
            if matches!(self.active_tool, Tool::SelectRegion) {
                if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
                    let (hit, (min_x, min_y, max_x, max_y)) = hit_test_crop(sx, sy, cx, cy, click_x, click_y, self.scale_factor);
                    match hit {
                        CropHitResult::Handle(h) => {
                            self.crop_drag_state = CropDragState::Resizing { handle: h, initial_rect: (min_x, min_y, max_x, max_y) };
                        }
                        CropHitResult::Inside => {
                            self.crop_drag_state = CropDragState::Moving { start_mouse: (click_x, click_y), initial_rect: (min_x, min_y, max_x, max_y) };
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
                self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
            } else if matches!(self.active_tool, Tool::Text { .. }) {
                if canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Text) {
                    canvas.finish_current_stroke();
                    self.completed_strokes_dirty = true;
                }
                if let Some(mut stroke) = self.active_tool.create_stroke() {
                    stroke.points = vec![Point::new(click_x, click_y, 1.0, now_ms)];
                    canvas.start_stroke(stroke);
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
                self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
            } else if !matches!(self.active_tool, Tool::SelectRegion) {
                let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, None);
                self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, dirty_rect)?;
            }
        }
        Ok(())
    }

    pub fn handle_motion(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &mut Toolbar,
        move_x: f32,
        move_y: f32,
        now_ms: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_dragging {
            let old_x = toolbar.x;
            let old_y = toolbar.y;
            toolbar.x = (move_x - self.drag_offset_x).max(0.0).min(self.width as f32 - toolbar.width);
            toolbar.y = (move_y - self.drag_offset_y).max(0.0).min(self.height as f32 - toolbar.height);

            let dirty_x = (old_x.min(toolbar.x) - 10.0).max(0.0) as u16;
            let dirty_y = (old_y.min(toolbar.y) - 10.0).max(0.0) as u16;
            let dirty_w = (old_x.max(toolbar.x) + toolbar.width + 10.0 - dirty_x as f32).min(self.width as f32) as u16;
            let dirty_h = (old_y.max(toolbar.y) + toolbar.height + 150.0 - dirty_y as f32).min(self.height as f32) as u16;

            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, Some(Rectangle {
                x: dirty_x as i16, y: dirty_y as i16, width: dirty_w, height: dirty_h,
            }))?;
            return Ok(());
        }

        if matches!(self.active_tool, Tool::SelectRegion) && self.crop_drag_state != CropDragState::None {
            let old_crop = if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
                Some((sx, sy, cx, cy))
            } else { None };

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
                        CropHandle::TopLeft     => { min_x = move_x; min_y = move_y; }
                        CropHandle::TopRight    => { max_x = move_x; min_y = move_y; }
                        CropHandle::BottomLeft  => { min_x = move_x; max_y = move_y; }
                        CropHandle::BottomRight => { max_x = move_x; max_y = move_y; }
                        CropHandle::Top         => { min_y = move_y; }
                        CropHandle::Bottom      => { max_y = move_y; }
                        CropHandle::Left        => { min_x = move_x; }
                        CropHandle::Right       => { max_x = move_x; }
                    }
                    self.crop_start = Some((min_x, min_y));
                    self.crop_current = Some((max_x, max_y));
                }
                CropDragState::None => {}
            }

            let new_crop = if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
                Some((sx, sy, cx, cy))
            } else { None };

            let dirty_rect = compute_crop_dirty_rect(old_crop, new_crop, self.width, self.height, self.scale_factor);
            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, dirty_rect)?;
            return Ok(());
        }

        if canvas.current_stroke().is_some() && !self.passthrough {
            if matches!(self.active_tool, Tool::Text { .. }) {
                return Ok(());
            }

            let is_spotlight = canvas.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Spotlight);
            let is_shape = canvas.current_stroke().map_or(false, |s| {
                s.stroke_type != StrokeType::Freehand
                    && s.stroke_type != StrokeType::Text
                    && s.stroke_type != StrokeType::Laser
                    && s.stroke_type != StrokeType::Spotlight
            });

            if is_spotlight {
                let old_pt = self.prev_spotlight_point;
                let new_pt = Point::new(move_x, move_y, 1.0, now_ms);
                if let Some(stroke) = canvas.current_stroke_mut() {
                    stroke.points = vec![new_pt];
                }
                self.prev_spotlight_point = Some(new_pt);
                let dirty_rect = get_dirty_rect(canvas, self.width, self.height, old_pt, None);
                self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, dirty_rect)?;
            } else if is_shape {
                let old_bounds = self.prev_shape_bounds;
                let new_shape_pt = Point::new(move_x, move_y, 1.0, now_ms);
                if let Some(stroke) = canvas.current_stroke_mut() {
                    if stroke.points.len() >= 2 { stroke.points[1] = new_shape_pt; }
                    else { stroke.add_point(new_shape_pt); }
                    let p1 = stroke.points[0];
                    let p2 = *stroke.points.last().unwrap();
                    self.prev_shape_bounds = Some((p1.x.min(p2.x), p1.y.min(p2.y), p1.x.max(p2.x), p1.y.max(p2.y)));
                }
                let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, old_bounds);
                self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, dirty_rect)?;
            } else {
                canvas.add_point_to_current_stroke(Point::new(move_x, move_y, 1.0, now_ms));
                let dirty_rect = get_dirty_rect(canvas, self.width, self.height, None, None);
                self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, dirty_rect)?;
            }
        }
        Ok(())
    }

    pub fn handle_button_release(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if matches!(self.active_tool, Tool::SelectRegion) && self.crop_drag_state != CropDragState::None {
            self.crop_drag_state = CropDragState::None;
            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
        } else {
            if self.is_dragging {
                self.is_dragging = false;
                self.apply_passthrough(conn, win_id, root, toolbar)?;
                self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
            }
            if canvas.current_stroke().is_some() {
                self.prev_shape_bounds = None;
                if !matches!(self.active_tool, Tool::Text { .. }) && !matches!(self.active_tool, Tool::Spotlight { .. }) {
                    canvas.finish_current_stroke();
                    self.completed_strokes_dirty = true;
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                }
            }
        }
        Ok(())
    }

    pub fn handle_key_press(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        root: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
        keysym: u32,
        state: u16,
        keycode: u8,
        keycode_a: u8,
        now_ms: u64,
    ) -> Result<bool, Box<dyn std::error::Error>> {
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

        let is_ctrl = (state & u16::from(ModMask::CONTROL)) != 0;
        let is_alt  = (state & u16::from(ModMask::M1)) != 0;

        // Global hotkey: Ctrl+Alt+A
        if keycode_a > 0 && keycode == keycode_a && is_ctrl && is_alt {
            self.passthrough = !self.passthrough;
            self.apply_passthrough(conn, win_id, root, toolbar)?;
            self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
            return Ok(false);
        }

        let is_typing_text = matches!(self.active_tool, Tool::Text { .. });

        if is_typing_text {
            if canvas.current_stroke().is_none() {
                if let Some(mut stroke) = self.active_tool.create_stroke() {
                    stroke.points = vec![Point::new(self.width as f32 / 2.0, self.height as f32 / 2.0, 1.0, now_ms)];
                    canvas.start_stroke(stroke);
                }
            }
            match keysym {
                XK_RETURN | XK_KP_ENTER => {
                    canvas.finish_current_stroke();
                    self.completed_strokes_dirty = true;
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                }
                XK_BACKSPACE => {
                    if let Some(stroke) = canvas.current_stroke_mut() {
                        if let Some(ref mut text) = stroke.text_content { text.pop(); }
                    }
                    let dirty = get_dirty_rect(canvas, self.width, self.height, None, None);
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, dirty)?;
                }
                XK_ESCAPE => {
                    canvas.cancel_current_stroke();
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                }
                _ => {
                    if let Some(ch) = keysym_to_char(keysym) {
                        if let Some(stroke) = canvas.current_stroke_mut() {
                            stroke.text_content.get_or_insert_with(String::new).push(ch);
                        }
                        let dirty = get_dirty_rect(canvas, self.width, self.height, None, None);
                        self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, dirty)?;
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
                        self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                    } else {
                        return Ok(true); // signal "exit"
                    }
                }
                XK_S_LOWER | XK_S_UPPER => {
                    let is_shift = (state & u16::from(ModMask::SHIFT)) != 0;
                    if is_ctrl && is_shift {
                        self.active_tool = Tool::default_select_region();
                    } else {
                        self.trigger_save_full(conn, win_id, root, canvas);
                    }
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                }
                XK_SPACE => {
                    self.passthrough = !self.passthrough;
                    self.apply_passthrough(conn, win_id, root, toolbar)?;
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                }
                XK_B => {
                    canvas.cycle_background_mode();
                    self.completed_strokes_dirty = true;
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                }
                XK_U => {
                    if canvas.undo() {
                        self.completed_strokes_dirty = true;
                        self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                    }
                }
                XK_R => {
                    if canvas.redo() {
                        self.completed_strokes_dirty = true;
                        self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                    }
                }
                XK_C_LOWER | XK_C_UPPER => {
                    if is_ctrl {
                        self.active_tool = Tool::default_select_region();
                    } else {
                        canvas.clear();
                        self.completed_strokes_dirty = true;
                    }
                    self.redraw_rect(conn, win_id, gc_id, canvas, toolbar, None)?;
                }
                _ => {}
            }
        }

        Ok(false)
    }
}
