use crate::core::{Tool, Color, ShapeKind, BackgroundMode, MonitorMode, BlendMode, render_text_to_pixmap};


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolbarAction {
    SelectTool(Tool),
    SelectShape(ShapeKind),
    SetColor(Color),
    ToggleBackgroundMode,
    Clear,
    TogglePassthrough,
    ToggleSettingsMenu,
    ToggleColorMenu,
    ToggleMonitorMode,
    MinimizeToTray,
    Exit,
    StartDrag,
}

pub struct Toolbar {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale_factor: f32,
}

impl Toolbar {
    pub fn new(screen_width: f32) -> Self {
        Self::new_with_scale(screen_width, 1.0)
    }

    pub fn new_with_scale(screen_width: f32, scale_factor: f32) -> Self {
        let scale = scale_factor.max(0.5);
        let width = 660.0 * scale;
        let height = 38.0 * scale;
        let x = (screen_width - width) / 2.0;
        let y = 12.0 * scale;
        Self { x, y, width, height, scale_factor: scale }
    }

    pub fn handle_click(
        &self,
        click_x: f32,
        click_y: f32,
        show_settings_menu: bool,
        show_color_menu: bool,
    ) -> Option<ToolbarAction> {
        let scale = self.scale_factor;

        // Check if click is inside Color Popup Menu
        if show_color_menu {
            let menu_x = self.x + 330.0 * scale;
            let menu_y = self.y + self.height + 6.0 * scale;
            let menu_w = 150.0 * scale;
            let menu_h = 110.0 * scale;

            if click_x >= menu_x && click_x <= menu_x + menu_w && click_y >= menu_y && click_y <= menu_y + menu_h {
                let rx = (click_x - menu_x) / scale;
                let ry = (click_y - menu_y) / scale;

                let col = ((rx - 8.0) / 34.0).floor() as i32;
                let row = ((ry - 8.0) / 32.0).floor() as i32;

                if col >= 0 && col < 4 && row >= 0 && row < 3 {
                    let index = (row * 4 + col) as usize;
                    let colors = Self::palette_colors();
                    if let Some(color) = colors.get(index) {
                        return Some(ToolbarAction::SetColor(*color));
                    }
                }
                return None;
            }
        }

        // Check if click is inside Settings Popup Menu
        if show_settings_menu {
            let menu_x = self.x + 400.0 * scale;
            let menu_y = self.y + self.height + 6.0 * scale;
            let menu_w = 240.0 * scale;
            let menu_h = 130.0 * scale;

            if click_x >= menu_x && click_x <= menu_x + menu_w && click_y >= menu_y && click_y <= menu_y + menu_h {
                let ry = (click_y - menu_y) / scale;
                if ry >= 8.0 && ry < 44.0 {
                    return Some(ToolbarAction::ToggleMonitorMode);
                }
                if ry >= 48.0 && ry < 84.0 {
                    return Some(ToolbarAction::TogglePassthrough);
                }
                if ry >= 88.0 && ry < 124.0 {
                    return Some(ToolbarAction::ToggleBackgroundMode);
                }
                return None;
            }
        }

        if click_x < self.x || click_x > self.x + self.width || click_y < self.y || click_y > self.y + self.height {
            return None;
        }

        let rx = (click_x - self.x) / self.scale_factor;

        // Drag Handle (Grip) at rx 0..24
        if rx >= 0.0 && rx < 24.0 {
            return Some(ToolbarAction::StartDrag);
        }

        // Tools (rx 28..330)
        if rx >= 28.0 && rx < 58.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_pen()));
        }
        if rx >= 62.0 && rx < 92.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_highlighter()));
        }
        if rx >= 96.0 && rx < 126.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Line));
        }
        if rx >= 130.0 && rx < 160.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Arrow));
        }
        if rx >= 164.0 && rx < 194.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Rectangle));
        }
        if rx >= 198.0 && rx < 228.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Oval));
        }
        if rx >= 232.0 && rx < 262.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_laser()));
        }
        if rx >= 266.0 && rx < 296.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_spotlight()));
        }
        if rx >= 300.0 && rx < 330.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_eraser()));
        }

        // Color Palette Button (rx 336..380)
        if rx >= 336.0 && rx < 380.0 {
            return Some(ToolbarAction::ToggleColorMenu);
        }

        // Action Buttons (rx 388..650)
        if rx >= 388.0 && rx < 424.0 {
            return Some(ToolbarAction::ToggleBackgroundMode);
        }
        if rx >= 428.0 && rx < 464.0 {
            return Some(ToolbarAction::Clear);
        }
        if rx >= 468.0 && rx < 504.0 {
            return Some(ToolbarAction::TogglePassthrough);
        }
        if rx >= 508.0 && rx < 544.0 {
            return Some(ToolbarAction::ToggleSettingsMenu);
        }
        if rx >= 548.0 && rx < 584.0 {
            return Some(ToolbarAction::MinimizeToTray);
        }
        if rx >= 588.0 && rx < 618.0 {
            return Some(ToolbarAction::Exit);
        }

        Some(ToolbarAction::StartDrag)
    }

    pub fn palette_colors() -> Vec<Color> {
        vec![
            Color::new(235, 50, 50, 255),   // Red
            Color::new(245, 130, 30, 255),  // Orange
            Color::new(245, 210, 30, 255),  // Yellow
            Color::new(50, 205, 80, 255),   // Green
            Color::new(30, 210, 220, 255),  // Cyan
            Color::new(50, 130, 245, 255),  // Blue
            Color::new(150, 60, 245, 255),  // Purple
            Color::new(245, 80, 170, 255),  // Pink
            Color::new(255, 255, 255, 255), // White
            Color::new(180, 180, 180, 255), // Light Gray
            Color::new(70, 70, 70, 255),    // Dark Gray
            Color::new(20, 20, 20, 255),    // Black
        ]
    }

    pub fn draw(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        active_tool: Tool,
        passthrough: bool,
        bg_mode: BackgroundMode,
        show_settings_menu: bool,
        show_color_menu: bool,
        monitor_mode: MonitorMode,
    ) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform, LineCap, LineJoin};

        let scale = self.scale_factor;

        // Main Toolbar Container Background
        let mut pb = PathBuilder::new();
        self.add_rounded_rect(&mut pb, self.x, self.y, self.width, self.height, 10.0 * scale);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(20, 22, 28, 235));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            border_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 40));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
        }

        // 1. Draw Drag Grip Handle (⠿) at left
        let grip_cx = self.x + 12.0 * scale;
        let grip_cy = self.y + self.height / 2.0;
        let mut grip_paint = Paint::default();
        grip_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 120));
        grip_paint.anti_alias = true;

        for dx in &[-3.0 * scale, 3.0 * scale] {
            for dy in &[-6.0 * scale, 0.0, 6.0 * scale] {
                let mut dot_pb = PathBuilder::new();
                dot_pb.push_circle(grip_cx + dx, grip_cy + dy, 1.5 * scale);
                if let Some(dot_path) = dot_pb.finish() {
                    pixmap.fill_path(&dot_path, &grip_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
                }
            }
        }

        // Section Dividers
        let mut div_paint = Paint::default();
        div_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 30));
        let dividers = [24.0, 334.0, 384.0];
        for dx in &dividers {
            let mut pb = PathBuilder::new();
            pb.move_to(self.x + dx * scale, self.y + 6.0 * scale);
            pb.line_to(self.x + dx * scale, self.y + self.height - 6.0 * scale);
            if let Some(path) = pb.finish() {
                let mut stroke = Stroke::default();
                stroke.width = 1.0 * scale;
                pixmap.stroke_path(&path, &div_paint, &stroke, Transform::identity(), None);
            }
        }

        // 2. Draw Tool Buttons (9 tools)
        let tools = [
            ("Pen", 28.0),
            ("Highlighter", 62.0),
            ("Line", 96.0),
            ("Arrow", 130.0),
            ("Rectangle", 164.0),
            ("Oval", 198.0),
            ("Laser", 232.0),
            ("Spotlight", 266.0),
            ("Eraser", 300.0),
        ];

        for (name, bx) in &tools {
            let is_active = match *name {
                "Pen" => matches!(active_tool, Tool::Pen { .. }),
                "Highlighter" => matches!(active_tool, Tool::Highlighter { .. }),
                "Laser" => matches!(active_tool, Tool::Laser { .. }),
                "Spotlight" => matches!(active_tool, Tool::Spotlight { .. }),
                "Eraser" => matches!(active_tool, Tool::Eraser { .. }),
                "Line" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Line, .. }),
                "Arrow" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Arrow, .. }),
                "Rectangle" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Rectangle, .. }),
                "Oval" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Oval, .. }),
                _ => false,
            };

            let x = self.x + bx * scale;
            let y = self.y + 4.0 * scale;
            let w = 30.0 * scale;
            let h = 30.0 * scale;

            let mut btn_pb = PathBuilder::new();
            self.add_rounded_rect(&mut btn_pb, x, y, w, h, 6.0 * scale);
            if let Some(btn_path) = btn_pb.finish() {
                let mut paint = Paint::default();
                if is_active {
                    paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 220));
                } else {
                    paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 12));
                }
                paint.anti_alias = true;
                pixmap.fill_path(&btn_path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            let cx = x + w / 2.0;
            let cy = y + h / 2.0;

            let mut icon_paint = Paint::default();
            icon_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 230));
            icon_paint.anti_alias = true;

            let mut icon_stroke = Stroke::default();
            icon_stroke.width = 1.8 * scale;
            icon_stroke.line_cap = LineCap::Round;
            icon_stroke.line_join = LineJoin::Round;

            match *name {
                "Pen" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 5.0 * scale, cy + 5.0 * scale);
                    ipb.line_to(cx + 4.0 * scale, cy - 4.0 * scale);
                    ipb.line_to(cx + 5.0 * scale, cy - 3.0 * scale);
                    ipb.line_to(cx - 3.0 * scale, cy + 5.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Highlighter" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 5.0 * scale, cy + 4.0 * scale);
                    ipb.line_to(cx + 3.0 * scale, cy - 4.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 1.0 * scale);
                    ipb.line_to(cx - 2.0 * scale, cy + 7.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        let mut hpaint = icon_paint.clone();
                        hpaint.set_color(tiny_skia::Color::from_rgba8(245, 210, 30, 220));
                        pixmap.fill_path(&ipath, &hpaint, tiny_skia::FillRule::Winding, Transform::identity(), None);
                    }
                }
                "Line" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy + 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 5.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Arrow" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy + 4.0 * scale);
                    ipb.line_to(cx + 5.0 * scale, cy - 4.0 * scale);
                    ipb.move_to(cx + 1.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy + 0.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Rectangle" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy + 5.0 * scale);
                    ipb.line_to(cx - 6.0 * scale, cy + 5.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Oval" => {
                    let mut ipb = PathBuilder::new();
                    ipb.push_circle(cx, cy, 5.5 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Laser" => {
                    let mut ipb = PathBuilder::new();
                    ipb.push_circle(cx, cy, 4.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        let mut lpaint = Paint::default();
                        lpaint.set_color(tiny_skia::Color::from_rgba8(245, 50, 50, 240));
                        lpaint.anti_alias = true;
                        pixmap.fill_path(&ipath, &lpaint, tiny_skia::FillRule::Winding, Transform::identity(), None);
                    }
                }
                "Spotlight" => {
                    let mut ipb = PathBuilder::new();
                    ipb.push_circle(cx, cy, 6.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Eraser" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 5.0 * scale, cy + 3.0 * scale);
                    ipb.line_to(cx + 1.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 1.0 * scale);
                    ipb.line_to(cx + 0.0 * scale, cy + 7.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                _ => {}
            }
        }

        // 3. Draw Color Palette Button (🎨) at rx 336
        let color_btn_x = self.x + 336.0 * scale;
        let color_btn_y = self.y + 4.0 * scale;
        let color_btn_w = 42.0 * scale;
        let color_btn_h = 30.0 * scale;

        let mut cb_pb = PathBuilder::new();
        self.add_rounded_rect(&mut cb_pb, color_btn_x, color_btn_y, color_btn_w, color_btn_h, 6.0 * scale);
        if let Some(cb_path) = cb_pb.finish() {
            let mut cb_paint = Paint::default();
            if show_color_menu {
                cb_paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 220));
            } else {
                cb_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 15));
            }
            cb_paint.anti_alias = true;
            pixmap.fill_path(&cb_path, &cb_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        }

        // Active Color Circle Indicator + Palette Dot
        let active_c = active_tool.color().unwrap_or(Color::new(235, 50, 50, 255));
        let circle_cx = color_btn_x + 15.0 * scale;

        let circle_cy = color_btn_y + color_btn_h / 2.0;

        let mut circ_pb = PathBuilder::new();
        circ_pb.push_circle(circle_cx, circle_cy, 7.0 * scale);
        if let Some(circ_path) = circ_pb.finish() {
            let mut cpaint = Paint::default();
            cpaint.set_color(tiny_skia::Color::from_rgba8(active_c.r, active_c.g, active_c.b, active_c.a));
            cpaint.anti_alias = true;
            pixmap.fill_path(&circ_path, &cpaint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut cstroke = Stroke::default();
            cstroke.width = 1.2 * scale;
            let mut cborder = Paint::default();
            cborder.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 200));
            pixmap.stroke_path(&circ_path, &cborder, &cstroke, Transform::identity(), None);
        }

        // Palette Arrow Indicator
        let mut arr_pb = PathBuilder::new();
        let arr_cx = color_btn_x + 30.0 * scale;
        arr_pb.move_to(arr_cx - 3.0 * scale, circle_cy - 2.0 * scale);
        arr_pb.line_to(arr_cx, circle_cy + 2.0 * scale);
        arr_pb.line_to(arr_cx + 3.0 * scale, circle_cy - 2.0 * scale);
        if let Some(arr_path) = arr_pb.finish() {
            let mut apaint = Paint::default();
            apaint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 200));
            apaint.anti_alias = true;
            let mut astroke = Stroke::default();
            astroke.width = 1.5 * scale;
            astroke.line_cap = LineCap::Round;
            astroke.line_join = LineJoin::Round;
            pixmap.stroke_path(&arr_path, &apaint, &astroke, Transform::identity(), None);
        }

        // 4. Draw Action Buttons (Board, Clear, Pass, Settings, Tray, Exit)
        let action_buttons = [
            ("Board", 388.0, 36.0),
            ("Clear", 428.0, 36.0),
            ("Pass", 468.0, 36.0),
            ("Settings", 508.0, 36.0),
            ("Tray", 548.0, 36.0),
            ("Exit", 588.0, 30.0),
        ];

        for (name, bx, bw) in &action_buttons {
            let is_active = match *name {
                "Pass" => passthrough,
                "Board" => bg_mode != BackgroundMode::Transparent,
                "Settings" => show_settings_menu,
                _ => false,
            };

            let x = self.x + bx * scale;
            let y = self.y + 4.0 * scale;
            let w = *bw * scale;
            let h = 30.0 * scale;

            let mut btn_pb = PathBuilder::new();
            self.add_rounded_rect(&mut btn_pb, x, y, w, h, 6.0 * scale);
            if let Some(btn_path) = btn_pb.finish() {
                let mut paint = Paint::default();
                if is_active {
                    paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 220));
                } else {
                    paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 12));
                }
                paint.anti_alias = true;
                pixmap.fill_path(&btn_path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            let cx = x + w / 2.0;
            let cy = y + h / 2.0;

            let mut icon_paint = Paint::default();
            icon_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 230));
            icon_paint.anti_alias = true;

            let mut icon_stroke = Stroke::default();
            icon_stroke.width = 1.8 * scale;
            icon_stroke.line_cap = LineCap::Round;
            icon_stroke.line_join = LineJoin::Round;

            match *name {
                "Board" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy + 5.0 * scale);
                    ipb.line_to(cx - 6.0 * scale, cy + 5.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Clear" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 5.0 * scale);
                    ipb.move_to(cx - 4.0 * scale, cy - 3.0 * scale);
                    ipb.line_to(cx - 3.0 * scale, cy + 6.0 * scale);
                    ipb.line_to(cx + 3.0 * scale, cy + 6.0 * scale);
                    ipb.line_to(cx + 4.0 * scale, cy - 3.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Pass" => {
                    let mut ipb = PathBuilder::new();
                    if passthrough {
                        ipb.move_to(cx - 4.0 * scale, cy - 6.0 * scale);
                        ipb.line_to(cx - 4.0 * scale, cy + 6.0 * scale);
                        ipb.line_to(cx - 1.0 * scale, cy + 2.0 * scale);
                        ipb.line_to(cx + 3.0 * scale, cy + 6.0 * scale);
                        ipb.line_to(cx + 5.0 * scale, cy + 4.0 * scale);
                        ipb.line_to(cx + 1.0 * scale, cy + 0.0 * scale);
                        ipb.line_to(cx + 4.0 * scale, cy - 2.0 * scale);
                        ipb.close();
                    } else {
                        ipb.move_to(cx - 6.0 * scale, cy - 6.0 * scale);
                        ipb.line_to(cx + 6.0 * scale, cy + 6.0 * scale);
                        ipb.move_to(cx - 7.0 * scale, cy);
                        ipb.quad_to(cx, cy - 4.0 * scale, cx + 7.0 * scale, cy);
                        ipb.quad_to(cx, cy + 4.0 * scale, cx - 7.0 * scale, cy);
                    }
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Settings" => {
                    let mut ipb = PathBuilder::new();
                    let r_in = 3.0 * scale;
                    let r_out = 6.0 * scale;
                    let kappa = 0.55228475;
                    ipb.move_to(cx - r_in, cy);
                    ipb.cubic_to(cx - r_in, cy - r_in * kappa, cx - r_in * kappa, cy - r_in, cx, cy - r_in);
                    ipb.cubic_to(cx + r_in * kappa, cy - r_in, cx + r_in, cy - r_in * kappa, cx + r_in, cy);
                    ipb.cubic_to(cx + r_in, cy + r_in * kappa, cx + r_in * kappa, cy + r_in, cx, cy + r_in);
                    ipb.cubic_to(cx - r_in * kappa, cy + r_in, cx - r_in, cy + r_in * kappa, cx - r_in, cy);
                    ipb.close();

                    for i in 0..6 {
                        let angle = (i as f32) * std::f32::consts::PI / 3.0;
                        let dx = angle.cos() * r_out;
                        let dy = angle.sin() * r_out;
                        ipb.move_to(cx + dx * 0.6, cy + dy * 0.6);
                        ipb.line_to(cx + dx, cy + dy);
                    }

                    if let Some(ipath) = ipb.finish() {
                        let mut gear_stroke = icon_stroke.clone();
                        gear_stroke.width = 1.6 * scale;
                        pixmap.stroke_path(&ipath, &icon_paint, &gear_stroke, Transform::identity(), None);
                    }
                }
                "Tray" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy - 1.0 * scale);
                    ipb.line_to(cx - 6.0 * scale, cy + 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy + 5.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 1.0 * scale);

                    ipb.move_to(cx, cy - 5.0 * scale);
                    ipb.line_to(cx, cy + 2.0 * scale);
                    ipb.move_to(cx - 3.0 * scale, cy - 1.0 * scale);
                    ipb.line_to(cx, cy + 2.5 * scale);
                    ipb.line_to(cx + 3.0 * scale, cy - 1.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Exit" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 5.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx + 5.0 * scale, cy + 5.0 * scale);
                    ipb.move_to(cx + 5.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx - 5.0 * scale, cy + 5.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                _ => {}
            }
        }

        // Draw Color Menu Popup if active
        if show_color_menu {
            self.draw_color_popup(pixmap, active_c);
        }

        // Draw Settings Menu Popup if active
        if show_settings_menu {
            self.draw_settings_popup(pixmap, monitor_mode, passthrough, bg_mode);
        }
    }

    fn draw_color_popup(&self, pixmap: &mut tiny_skia::Pixmap, active_c: Color) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let scale = self.scale_factor;
        let menu_x = self.x + 330.0 * scale;
        let menu_y = self.y + self.height + 6.0 * scale;
        let menu_w = 150.0 * scale;
        let menu_h = 110.0 * scale;

        let mut pb = PathBuilder::new();
        self.add_rounded_rect(&mut pb, menu_x, menu_y, menu_w, menu_h, 8.0 * scale);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(20, 22, 28, 245));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border = Paint::default();
            border.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 50));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &border, &stroke, Transform::identity(), None);
        }

        let colors = Self::palette_colors();
        for (i, color) in colors.iter().enumerate() {
            let row = i / 4;
            let col = i % 4;

            let cx = menu_x + (22.0 + (col as f32) * 34.0) * scale;
            let cy = menu_y + (22.0 + (row as f32) * 32.0) * scale;
            let radius = 11.0 * scale;

            let mut c_pb = PathBuilder::new();
            c_pb.push_circle(cx, cy, radius);
            if let Some(c_path) = c_pb.finish() {
                let mut c_paint = Paint::default();
                c_paint.set_color(tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a));
                c_paint.anti_alias = true;
                pixmap.fill_path(&c_path, &c_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

                let is_selected = color.r == active_c.r && color.g == active_c.g && color.b == active_c.b;
                let mut b_paint = Paint::default();
                let mut b_stroke = Stroke::default();
                if is_selected {
                    b_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
                    b_stroke.width = 2.5 * scale;
                } else {
                    b_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 80));
                    b_stroke.width = 1.0 * scale;
                }
                pixmap.stroke_path(&c_path, &b_paint, &b_stroke, Transform::identity(), None);
            }
        }
    }

    fn draw_settings_popup(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        monitor_mode: MonitorMode,
        passthrough: bool,
        bg_mode: BackgroundMode,
    ) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let scale = self.scale_factor;
        let menu_x = self.x + 400.0 * scale;
        let menu_y = self.y + self.height + 6.0 * scale;
        let menu_w = 240.0 * scale;
        let menu_h = 130.0 * scale;

        // Popup background
        let mut pb = PathBuilder::new();
        self.add_rounded_rect(&mut pb, menu_x, menu_y, menu_w, menu_h, 8.0 * scale);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(20, 22, 28, 245));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border = Paint::default();
            border.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 50));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &border, &stroke, Transform::identity(), None);
        }

        // Items in Settings Menu
        let items = [
            ("Display:", monitor_mode.label()),
            ("Click Pass:", if passthrough { "Enabled" } else { "Disabled" }),
            ("Background:", match bg_mode {
                BackgroundMode::Transparent => "Transparent",
                BackgroundMode::Blackboard => "Blackboard",
                BackgroundMode::Whiteboard => "Whiteboard",
            }),
        ];

        let mut item_y = menu_y + 8.0 * scale;
        for (label, val) in &items {
            let mut row_pb = PathBuilder::new();
            self.add_rounded_rect(&mut row_pb, menu_x + 6.0 * scale, item_y, menu_w - 12.0 * scale, 34.0 * scale, 6.0 * scale);
            if let Some(row_path) = row_pb.finish() {
                let mut row_paint = Paint::default();
                row_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 20));
                pixmap.fill_path(&row_path, &row_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            // Render indicator box for setting item
            let mut ind_pb = PathBuilder::new();
            self.add_rounded_rect(&mut ind_pb, menu_x + 12.0 * scale, item_y + 7.0 * scale, 18.0 * scale, 18.0 * scale, 4.0 * scale);
            if let Some(ind_path) = ind_pb.finish() {
                let mut ind_paint = Paint::default();
                ind_paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 220));
                pixmap.fill_path(&ind_path, &ind_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            // Render Label Text
            render_text_to_pixmap(
                label,
                menu_x + 38.0 * scale,
                item_y + 8.0 * scale,
                14.0 * scale,
                Color::new(220, 225, 235, 255),
                BlendMode::Normal,
                pixmap,
            );

            // Render Value Text
            render_text_to_pixmap(
                val,
                menu_x + 125.0 * scale,
                item_y + 8.0 * scale,
                14.0 * scale,
                Color::new(140, 200, 255, 255),
                BlendMode::Normal,
                pixmap,
            );



            item_y += 38.0 * scale;
        }
    }

    fn add_rounded_rect(&self, pb: &mut tiny_skia::PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolbar_hidpi_scaling() {
        let tb = Toolbar::new_with_scale(1920.0, 2.0);
        assert_eq!(tb.scale_factor, 2.0);
        assert_eq!(tb.width, 660.0 * 2.0);
        assert_eq!(tb.height, 38.0 * 2.0);
    }
}
