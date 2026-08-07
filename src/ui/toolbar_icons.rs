/// Renders all toolbar tool icons onto the pixmap.
/// Each icon is drawn relative to its center point (cx, cy) at the given scale.
pub fn draw_tool_icon(
    name: &str,
    cx: f32,
    cy: f32,
    scale: f32,
    has_crop_selection: bool,
    pixmap: &mut tiny_skia::Pixmap,
) {
    use tiny_skia::{PathBuilder, Paint, Stroke, Transform, LineCap, LineJoin};

    let mut icon_paint = Paint::default();
    icon_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 230));
    icon_paint.anti_alias = true;

    let mut icon_stroke = Stroke::default();
    icon_stroke.width = 1.8 * scale;
    icon_stroke.line_cap = LineCap::Round;
    icon_stroke.line_join = LineJoin::Round;

    match name {
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
            ipb.line_to(cx + 6.0 * scale, cy);
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
            // Neon glowing laser dot with outer pulse ring
            let mut ipb = PathBuilder::new();
            ipb.push_circle(cx, cy, 3.5 * scale);
            if let Some(ipath) = ipb.finish() {
                let mut lpaint = Paint::default();
                lpaint.set_color(tiny_skia::Color::from_rgba8(255, 60, 60, 255));
                lpaint.anti_alias = true;
                pixmap.fill_path(&ipath, &lpaint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }
            let mut rpb = PathBuilder::new();
            rpb.push_circle(cx, cy, 6.0 * scale);
            if let Some(rpath) = rpb.finish() {
                let mut rstroke = icon_stroke.clone();
                rstroke.width = 1.2 * scale;
                pixmap.stroke_path(&rpath, &icon_paint, &rstroke, Transform::identity(), None);
            }
        }
        "Spotlight" => {
            // Magnifying glass / Spotlight search motif
            let mut ipb = PathBuilder::new();
            ipb.push_circle(cx - 1.5 * scale, cy - 1.5 * scale, 4.5 * scale);
            ipb.move_to(cx + 1.8 * scale, cy + 1.8 * scale);
            ipb.line_to(cx + 6.0 * scale, cy + 6.0 * scale);
            if let Some(ipath) = ipb.finish() {
                pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
            }
        }
        "Eraser" => {
            // Detailed angled 3D Eraser block with cap divider line
            let mut ipb = PathBuilder::new();
            ipb.move_to(cx - 5.5 * scale, cy + 2.5 * scale);
            ipb.line_to(cx + 0.5 * scale, cy - 5.5 * scale);
            ipb.line_to(cx + 5.5 * scale, cy - 1.5 * scale);
            ipb.line_to(cx - 0.5 * scale, cy + 6.5 * scale);
            ipb.close();
            // Eraser felt cap dividing line
            ipb.move_to(cx - 2.5 * scale, cy - 1.5 * scale);
            ipb.line_to(cx + 2.5 * scale, cy + 2.5 * scale);
            if let Some(ipath) = ipb.finish() {
                pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
            }
        }
        "Crop" => {
            if has_crop_selection {
                // Checkmark (confirm crop)
                let mut ipb = PathBuilder::new();
                ipb.move_to(cx - 5.0 * scale, cy + 0.5 * scale);
                ipb.line_to(cx - 1.5 * scale, cy + 4.5 * scale);
                ipb.line_to(cx + 5.5 * scale, cy - 4.5 * scale);
                if let Some(ipath) = ipb.finish() {
                    let mut check_stroke = icon_stroke.clone();
                    check_stroke.width = 2.4 * scale;
                    pixmap.stroke_path(&ipath, &icon_paint, &check_stroke, Transform::identity(), None);
                }
            } else {
                // Corner brackets
                let mut ipb = PathBuilder::new();
                let s = 5.0 * scale;
                ipb.move_to(cx - s, cy - s + 3.0 * scale);
                ipb.line_to(cx - s, cy - s);
                ipb.line_to(cx - s + 3.0 * scale, cy - s);
                ipb.move_to(cx + s - 3.0 * scale, cy - s);
                ipb.line_to(cx + s, cy - s);
                ipb.line_to(cx + s, cy - s + 3.0 * scale);
                ipb.move_to(cx - s, cy + s - 3.0 * scale);
                ipb.line_to(cx - s, cy + s);
                ipb.line_to(cx - s + 3.0 * scale, cy + s);
                ipb.move_to(cx + s - 3.0 * scale, cy + s);
                ipb.line_to(cx + s, cy + s);
                ipb.line_to(cx + s, cy + s - 3.0 * scale);
                if let Some(ipath) = ipb.finish() {
                    pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
                }
            }
        }
        _ => {}
    }
}

