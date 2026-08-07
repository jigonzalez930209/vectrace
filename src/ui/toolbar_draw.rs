use crate::core::{Tool, Color, BlendMode, BackgroundMode, MonitorMode, render_text_to_pixmap};
use crate::ui::toolbar::{Toolbar, ToolbarLayout, layout, BAR_H, BTN_H, TOOL_BTN, ACTION_BTN_W, COLOR_BTN_W};
use crate::ui::toolbar_icons::{draw_tool_icon, draw_action_icon};

impl Toolbar {
    pub fn draw(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        active_tool: Tool,
        passthrough: bool,
        bg_mode: BackgroundMode,
        show_settings_menu: bool,
        show_color_menu: bool,
        monitor_mode: MonitorMode,
        has_crop_selection: bool,
        hover_tooltip: Option<(&str, (f32, f32))>,
    ) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let scale = self.scale_factor;
        let lay = layout();

        // Toolbar background
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

        // Drag grip dots (6-dot matrix centered in 24px grip area)
        let mut grip_pb = PathBuilder::new();
        let grip_center_x = self.x + (lay.grip_x + 12.0) * scale;
        let grip_center_y = self.y + (BAR_H / 2.0) * scale;
        let dot_r = 1.3 * scale;
        let col_gap = 5.0 * scale;
        let row_gap = 5.0 * scale;
        let gx_start = grip_center_x - (col_gap / 2.0);
        let gy_start = grip_center_y - row_gap;

