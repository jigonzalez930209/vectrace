use crate::core::canvas::{current_time_ms, Color, BlendMode};
use crate::core::render::render_text_to_pixmap;

#[derive(Debug, Clone)]
pub struct ToastNotification {
    pub message: String,
    pub expire_ms: u64,
}

impl ToastNotification {
    pub fn new(message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            message: message.into(),
            expire_ms: current_time_ms() + duration_ms,
        }
    }

    pub fn is_expired(&self) -> bool {
        current_time_ms() >= self.expire_ms
    }

    pub fn draw(&self, pixmap: &mut tiny_skia::Pixmap, canvas_width: f32, scale: f32) {
        if self.is_expired() {
            return;
        }

        let font_size = (14.0 * scale).round().max(1.0);
        let padding_x = (18.0 * scale).round();
        let text_w = ((self.message.len() as f32 * 8.0) * scale).round();
        let toast_w = text_w + padding_x * 2.0;
        let toast_h = (32.0 * scale).round();
        let toast_x = ((canvas_width - toast_w) / 2.0).round();
        let toast_y = (60.0 * scale).round();

        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let mut pb = PathBuilder::new();
        let r = 8.0 * scale;
        pb.move_to(toast_x + r, toast_y);
        pb.line_to(toast_x + toast_w - r, toast_y);
        pb.quad_to(toast_x + toast_w, toast_y, toast_x + toast_w, toast_y + r);
        pb.line_to(toast_x + toast_w, toast_y + toast_h - r);
        pb.quad_to(toast_x + toast_w, toast_y + toast_h, toast_x + toast_w - r, toast_y + toast_h);
        pb.line_to(toast_x + r, toast_y + toast_h);
        pb.quad_to(toast_x, toast_y + toast_h, toast_x, toast_y + toast_h - r);
        pb.line_to(toast_x, toast_y + r);
        pb.quad_to(toast_x, toast_y, toast_x + r, toast_y);

        if let Some(path) = pb.finish() {
            let mut bg_paint = Paint::default();
            bg_paint.set_color(tiny_skia::Color::from_rgba8(24, 28, 36, 240));
            bg_paint.anti_alias = true;
            pixmap.fill_path(&path, &bg_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            border_paint.set_color(tiny_skia::Color::from_rgba8(50, 160, 255, 200));
            let mut stroke = Stroke::default();
            stroke.width = 1.2 * scale;
            pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
        }

        render_text_to_pixmap(
            &self.message,
            toast_x + padding_x,
            toast_y + toast_h / 2.0 - font_size / 2.0,
            font_size,
            Color::new(255, 255, 255, 255),
            BlendMode::Normal,
            pixmap,
        );
    }
}
