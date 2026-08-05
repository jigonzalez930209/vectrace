use crate::core::{Tool, Color, ShapeKind, BackgroundMode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolbarAction {
    SelectTool(Tool),
    SelectShape(ShapeKind),
    SetColor(Color),
    ToggleBackgroundMode,
    Clear,
    TogglePassthrough,
    Exit,
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
        let width = 740.0 * scale;
        let height = 48.0 * scale;
        let x = (screen_width - width) / 2.0;
        let y = 15.0 * scale;
        Self { x, y, width, height, scale_factor: scale }
    }

    pub fn handle_click(&self, click_x: f32, click_y: f32) -> Option<ToolbarAction> {
        if click_x < self.x || click_x > self.x + self.width || click_y < self.y || click_y > self.y + self.height {
            return None;
        }

        let rx = (click_x - self.x) / self.scale_factor;

        // Tools
        if rx >= 10.0 && rx < 48.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_pen()));
        }
        if rx >= 52.0 && rx < 90.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_highlighter()));
        }
        if rx >= 94.0 && rx < 132.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Line));
        }
        if rx >= 136.0 && rx < 174.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Arrow));
        }
        if rx >= 178.0 && rx < 216.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Rectangle));
        }
        if rx >= 220.0 && rx < 258.0 {
            return Some(ToolbarAction::SelectShape(ShapeKind::Oval));
        }
        if rx >= 262.0 && rx < 300.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_laser()));
        }
        if rx >= 304.0 && rx < 342.0 {
            return Some(ToolbarAction::SelectTool(Tool::default_spotlight()));
        }
        if rx >= 346.0 && rx < 384.0 {
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

        let mut swatch_x = 398.0;
        for color in &colors {
            if rx >= swatch_x && rx < swatch_x + 24.0 {
                return Some(ToolbarAction::SetColor(*color));
            }
            swatch_x += 28.0;
        }

        // Action Buttons
        if rx >= 578.0 && rx < 618.0 {
            return Some(ToolbarAction::ToggleBackgroundMode);
        }
        if rx >= 622.0 && rx < 660.0 {
            return Some(ToolbarAction::Clear);
        }
        if rx >= 664.0 && rx < 702.0 {
            return Some(ToolbarAction::TogglePassthrough);
        }
        if rx >= 706.0 && rx < 734.0 {
            return Some(ToolbarAction::Exit);
        }

        None
    }

    pub fn draw(&self, pixmap: &mut tiny_skia::Pixmap, active_tool: Tool, passthrough: bool, bg_mode: BackgroundMode) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform, LineCap, LineJoin};

        let scale = self.scale_factor;

        // Draw Toolbar background
        let mut pb = PathBuilder::new();
        self.add_rounded_rect(&mut pb, self.x, self.y, self.width, self.height, 12.0 * scale);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(20, 20, 25, 235));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            border_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 45));
            border_paint.anti_alias = true;
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
        }

        // Tool buttons configuration
        let tool_buttons = [
            ("Pen", 10.0, 38.0),
            ("High", 52.0, 38.0),
            ("Line", 94.0, 38.0),
            ("Arrow", 136.0, 38.0),
            ("Rect", 178.0, 38.0),
            ("Oval", 220.0, 38.0),
            ("Laser", 262.0, 38.0),
            ("Spotlight", 304.0, 38.0),
            ("Eraser", 346.0, 38.0),
        ];

        for (name, bx, bw) in &tool_buttons {
            let is_active = match *name {
                "Pen" => matches!(active_tool, Tool::Pen { .. }),
                "High" => matches!(active_tool, Tool::Highlighter { .. }),
                "Line" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Line, .. }),
                "Arrow" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Arrow, .. }),
                "Rect" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Rectangle, .. }),
                "Oval" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Oval, .. }),
                "Laser" => matches!(active_tool, Tool::Laser { .. }),
                "Spotlight" => matches!(active_tool, Tool::Spotlight { .. }),
                "Eraser" => matches!(active_tool, Tool::Eraser { .. }),
                _ => false,
            };

            let x = self.x + bx * scale;
            let y = self.y + 6.0 * scale;
            let w = *bw * scale;
            let h = 36.0 * scale;

            let mut btn_pb = PathBuilder::new();
            self.add_rounded_rect(&mut btn_pb, x, y, w, h, 8.0 * scale);
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
            icon_stroke.width = 2.0 * scale;
            icon_stroke.line_cap = LineCap::Round;
            icon_stroke.line_join = LineJoin::Round;

            match *name {
                "Pen" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0 * scale, cy + 5.0 * scale);
                    ipb.line_to(cx + 5.0 * scale, cy - 7.0 * scale);
                    ipb.line_to(cx + 7.0 * scale, cy - 5.0 * scale);
                    ipb.line_to(cx - 5.0 * scale, cy + 7.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "High" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0 * scale, cy + 7.0 * scale);
                    ipb.line_to(cx + 5.0 * scale, cy - 5.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        let mut thick_stroke = icon_stroke.clone();
                        thick_stroke.width = 5.0 * scale;
                        pixmap.stroke_path(&ipath, &icon_paint, &thick_stroke, Transform::identity(), None);
                    }
                }
                "Line" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0 * scale, cy + 8.0 * scale);
                    ipb.line_to(cx + 8.0 * scale, cy - 8.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Arrow" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0 * scale, cy + 8.0 * scale);
                    ipb.line_to(cx + 8.0 * scale, cy - 8.0 * scale);
                    ipb.move_to(cx + 8.0 * scale, cy - 8.0 * scale);
                    ipb.line_to(cx + 1.0 * scale, cy - 8.0 * scale);
                    ipb.move_to(cx + 8.0 * scale, cy - 8.0 * scale);
                    ipb.line_to(cx + 8.0 * scale, cy - 1.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Rect" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0 * scale, cy - 7.0 * scale);
                    ipb.line_to(cx + 7.0 * scale, cy - 7.0 * scale);
                    ipb.line_to(cx + 7.0 * scale, cy + 7.0 * scale);
                    ipb.line_to(cx - 7.0 * scale, cy + 7.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Oval" => {
                    let mut ipb = PathBuilder::new();
                    let rx = 7.0 * scale;
                    let ry = 7.0 * scale;
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
                "Laser" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy + 6.0 * scale);
                    ipb.line_to(cx + 4.0 * scale, cy - 4.0 * scale);
                    ipb.move_to(cx + 4.0 * scale, cy - 4.0 * scale);
                    ipb.line_to(cx + 8.0 * scale, cy - 8.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        let mut laser_paint = Paint::default();
                        laser_paint.set_color(tiny_skia::Color::from_rgba8(255, 50, 120, 255));
                        pixmap.stroke_path(&ipath, &laser_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Spotlight" => {
                    let mut ipb = PathBuilder::new();
                    let r = 5.0 * scale;
                    let lcx = cx - 2.0 * scale;
                    let lcy = cy - 2.0 * scale;
                    let kappa = 0.55228475;
                    let ox = r * kappa;
                    let oy = r * kappa;
                    ipb.move_to(lcx - r, lcy);
                    ipb.cubic_to(lcx - r, lcy - oy, lcx - ox, lcy - r, lcx, lcy - r);
                    ipb.cubic_to(lcx + ox, lcy - r, lcx + r, lcy - oy, lcx + r, lcy);
                    ipb.cubic_to(lcx + r, lcy + oy, lcx + ox, lcy + r, lcx, lcy + r);
                    ipb.cubic_to(lcx - ox, lcy + r, lcx - r, lcy + oy, lcx - r, lcy);
                    ipb.close();

                    ipb.move_to(lcx + 3.5 * scale, lcy + 3.5 * scale);
                    ipb.line_to(cx + 7.0 * scale, cy + 7.0 * scale);

                    if let Some(ipath) = ipb.finish() {
                        let mut loupe_stroke = icon_stroke.clone();
                        loupe_stroke.width = 2.2 * scale;
                        pixmap.stroke_path(&ipath, &icon_paint, &loupe_stroke, Transform::identity(), None);
                    }
                }
                "Eraser" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0 * scale, cy + 4.0 * scale);
                    ipb.line_to(cx - 2.0 * scale, cy - 6.0 * scale);
                    ipb.line_to(cx + 8.0 * scale, cy - 6.0 * scale);
                    ipb.line_to(cx + 2.0 * scale, cy + 4.0 * scale);
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
        div1.move_to(self.x + 390.0 * scale, self.y + 10.0 * scale);
        div1.line_to(self.x + 390.0 * scale, self.y + 38.0 * scale);
        if let Some(path) = div1.finish() {
            let mut div_paint = Paint::default();
            div_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 40));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
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

        let mut swatch_x = self.x + 398.0 * scale;
        for c in &colors {
            let cx = swatch_x + 12.0 * scale;
            let cy = self.y + 24.0 * scale;
            let r = 9.0 * scale;

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
                        bstroke.width = 2.0 * scale;
                        pixmap.stroke_path(&cpath, &border_p, &bstroke, Transform::identity(), None);
                    }
                }
            }

            swatch_x += 28.0 * scale;
        }

        // Draw Divider 2
        let mut div2 = PathBuilder::new();
        div2.move_to(self.x + 570.0 * scale, self.y + 10.0 * scale);
        div2.line_to(self.x + 570.0 * scale, self.y + 38.0 * scale);
        if let Some(path) = div2.finish() {
            let mut div_paint = Paint::default();
            div_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 40));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &div_paint, &stroke, Transform::identity(), None);
        }

        // Draw Action Buttons (Board, Clear, Pass, Exit)
        let action_buttons = [
            ("Board", 578.0, 40.0),
            ("Clear", 622.0, 38.0),
            ("Pass", 664.0, 38.0),
            ("Exit", 706.0, 28.0),
        ];

        for (name, bx, bw) in &action_buttons {
            let is_active = match *name {
                "Pass" => passthrough,
                "Board" => bg_mode != BackgroundMode::Transparent,
                _ => false,
            };

            let x = self.x + bx * scale;
            let y = self.y + 6.0 * scale;
            let w = *bw * scale;
            let h = 36.0 * scale;

            let mut btn_pb = PathBuilder::new();
            self.add_rounded_rect(&mut btn_pb, x, y, w, h, 8.0 * scale);
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
            icon_stroke.width = 2.0 * scale;
            icon_stroke.line_cap = LineCap::Round;
            icon_stroke.line_join = LineJoin::Round;

            match *name {
                "Board" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 7.0 * scale, cy - 6.0 * scale);
                    ipb.line_to(cx + 7.0 * scale, cy - 6.0 * scale);
                    ipb.line_to(cx + 7.0 * scale, cy + 6.0 * scale);
                    ipb.line_to(cx - 7.0 * scale, cy + 6.0 * scale);
                    ipb.close();
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Clear" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 8.0 * scale, cy - 6.0 * scale);
                    ipb.line_to(cx + 8.0 * scale, cy - 6.0 * scale);
                    ipb.move_to(cx - 6.0 * scale, cy - 4.0 * scale);
                    ipb.line_to(cx - 4.0 * scale, cy + 8.0 * scale);
                    ipb.line_to(cx + 4.0 * scale, cy + 8.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy - 4.0 * scale);
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Pass" => {
                    let mut ipb = PathBuilder::new();
                    if passthrough {
                        ipb.move_to(cx - 5.0 * scale, cy - 7.0 * scale);
                        ipb.line_to(cx - 5.0 * scale, cy + 7.0 * scale);
                        ipb.line_to(cx - 1.0 * scale, cy + 2.0 * scale);
                        ipb.line_to(cx + 4.0 * scale, cy + 7.0 * scale);
                        ipb.line_to(cx + 6.0 * scale, cy + 5.0 * scale);
                        ipb.line_to(cx + 1.0 * scale, cy + 0.0 * scale);
                        ipb.line_to(cx + 5.0 * scale, cy - 2.0 * scale);
                        ipb.close();
                    } else {
                        ipb.move_to(cx - 7.0 * scale, cy - 7.0 * scale);
                        ipb.line_to(cx + 7.0 * scale, cy + 7.0 * scale);
                        ipb.move_to(cx - 8.0 * scale, cy);
                        ipb.quad_to(cx, cy - 5.0 * scale, cx + 8.0 * scale, cy);
                        ipb.quad_to(cx, cy + 5.0 * scale, cx - 8.0 * scale, cy);
                    }
                    if let Some(ipath) = ipb.finish() {
                        pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                    }
                }
                "Exit" => {
                    let mut ipb = PathBuilder::new();
                    ipb.move_to(cx - 6.0 * scale, cy - 6.0 * scale);
                    ipb.line_to(cx + 6.0 * scale, cy + 6.0 * scale);
                    ipb.move_to(cx + 6.0 * scale, cy - 6.0 * scale);
                    ipb.line_to(cx - 6.0 * scale, cy + 6.0 * scale);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolbar_hidpi_scaling() {
        let tb = Toolbar::new_with_scale(1920.0, 2.0);
        assert_eq!(tb.scale_factor, 2.0);
        assert_eq!(tb.width, 740.0 * 2.0);
        assert_eq!(tb.height, 48.0 * 2.0);
    }
}