        for col in 0..2 {
            for row in 0..3 {
                grip_pb.push_circle(
                    gx_start + col as f32 * col_gap,
                    gy_start + row as f32 * row_gap,
                    dot_r,
                );
            }
        }
        if let Some(path) = grip_pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 130));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        }

        // Section dividers
        let mut div_paint = Paint::default();
        div_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 30));
        for dx in &lay.dividers {
            let mut pb = PathBuilder::new();
            pb.move_to(self.x + dx * scale, self.y + 6.0 * scale);
            pb.line_to(self.x + dx * scale, self.y + self.height - 6.0 * scale);
            if let Some(path) = pb.finish() {
                let mut stroke = Stroke::default();
                stroke.width = 1.0 * scale;
                pixmap.stroke_path(&path, &div_paint, &stroke, Transform::identity(), None);
            }
        }

        // Tool buttons
        let tools = [
            "Pen", "Highlighter", "Line", "Arrow", "Rectangle",
            "Oval", "Laser", "Spotlight", "Eraser", "Crop",
        ];
        for (i, name) in tools.iter().enumerate() {
            let is_active = is_tool_active(*name, active_tool);
            let x = self.x + lay.tool_xs[i] * scale;
            let y = self.y + 4.0 * scale;
            let w = TOOL_BTN * scale;
            let h = BTN_H * scale;

            let mut btn_pb = PathBuilder::new();
            self.add_rounded_rect(&mut btn_pb, x, y, w, h, 6.0 * scale);
            if let Some(btn_path) = btn_pb.finish() {
                let mut paint = Paint::default();
                if *name == "Crop" && has_crop_selection {
                    paint.set_color(tiny_skia::Color::from_rgba8(35, 185, 110, 240));
                } else if is_active {
                    paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 220));
                } else {
                    paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 12));
                }
                paint.anti_alias = true;
                pixmap.fill_path(&btn_path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            draw_tool_icon(name, x + w / 2.0, y + h / 2.0, scale, has_crop_selection, pixmap);
        }

        // Color button
        self.draw_color_button(pixmap, &lay, active_tool, show_color_menu);

        // Action buttons
        let action_buttons = ["Save", "Board", "Clear", "Pass", "Settings", "Tray", "Exit"];
        for (i, name) in action_buttons.iter().enumerate() {
            let is_active = match *name {
                "Pass"     => passthrough,
                "Board"    => bg_mode != BackgroundMode::Transparent,
                "Settings" => show_settings_menu,
                _          => false,
            };

            let x = self.x + lay.action_xs[i] * scale;
            let y = self.y + 4.0 * scale;
            let w = ACTION_BTN_W * scale;
            let h = BTN_H * scale;

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

            draw_action_icon(name, x + w / 2.0, y + h / 2.0, scale, pixmap);
        }

        if show_color_menu {
            self.draw_color_menu(pixmap);
        }
        if show_settings_menu {
            self.draw_settings_menu(pixmap, passthrough, bg_mode, monitor_mode);
        }

        if let Some((text, (tx, ty))) = hover_tooltip {
            let font_size = (12.0 * scale).round().max(1.0);
            let padding_x = 10.0 * scale;
            let padding_y = 6.0 * scale;

            // Calculate exact text pixel width using system font metrics
            let text_w = if let Some(font) = crate::core::render::get_system_font() {
                text.chars().map(|ch| font.rasterize(ch, font_size).0.advance_width).sum::<f32>()
            } else {
                text.len() as f32 * (font_size * 0.6)
            };

            let box_w = (text_w + padding_x * 2.0).round();
            let box_h = (font_size + padding_y * 2.0).round();
            let box_x = (tx - box_w / 2.0).round();
            let box_y = ty.round();

            let mut tbpb = PathBuilder::new();
            self.add_rounded_rect(&mut tbpb, box_x, box_y, box_w, box_h, 6.0 * scale);
            if let Some(tbpath) = tbpb.finish() {
                let mut tbpaint = Paint::default();
                tbpaint.set_color(tiny_skia::Color::from_rgba8(15, 18, 24, 245));
                tbpaint.anti_alias = true;
                pixmap.fill_path(&tbpath, &tbpaint, tiny_skia::FillRule::Winding, Transform::identity(), None);

                let mut stroke_paint = Paint::default();
                stroke_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 45));
                let mut stroke = Stroke::default();
                stroke.width = 1.0 * scale;
                pixmap.stroke_path(&tbpath, &stroke_paint, &stroke, Transform::identity(), None);
            }

            let text_x = (box_x + padding_x).round();
            let text_y = (box_y + padding_y).round();

            render_text_to_pixmap(
                text,
                text_x,
                text_y,
                font_size,
                Color::new(235, 240, 250, 255),
                BlendMode::Normal,
                pixmap,
            );
        }
    }

    fn draw_color_button(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        lay: &ToolbarLayout,
        active_tool: Tool,
        show_color_menu: bool,
    ) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform, LineCap, LineJoin};

        let scale = self.scale_factor;
        let color_btn_x = self.x + lay.color_x * scale;
        let color_btn_y = self.y + 4.0 * scale;
        let color_btn_w = COLOR_BTN_W * scale;
        let color_btn_h = BTN_H * scale;

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

        let active_c = active_tool.color().unwrap_or(Color::new(235, 50, 50, 255));
        let circle_cx = color_btn_x + 16.0 * scale;
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

        // Dropdown arrow
        let mut arr_pb = PathBuilder::new();
        let arr_cx = color_btn_x + 32.0 * scale;
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
    }

    pub fn draw_color_menu(&self, pixmap: &mut tiny_skia::Pixmap) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let scale = self.scale_factor;
        let lay = layout();
        let menu_x = self.x + lay.color_menu_x() * scale;
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

            let mut border_paint = Paint::default();
            border_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 40));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
        }

        let colors = Toolbar::palette_colors();
        for (i, color) in colors.iter().enumerate() {
            let row = (i / 4) as f32;
            let col = (i % 4) as f32;
            let cx = menu_x + 22.0 * scale + col * 34.0 * scale;
            let cy = menu_y + 22.0 * scale + row * 32.0 * scale;

            let mut dot_pb = PathBuilder::new();
            dot_pb.push_circle(cx, cy, 11.0 * scale);
            if let Some(dot_path) = dot_pb.finish() {
                let mut cpaint = Paint::default();
                cpaint.set_color(tiny_skia::Color::from_rgba8(color.r, color.g, color.b, color.a));
                cpaint.anti_alias = true;
                pixmap.fill_path(&dot_path, &cpaint, tiny_skia::FillRule::Winding, Transform::identity(), None);

                let mut stroke = Stroke::default();
                stroke.width = 1.0 * scale;
                let mut bpaint = Paint::default();
                bpaint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 120));
                pixmap.stroke_path(&dot_path, &bpaint, &stroke, Transform::identity(), None);
            }
        }
    }

    pub fn draw_settings_menu(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        passthrough: bool,
        bg_mode: BackgroundMode,
        monitor_mode: MonitorMode,
    ) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let scale = self.scale_factor;
        let lay = layout();
        let menu_x = self.x + lay.settings_menu_x() * scale;
        let menu_y = self.y + self.height + 6.0 * scale;
        let menu_w = 260.0 * scale;
        let menu_h = 130.0 * scale;

        let mut pb = PathBuilder::new();
        self.add_rounded_rect(&mut pb, menu_x, menu_y, menu_w, menu_h, 8.0 * scale);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(20, 22, 28, 245));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            border_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 40));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
        }

        let items = [
            ("Display Mode", match monitor_mode { MonitorMode::Primary => "Primary", MonitorMode::All => "All" }),
            ("Click-Through", if passthrough { "ON" } else { "OFF" }),
            ("Background", match bg_mode {
                BackgroundMode::Transparent => "Clear",
                BackgroundMode::Blackboard  => "Black",
                BackgroundMode::Whiteboard  => "White",
            }),
        ];

        let mut item_y = (menu_y + 8.0 * scale).round();
        let font_size = (13.0 * scale).round().max(1.0);

        for (label, val) in &items {
            let mut row_pb = PathBuilder::new();
            self.add_rounded_rect(&mut row_pb, menu_x + 6.0 * scale, item_y, menu_w - 12.0 * scale, 34.0 * scale, 6.0 * scale);
            if let Some(row_path) = row_pb.finish() {
                let mut row_paint = Paint::default();
                row_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 10));
                row_paint.anti_alias = true;
                pixmap.fill_path(&row_path, &row_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            let mut ind_pb = PathBuilder::new();
            self.add_rounded_rect(&mut ind_pb, menu_x + 12.0 * scale, item_y + 8.0 * scale, 16.0 * scale, 16.0 * scale, 4.0 * scale);
            if let Some(ind_path) = ind_pb.finish() {
                let mut ind_paint = Paint::default();
                ind_paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 220));
                pixmap.fill_path(&ind_path, &ind_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            render_text_to_pixmap(
                label,
                (menu_x + 36.0 * scale).round(),
                (item_y + 10.0 * scale).round(),
                font_size,
                Color::new(220, 225, 235, 255),
                BlendMode::Normal,
                pixmap,
            );

            render_text_to_pixmap(
                val,
                (menu_x + 175.0 * scale).round(),
                (item_y + 10.0 * scale).round(),
                font_size,
                Color::new(140, 200, 255, 255),
                BlendMode::Normal,
                pixmap,
            );

            item_y = (item_y + 38.0 * scale).round();
        }
    }

    pub fn add_rounded_rect(&self, pb: &mut tiny_skia::PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
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

fn is_tool_active(name: &str, active_tool: Tool) -> bool {
    use crate::core::{ShapeKind};
    match name {
        "Pen"         => matches!(active_tool, Tool::Pen { .. }),
        "Highlighter" => matches!(active_tool, Tool::Highlighter { .. }),
        "Laser"       => matches!(active_tool, Tool::Laser { .. }),
        "Spotlight"   => matches!(active_tool, Tool::Spotlight { .. }),
        "Eraser"      => matches!(active_tool, Tool::Eraser { .. }),
        "Crop"        => matches!(active_tool, Tool::SelectRegion),
        "Line"        => matches!(active_tool, Tool::Shape { kind: ShapeKind::Line, .. }),
        "Arrow"       => matches!(active_tool, Tool::Shape { kind: ShapeKind::Arrow, .. }),
        "Rectangle"   => matches!(active_tool, Tool::Shape { kind: ShapeKind::Rectangle, .. }),
        "Oval"        => matches!(active_tool, Tool::Shape { kind: ShapeKind::Oval, .. }),
        _             => false,
    }
}
