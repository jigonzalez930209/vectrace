use crate::core::canvas::{current_time_ms, Color, BlendMode};
use crate::core::render::{measure_text_ink, render_text_to_pixmap};

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

    /// Draw toast centered horizontally, with its top at `below_y` (typically just under the toolbar).
    /// Uses the same Roboto metrics / padding model as toolbar tooltips.
    pub fn draw(
        &self,
        pixmap: &mut tiny_skia::Pixmap,
        canvas_width: f32,
        scale: f32,
        below_y: f32,
    ) {
        if self.is_expired() {
            return;
        }

        let font_size = (12.0 * scale).round().max(1.0);
        let pad = 8.0 * scale;

        let (text_w, ink_top, ink_bottom) = measure_text_ink(&self.message, font_size);
        let ink_h = (ink_bottom - ink_top).max(1.0);

        let toast_w = (text_w + pad * 2.0).round().max(1.0);
        let toast_h = (ink_h + pad * 2.0).round().max(1.0);
        let toast_x = ((canvas_width - toast_w) / 2.0).round();
        let toast_y = below_y.round();

        use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

        let mut pb = PathBuilder::new();
        let r = 6.0 * scale;
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
            bg_paint.set_color(tiny_skia::Color::from_rgba8(15, 18, 24, 245));
            bg_paint.anti_alias = true;
            pixmap.fill_path(&path, &bg_paint, tiny_skia::FillRule::Winding, Transform::identity(), None);

            let mut border_paint = Paint::default();
            border_paint.set_color(tiny_skia::Color::from_rgba8(50, 160, 255, 180));
            let mut stroke = Stroke::default();
            stroke.width = 1.0 * scale;
            pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
        }

        let text_x = toast_x + ((toast_w - text_w) * 0.5).round();
        let text_y = toast_y + pad - ink_top;

        render_text_to_pixmap(
            &self.message,
            text_x,
            text_y,
            font_size,
            Color::new(235, 240, 250, 255),
            BlendMode::Normal,
            pixmap,
        );
    }
}
