use crate::core::{Tool, Color, ShapeKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolbarAction {
    SelectTool(Tool),
    SelectShape(ShapeKind),
    SetColor(Color),
    Clear,
    TogglePassthrough,
    Exit,
}

pub struct Toolbar {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Toolbar {
    pub fn new(screen_width: f32) -> Self {
        let width = 686.0;
        let height = 48.0;
        let x = (screen_width - width) / 2.0;
        let y = 15.0;
        Self { x, y, width, height }
    }

    pub fn handle_click(&self, click_x: f32, click_y: f32) -> Option<ToolbarAction> {
        if click_x < self.x || click_x > self.x + self.width || click_y < self.y || click_y > self.y + self.height {
            return None;
        }

        let rx = click_x - self.x;

        // Tools
        if rx >= 10.0 && rx < 50.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_pen()));
        }
        if rx >= 54.0 && rx < 94.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_highlighter()));
        }
        if rx >= 98.0 && rx < 138.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Line));
        }
        if rx >= 142.0 && rx < 182.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Arrow));
        }
        if rx >= 186.0 && rx < 226.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Rectangle));
        }
        if rx >= 230.0 && rx < 270.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Oval));
        }
        if rx >= 274.0 && rx < 314.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_text()));
        }
        if rx >= 318.0 && rx < 358.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_eraser()));
        }

        // Color Swatches
        let colors = [
            Color::new(235, 50, 50, 255),   // Red
            Color::new(50, 200, 75, 255),   // Green
            Color::new(50, 130, 245, 255),  // Blue
            Color::new(245, 200, 30, 255),  // Yellow
            Color::new(255, 255, 255, 255), // White
            Color::new(30, 30, 30, 255),    // Black
        ];

        let mut swatch_x = 372.0;
        for color in &colors {
            if rx >= swatch_x && rx < swatch_x + 24.0 {
                return Some(ToolbarAction::SetColor(*color));
            }
            swatch_x += 28.0;
        }

        // Action Buttons
        if rx >= 550.0 && rx < 590.0 {
            return Some(ToolbarAction::Clear);
        }
        if rx >= 594.0 && rx < 636.0 {
            return Some(ToolbarAction::TogglePassthrough);
        }
        if rx >= 640.0 && rx < 676.0 {
            return Some(ToolbarAction::Exit);
        }

        None
    }

    pub fn draw(&self, pixmap: &mut tiny_skia::Pixmap, active_tool: Tool, passthrough: bool) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform, LineCap, LineJoin};

        // Draw Toolbar background
        let mut pb = PathBuilder::new();
        self.add_rounded_rect(&mut pb, self.x, self.y, self.width, self.height, 12.0);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(20, 20, 25, 230));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            border_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 45));
            border_paint.anti_alias = true;
            let mut stroke = Stroke::default();
            stroke.width = 1.0;
            pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
        }

        // Tool buttons configuration
        let tool_buttons = [
            ("Pen", 10.0, 40.0),
            ("High", 54.0, 40.0),
            ("Line", 98.0, 40.0),
            ("Arrow", 142.0, 40.0),
            ("Rect", 186.0, 40.0),
            ("Oval", 230.0, 40.0),
            ("Text", 274.0, 40.0),
            ("Eraser", 318.0, 40.0),
        ];

        for (name, bx, bw) in &tool_buttons {
            let is_active = match *name {
                "Pen" => matches!(active_tool, Tool::Pen { .. }),
                "High" => matches!(active_tool, Tool::Highlighter { .. }),
                "Line" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Line, .. }),
                "Arrow" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Arrow, .. }),
                "Rect" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Rectangle, .. }),
                "Oval" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Oval, .. }),
                "Text" => matches!(active_tool, Tool::Text { .. }),
                "Eraser" => matches!(active_tool, Tool::Eraser { .. }),
                _ => false,
            };

            let x = self.x + bx;
            let y = self.y + 6.0;
            let w = *bw;
            let h = 36.0;

            let mut btn_pb = PathBuilder::new();
            self.add_rounded_rect(&mut btn_pb, x, y, w, h, 8.0);
            if let Some(btn_path) = btn_pb.finish() {
                let mut paint = Paint::default();
                if is_active {
                    paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 200));
                } else {
                    paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 15));
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
            icon_stroke.width = 2.0;
            icon_stroke.line_cap = LineCap::Round;
            icon_stroke.line_join = LineJoin::Round;

            match *name {
                "Pen" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0, cy + 5.0);
                    ipb.line_to(cx + 5.0, cy - 7.0);
                    ipb.line_to(cx + 7.0, cy - 5.0);
                    ipb.line_to(cx - 5.0, cy + 7.0);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "High" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0, cy + 7.0);
                    ipb.line_to(cx + 5.0, cy - 5.0);
                    if let Some(ipath) = ipb.finish() {
                        let mut thick_stroke = icon_stroke.clone();
                        thick_stroke.width = 5.0;
                        pixmap.stroke_path(&ipath, &icon_paint, &thick_stroke, Transform::identity(), None);
                    }
                }
                "Line" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0, cy + 8.0);
                    ipb.line_to(cx + 8.0, cy - 8.0);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Arrow" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0, cy + 8.0);
                    ipb.line_to(cx + 8.0, cy - 8.0);
                    ipb.move_to(cx + 8.0, cy - 8.0);
                    ipb.line_to(cx + 1.0, cy - 8.0);
                    ipb.move_to(cx + 8.0, cy - 8.0);
                    ipb.line_to(cx + 8.0, cy - 1.0);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Rect" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0, cy - 7.0);
                    ipb.line_to(cx + 7.0, cy - 7.0);
                    ipb.line_to(cx + 7.0, cy + 7.0);
                    ipb.line_to(cx - 7.0, cy + 7.0);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Oval" => {
                    let mut ipb = PathBuilder::new();
                    let rx = 7.0;
                    let ry = 7.0;
                    let kappa = 0.55228475;
                    let ox = rx * kappa;
                    let oy = ry * kappa;
                    ipb.move_to(cx - rx, cy);
                    ipb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
                    ipb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
                    ipb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
                    ipb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Text" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0, cy - 7.0);
                    ipb.line_to(cx + 7.0, cy - 7.0);
                    ipb.move_to(cx, cy - 7.0);
                    ipb.line_to(cx, cy + 7.0);
                    if let Some(ipath) = ipb.finish() {
                        let mut bold_stroke = icon_stroke.clone();
                        bold_stroke.width = 2.5;
                        pixmap.stroke_path(&ipath, &icon_paint, &bold_stroke, Transform::identity(), None);
                    }
                }
                "Eraser" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0, cy + 4.0);
                    ipb.line_to(cx - 2.0, cy - 6.0);
                    ipb.line_to(cx + 8.0, cy - 6.0);
                    ipb.line_to(cx + 2.0, cy + 4.0);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                _ => {}
            }
        }

        // Draw Divider 1
        let mut div1 = PathBuilder::new();
        div1.move_to(self.x + 364.0, self.y + 10.0);
        div1.line_to(self.x + 364.0, self.y + 38.0);
        if let Some(path) = div1.finish() {
            let mut div_paint = Paint::default();
            div_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 40));
            let mut stroke = Stroke::default();
            stroke.width = 1.0;
            pixmap.stroke_path(&path, &div_paint, &stroke, Transform::identity(), None);
        }

        // Draw Color Swatches
        let colors = [
            Color::new(235, 50, 50, 255),
            Color::new(50, 200, 75, 255),
            Color::new(50, 130, 245, 255),
            Color::new(245, 200, 30, 255),
            Color::new(255, 255, 255, 255),
            Color::new(30, 30, 30, 255),
        ];

        let active_color = active_tool.color();

        let mut swatch_x = self.x + 372.0;
        for c in &colors {
            let cx = swatch_x + 12.0;
            let cy = self.y + 24.0;
            let r = 9.0;

            let mut cpb = PathBuilder::new();
            let kappa = 0.55228475;
            let ox = r * kappa;
            let oy = r * kappa;
            cpb.move_to(cx - r, cy);
            cpb.cubic_to(cx - r, cy - oy, cx - ox, cy - r, cx, cy - r);
            cpb.cubic_to(cx + ox, cy - r, cx + r, cy - oy, cx + r, cy);
            cpb.cubic_to(cx + r, cy + oy, cx + ox, cy + r, cx, cy + r);
            cpb.cubic_to(cx - ox, cy + r, cx - r, cy + oy, cx - r, cy);
            cpb.close();

            if let Some(cpath) = cpb.finish() {
                let mut cpaint = Paint::default();
                cpaint.set_color(tiny_skia::Color::from_rgba8(c.r, c.g, c.b, c.a));
                cpaint.anti_alias = true;
                pixmap.fill_path(&cpath, &cpaint, tiny_skia::FillRule::Winding, Transform::identity(), None);

                if let Some(ac) = active_color {
                    if ac.r == c.r && ac.g == c.g && ac.b == c.b {
                        let mut border_p = Paint::default();
                        border_p.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 220));
                        border_p.anti_alias = true;
                        let mut bstroke = Stroke::default();
                        bstroke.width = 2.0;
                        pixmap.stroke_path(&cpath, &border_p, &bstroke, Transform::identity(), None);
                    }
                }
            }

            swatch_x += 28.0;
        }

        // Draw Divider 2
        let mut div2 = PathBuilder::new();
        div2.move_to(self.x + 542.0, self.y + 10.0);
        div2.line_to(self.x + 542.0, self.y + 38.0);
        if let Some(path) = div2.finish() {
            let mut div_paint = Paint::default();
            div_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 40));
            let mut stroke = Stroke::default();
            stroke.width = 1.0;
            pixmap.stroke_path(&path, &div_paint, &stroke, Transform::identity(), None);
        }

        // Draw Action Buttons (Clear, Pass, Exit)
        let action_buttons = [
            ("Clear", 550.0, 40.0),
            ("Pass", 594.0, 42.0),
            ("Exit", 640.0, 36.0),
        ];

        for (name, bx, bw) in &action_buttons {
            let is_active = match *name {
                "Pass" => passthrough,
                _ => false,
            };

            let x = self.x + bx;
            let y = self.y + 6.0;
            let w = *bw;
            let h = 36.0;

            let mut btn_pb = PathBuilder::new();
            self.add_rounded_rect(&mut btn_pb, x, y, w, h, 8.0);
            if let Some(btn_path) = btn_pb.finish() {
                let mut paint = Paint::default();
                if is_active {
                    paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 200));
                } else {
                    paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 15));
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
            icon_stroke.width = 2.0;
            icon_stroke.line_cap = LineCap::Round;
            icon_stroke.line_join = LineJoin::Round;

            match *name {
                "Clear" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0, cy - 6.0);
                    ipb.line_to(cx + 8.0, cy - 6.0);
                    ipb.move_to(cx - 6.0, cy - 4.0);
                    ipb.line_to(cx - 4.0, cy + 8.0);
                    ipb.line_to(cx + 4.0, cy + 8.0);
                    ipb.line_to(cx + 6.0, cy - 4.0);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Pass" => {
                    let mut ipb = PathBuilder::new();
                    if passthrough {
                        ipb.move_to(cx - 5.0, cy - 7.0);
                        ipb.line_to(cx - 5.0, cy + 7.0);
                        ipb.line_to(cx - 1.0, cy + 2.0);
                        ipb.line_to(cx + 4.0, cy + 7.0);
                        ipb.line_to(cx + 6.0, cy + 5.0);
                        ipb.line_to(cx + 1.0, cy + 0.0);
                        ipb.line_to(cx + 5.0, cy - 2.0);
                        ipb.close();
                    } else {
                        ipb.move_to(cx - 7.0, cy - 7.0);
                        ipb.line_to(cx + 7.0, cy + 7.0);
                        ipb.move_to(cx - 8.0, cy);
                        ipb.quad_to(cx, cy - 5.0, cx + 8.0, cy);
                        ipb.quad_to(cx, cy + 5.0, cx - 8.0, cy);
                    }
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Exit" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0, cy - 6.0);
                    ipb.line_to(cx + 6.0, cy + 6.0);
                    ipb.move_to(cx + 6.0, cy - 6.0);
                    ipb.line_to(cx - 6.0, cy + 6.0);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                _ => {}
            }
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
