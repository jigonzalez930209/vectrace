use crate::core::{Tool, Color, ShapeKind};

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

/// Logical (unscaled) toolbar metrics.
pub const GRIP_W: f32 = 24.0;
pub const BTN_GAP: f32 = 6.0;
pub const GROUP_PAD: f32 = 8.0;
pub const RIGHT_PAD: f32 = 10.0;
pub const TOOL_BTN: f32 = 30.0;
pub const COLOR_BTN_W: f32 = 44.0;
pub const ACTION_BTN_W: f32 = 36.0;
pub const BTN_H: f32 = 30.0;
pub const BAR_H: f32 = 38.0;
pub const TOOL_COUNT: usize = 10;
pub const ACTION_COUNT: usize = 7;

#[derive(Debug, Clone, Copy)]
pub struct ToolbarLayout {
    pub width: f32,
    pub dividers: [f32; 3],
    pub tool_xs: [f32; TOOL_COUNT],
    pub color_x: f32,
    pub action_xs: [f32; ACTION_COUNT],
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

    pub fn color_menu_x(&self) -> f32 {
        self.color_x
    }

    pub fn settings_menu_x(&self) -> f32 {
        self.action_xs[4] // Settings button
    }
}

pub fn layout() -> ToolbarLayout {
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
                if ry >= 8.0 && ry < 44.0  { return Some(ToolbarAction::ToggleMonitorMode); }
                if ry >= 48.0 && ry < 84.0  { return Some(ToolbarAction::TogglePassthrough); }
                if ry >= 88.0 && ry < 124.0 { return Some(ToolbarAction::ToggleBackgroundMode); }
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