/// Renders an action button icon (Save, Board, Clear, Pass, Settings, Tray, Exit).
pub fn draw_action_icon(
    name: &str,
    cx: f32,
    cy: f32,
    scale: f32,
    pixmap: &mut tiny_skia::Pixmap,
) {
    use tiny_skia::{PathBuilder, Paint, Stroke, Transform, LineCap, LineJoin};

    let mut icon_paint = Paint::default();
    icon_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 230));
    icon_paint.anti_alias = true;

    let mut icon_stroke = Stroke::default();
    icon_stroke.width = 1.8 * scale;
    icon_stroke.line_cap = LineCap::Round;
    icon_stroke.line_join = LineJoin::Round;

    match name {
        "Save" => {
            let mut ipb = PathBuilder::new();
            ipb.move_to(cx - 5.0 * scale, cy - 5.0 * scale);
            ipb.line_to(cx + 3.0 * scale, cy - 5.0 * scale);
            ipb.line_to(cx + 5.0 * scale, cy - 3.0 * scale);
            ipb.line_to(cx + 5.0 * scale, cy + 5.0 * scale);
            ipb.line_to(cx - 5.0 * scale, cy + 5.0 * scale);
            ipb.close();
            ipb.move_to(cx - 3.0 * scale, cy + 5.0 * scale);
            ipb.line_to(cx - 3.0 * scale, cy + 1.0 * scale);
            ipb.line_to(cx + 3.0 * scale, cy + 1.0 * scale);
            ipb.line_to(cx + 3.0 * scale, cy + 5.0 * scale);
            if let Some(ipath) = ipb.finish() {
                pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
            }
        }
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
            ipb.move_to(cx - 4.0 * scale, cy - 5.0 * scale);
            ipb.line_to(cx + 2.0 * scale, cy + 1.0 * scale);
            ipb.line_to(cx - 1.0 * scale, cy + 1.0 * scale);
            ipb.line_to(cx + 3.0 * scale, cy + 6.0 * scale);
            if let Some(ipath) = ipb.finish() {
                pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
            }
        }
        "Settings" => {
            let mut ipb = PathBuilder::new();
            let teeth = 6;
            let r_inner = 3.2 * scale;
            let r_outer = 6.0 * scale;
            
            for i in 0..(teeth * 2) {
                let angle = (i as f32) * std::f32::consts::PI / (teeth as f32);
                let r = if i % 2 == 0 { r_outer } else { r_inner };
                let x = cx + r * angle.cos();
                let y = cy + r * angle.sin();
                if i == 0 {
                    ipb.move_to(x, y);
                } else {
                    ipb.line_to(x, y);
                }
            }
            ipb.close();
            ipb.push_circle(cx, cy, 2.0 * scale);
            if let Some(ipath) = ipb.finish() {
                pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
            }
        }
        "Tray" => {
            let mut ipb = PathBuilder::new();
            ipb.move_to(cx - 5.0 * scale, cy + 4.0 * scale);
            ipb.line_to(cx + 5.0 * scale, cy + 4.0 * scale);
            if let Some(ipath) = ipb.finish() {
                pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
            }
        }
        "Exit" => {
            let mut ipb = PathBuilder::new();
            ipb.move_to(cx - 4.0 * scale, cy - 4.0 * scale);
            ipb.line_to(cx + 4.0 * scale, cy + 4.0 * scale);
            ipb.move_to(cx + 4.0 * scale, cy - 4.0 * scale);
            ipb.line_to(cx - 4.0 * scale, cy + 4.0 * scale);
            if let Some(ipath) = ipb.finish() {
                pixmap.stroke_path(&ipath, &icon_paint, &icon_stroke, Transform::identity(), None);
            }
        }
        _ => {}
    }
}
