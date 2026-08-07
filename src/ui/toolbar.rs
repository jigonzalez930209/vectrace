use crate::core::{Tool, Color, ShapeKind, BackgroundMode, MonitorMode, BlendMode, render_text_to_pixmap};


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolbarAction {
    SelectTool(Tool),
    SelectShape(ShapeKind),
    SetColor(Color),
    ToggleBackgroundMode,
    Clear,
    SaveFull,
    ConfirmCrop,
    TogglePassthrough,
    ToggleSettingsMenu,
    ToggleColorMenu,
    ToggleMonitorMode,
    MinimizeToTray,
    Exit,
    StartDrag,
}

/// Logical (unscaled) toolbar metrics. BTN_GAP +2 and GROUP_PAD +4 vs the old layout.
const GRIP_W: f32 = 24.0;
const BTN_GAP: f32 = 6.0;
const GROUP_PAD: f32 = 8.0;
const RIGHT_PAD: f32 = 10.0;
const TOOL_BTN: f32 = 30.0;
const COLOR_BTN_W: f32 = 44.0;
const ACTION_BTN_W: f32 = 36.0;
const BTN_H: f32 = 30.0;
const BAR_H: f32 = 38.0;
const TOOL_COUNT: usize = 10;
const ACTION_COUNT: usize = 7;

#[derive(Debug, Clone, Copy)]
struct ToolbarLayout {
    width: f32,
    dividers: [f32; 3],
    tool_xs: [f32; TOOL_COUNT],
    color_x: f32,
    action_xs: [f32; ACTION_COUNT],
}

impl ToolbarLayout {
    fn compute() -> Self {
        let div0 = GRIP_W;
        let tools_start = div0 + GROUP_PAD;
        let mut tool_xs = [0.0; TOOL_COUNT];
        for i in 0..TOOL_COUNT {
            tool_xs[i] = tools_start + i as f32 * (TOOL_BTN + BTN_GAP);
        }
        let tools_end = tool_xs[TOOL_COUNT - 1] + TOOL_BTN;
        let div1 = tools_end + GROUP_PAD;
        let color_x = div1 + GROUP_PAD;
        let color_end = color_x + COLOR_BTN_W;
        let div2 = color_end + GROUP_PAD;
        let actions_start = div2 + GROUP_PAD;
        let mut action_xs = [0.0; ACTION_COUNT];
        for i in 0..ACTION_COUNT {
            action_xs[i] = actions_start + i as f32 * (ACTION_BTN_W + BTN_GAP);
        }
        let actions_end = action_xs[ACTION_COUNT - 1] + ACTION_BTN_W;
        let width = actions_end + RIGHT_PAD;

        Self {
            width,
            dividers: [div0, div1, div2],
            tool_xs,
            color_x,
            action_xs,
        }
    }

    fn color_menu_x(&self) -> f32 {
        self.color_x
    }

    fn settings_menu_x(&self) -> f32 {
        self.action_xs[4] // Settings
    }
}

