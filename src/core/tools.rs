use crate::core::canvas::{Color, BlendMode, Stroke, StrokeType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Arrow,
    Rectangle,
    Oval,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Pen {
        color: Color,
        width: f32,
    },
    Highlighter {
        color: Color,
        width: f32,
    },
    Eraser {
        width: f32,
    },
    Shape {
        kind: ShapeKind,
        color: Color,
        width: f32,
    },
    Text {
        color: Color,
        font_size: f32,
    },
    Laser {
        color: Color,
        width: f32,
    },
    Spotlight {
        radius: f32,
    },
}

impl Tool {
    pub fn default_pen() -> Self {
        Tool::Pen {
            color: Color::new(255, 0, 0, 255), // Red
            width: 4.0,
        }
    }

    pub fn default_highlighter() -> Self {
        Tool::Highlighter {
            color: Color::new(255, 255, 0, 128), // Yellow, semi-transparent
            width: 24.0,
        }
    }

    pub fn default_eraser() -> Self {
        Tool::Eraser { width: 30.0 }
    }

    pub fn default_shape(kind: ShapeKind) -> Self {
        Tool::Shape {
            kind,
            color: Color::new(255, 0, 0, 255), // Red by default
            width: 4.0,
        }
    }

    pub fn default_text() -> Self {
        Tool::Text {
            color: Color::new(255, 255, 255, 255), // White by default
            font_size: 26.0,
        }
    }

    pub fn default_laser() -> Self {
        Tool::Laser {
            color: Color::new(255, 0, 100, 255), // Neon pink/red
            width: 8.0,
        }
    }

    pub fn default_spotlight() -> Self {
        Tool::Spotlight { radius: 120.0 }
    }

    pub fn color(&self) -> Option<Color> {
        match self {
            Tool::Pen { color, .. } => Some(*color),
            Tool::Highlighter { color, .. } => Some(*color),
            Tool::Shape { color, .. } => Some(*color),
            Tool::Text { color, .. } => Some(*color),
            Tool::Laser { color, .. } => Some(*color),
            Tool::Eraser { .. } => None,
            Tool::Spotlight { .. } => None,
        }
    }

    pub fn set_color(&mut self, new_color: Color) {
        match self {
            Tool::Pen { color, .. } => *color = new_color,
            Tool::Highlighter { color, .. } => {
                let mut c = new_color;
                if c.a == 255 {
                    c.a = 128; // Preserve semi-transparency for highlighter
                }
                *color = c;
            }
            Tool::Shape { color, .. } => *color = new_color,
            Tool::Text { color, .. } => *color = new_color,
            Tool::Laser { color, .. } => *color = new_color,
            Tool::Eraser { .. } => {}
            Tool::Spotlight { .. } => {}
        }
    }

    pub fn create_stroke(&self) -> Option<Stroke> {
        match self {
            Tool::Pen { color, width } => Some(Stroke::new(*color, *width, BlendMode::Normal)),
            Tool::Highlighter { color, width } => {
                let mut c = *color;
                if c.a == 255 {
                    c.a = 128;
                }
                // Use BlendMode::Normal (SourceOver) for clean semi-transparent highlighter on transparent overlays
                Some(Stroke::new(c, *width, BlendMode::Normal))
            }
            Tool::Shape { kind, color, width } => {
                let stroke_type = match kind {
                    ShapeKind::Line => StrokeType::Line,
                    ShapeKind::Arrow => StrokeType::Arrow,
                    ShapeKind::Rectangle => StrokeType::Rectangle,
                    ShapeKind::Oval => StrokeType::Oval,
                };
                Some(Stroke::new_shape(*color, *width, BlendMode::Normal, stroke_type))
            }
            Tool::Text { color, font_size } => {
                Some(Stroke::new_text(*color, crate::core::Point::new(0.0, 0.0, 1.0, 0), String::new(), *font_size))
            }
            Tool::Laser { color, width } => {
                Some(Stroke::new_shape(*color, *width, BlendMode::Normal, StrokeType::Laser))
            }
            Tool::Spotlight { radius } => {
                Some(Stroke::new_shape(Color::new(0, 0, 0, 0), *radius, BlendMode::Normal, StrokeType::Spotlight))
            }
            Tool::Eraser { width } => {
                Some(Stroke::new(Color::new(0, 0, 0, 0), *width, BlendMode::Clear))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_color_changes() {
        let mut tool = Tool::default_pen();
        assert_eq!(tool.color(), Some(Color::new(255, 0, 0, 255)));

        tool.set_color(Color::new(0, 255, 0, 255));
        assert_eq!(tool.color(), Some(Color::new(0, 255, 0, 255)));

        let mut shape_tool = Tool::default_shape(ShapeKind::Arrow);
        shape_tool.set_color(Color::new(50, 130, 245, 255));
        assert_eq!(shape_tool.color(), Some(Color::new(50, 130, 245, 255)));

        let mut text_tool = Tool::default_text();
        text_tool.set_color(Color::new(245, 200, 30, 255));
        assert_eq!(text_tool.color(), Some(Color::new(245, 200, 30, 255)));
    }
}