fn layout() -> ToolbarLayout {
    ToolbarLayout::compute()
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
        let lay = layout();
        let width = lay.width * scale;
        let height = BAR_H * scale;
        let x = (screen_width - width) / 2.0;
        let y = 12.0 * scale;
        Self { x, y, width, height, scale_factor: scale }
    }

    /// Logical X offset of the color button (for passthrough hit regions).
    pub fn color_btn_logical_x(&self) -> f32 {
        layout().color_x
    }

    /// Logical X offset of the settings button (for passthrough hit regions).
    pub fn settings_btn_logical_x(&self) -> f32 {
        layout().settings_menu_x()
    }

    pub fn handle_click(
        &self,
        click_x: f32,
        click_y: f32,
        show_settings_menu: bool,
        show_color_menu: bool,
        has_crop_selection: bool,
    ) -> Option<ToolbarAction> {
        let scale = self.scale_factor;
        let lay = layout();

        if show_color_menu {
            let menu_x = self.x + lay.color_menu_x() * scale;
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

        if show_settings_menu {
            let menu_x = self.x + lay.settings_menu_x() * scale;
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

        if rx >= 0.0 && rx < GRIP_W {
            return Some(ToolbarAction::StartDrag);
        }

        let tool_actions = [
            ToolbarAction::SelectTool(Tool::default_pen()),
            ToolbarAction::SelectTool(Tool::default_highlighter()),
            ToolbarAction::SelectShape(ShapeKind::Line),
            ToolbarAction::SelectShape(ShapeKind::Arrow),
            ToolbarAction::SelectShape(ShapeKind::Rectangle),
            ToolbarAction::SelectShape(ShapeKind::Oval),
            ToolbarAction::SelectTool(Tool::default_laser()),
            ToolbarAction::SelectTool(Tool::default_spotlight()),
            ToolbarAction::SelectTool(Tool::default_eraser()),
            if has_crop_selection {
                ToolbarAction::ConfirmCrop
            } else {
                ToolbarAction::SelectTool(Tool::default_select_region())
            },
        ];

        for (i, action) in tool_actions.into_iter().enumerate() {
            let x0 = lay.tool_xs[i];
            if rx >= x0 && rx < x0 + TOOL_BTN {
                return Some(action);
            }
        }

        if rx >= lay.color_x && rx < lay.color_x + COLOR_BTN_W {
            return Some(ToolbarAction::ToggleColorMenu);
        }

        let action_map = [
            ToolbarAction::SaveFull,
            ToolbarAction::ToggleBackgroundMode,
            ToolbarAction::Clear,
            ToolbarAction::TogglePassthrough,
            ToolbarAction::ToggleSettingsMenu,
            ToolbarAction::MinimizeToTray,
            ToolbarAction::Exit,
        ];
        for (i, action) in action_map.into_iter().enumerate() {
            let x0 = lay.action_xs[i];
            if rx >= x0 && rx < x0 + ACTION_BTN_W {
                return Some(action);
            }
        }

        Some(ToolbarAction::StartDrag)
    }

    pub fn palette_colors() -> Vec<Color> {
        vec![
            Color::new(235, 50, 50, 255),
            Color::new(245, 130, 30, 255),
            Color::new(245, 210, 30, 255),
            Color::new(50, 205, 80, 255),
            Color::new(30, 210, 220, 255),
            Color::new(50, 130, 245, 255),
            Color::new(150, 60, 245, 255),
            Color::new(245, 80, 170, 255),
            Color::new(255, 255, 255, 255),
            Color::new(180, 180, 180, 255),
            Color::new(70, 70, 70, 255),
            Color::new(20, 20, 20, 255),
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
        has_crop_selection: bool,
    ) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform, LineCap, LineJoin};

        let scale = self.scale_factor;
        let lay = layout();

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

        let mut grip_pb = PathBuilder::new();
        let gx = self.x + 10.0 * scale;
        let gy = self.y + 12.0 * scale;
        let dot_r = 1.2 * scale;
        let col_w = 4.0 * scale;
        let row_h = 4.5 * scale;

        for col in 0..2 {
            for row in 0..3 {
                let cx = gx + col as f32 * col_w;
                let cy = gy + row as f32 * row_h;
                grip_pb.push_circle(cx, cy, dot_r);
            }
        }

        if let Some(path) = grip_pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 120));
            paint.anti_alias = true;
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        }

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

        let tools = [
            "Pen",
            "Highlighter",
            "Line",
            "Arrow",
            "Rectangle",
            "Oval",
            "Laser",
            "Spotlight",
            "Eraser",
            "Crop",
        ];

        for (i, name) in tools.iter().enumerate() {
            let is_active = match *name {
                "Pen" => matches!(active_tool, Tool::Pen { .. }),
                "Highlighter" => matches!(active_tool, Tool::Highlighter { .. }),
                "Laser" => matches!(active_tool, Tool::Laser { .. }),
                "Spotlight" => matches!(active_tool, Tool::Spotlight { .. }),
                "Eraser" => matches!(active_tool, Tool::Eraser { .. }),
                "Crop" => matches!(active_tool, Tool::SelectRegion),
                "Line" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Line, .. }),
                "Arrow" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Arrow, .. }),
                "Rectangle" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Rectangle, .. }),
                "Oval" => matches!(active_tool, Tool::Shape { kind: ShapeKind::Oval, .. }),
                _ => false,
            };

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
                "Crop" => {
                    if has_crop_selection {
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

        let action_buttons = ["Save", "Board", "Clear", "Pass", "Settings", "Tray", "Exit"];

        for (i, name) in action_buttons.iter().enumerate() {
            let is_active = match *name {
                "Pass" => passthrough,
                "Board" => bg_mode != BackgroundMode::Transparent,
                "Settings" => show_settings_menu,
                _ => false,
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
                    ipb.push_circle(cx, cy, 3.5 * scale);
                    for i in 0..6 {
                        let angle = (i as f32) * std::f32::consts::PI / 3.0;
                        let r1 = 4.5 * scale;
                        let r2 = 6.5 * scale;
                        ipb.move_to(cx + r1 * angle.cos(), cy + r1 * angle.sin());
                        ipb.line_to(cx + r2 * angle.cos(), cy + r2 * angle.sin());
                    }
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

        if show_color_menu {
            self.draw_color_menu(pixmap);
        }

        if show_settings_menu {
            self.draw_settings_menu(pixmap, passthrough, bg_mode, monitor_mode);
        }
    }

    fn draw_color_menu(&self, pixmap: &mut tiny_skia::Pixmap) {
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

        let colors = Self::palette_colors();
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

    fn draw_settings_menu(&self, pixmap: &mut tiny_skia::Pixmap, passthrough: bool, bg_mode: BackgroundMode, monitor_mode: MonitorMode) {
        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let scale = self.scale_factor;
        let lay = layout();
        let menu_x = self.x + lay.settings_menu_x() * scale;
        let menu_y = self.y + self.height + 6.0 * scale;
        let menu_w = 240.0 * scale;
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
            ("Background", match bg_mode { BackgroundMode::Transparent => "Clear", BackgroundMode::Blackboard => "Black", BackgroundMode::Whiteboard => "White" }),
        ];

        let mut item_y = (menu_y + 8.0 * scale).round();

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
            self.add_rounded_rect(&mut ind_pb, menu_x + 12.0 * scale, item_y + 7.0 * scale, 18.0 * scale, 18.0 * scale, 4.0 * scale);
            if let Some(ind_path) = ind_pb.finish() {
                let mut ind_paint = Paint::default();
                ind_paint.set_color(tiny_skia::Color::from_rgba8(50, 120, 240, 220));
                pixmap.fill_path(&ind_path, &ind_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }

            let font_size = (14.0 * scale).round().max(1.0);
            render_text_to_pixmap(
                label,
                (menu_x + 38.0 * scale).round(),
                (item_y + 8.0 * scale).round(),
                font_size,
                Color::new(220, 225, 235, 255),
                BlendMode::Normal,
                pixmap,
            );

            render_text_to_pixmap(
                val,
                (menu_x + 125.0 * scale).round(),
                (item_y + 8.0 * scale).round(),
                font_size,
                Color::new(140, 200, 255, 255),
                BlendMode::Normal,
                pixmap,
            );

            item_y = (item_y + 38.0 * scale).round();
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
        let lay = layout();
        let tb = Toolbar::new_with_scale(1920.0, 2.0);
        assert_eq!(tb.scale_factor, 2.0);
        assert_eq!(tb.width, lay.width * 2.0);
        assert_eq!(tb.height, BAR_H * 2.0);
        let content_end = lay.action_xs[ACTION_COUNT - 1] + ACTION_BTN_W;
        assert_eq!(lay.width, content_end + RIGHT_PAD);
        assert!(BTN_GAP >= 6.0);
        assert!(GROUP_PAD >= 8.0);
    }
}
