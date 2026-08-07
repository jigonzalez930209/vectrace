use crate::core::canvas::{BlendMode, Color};
use crate::core::render::render_text_to_pixmap;

/// Converts a Unix timestamp in seconds to (year, month, day, hour, min, sec).
pub fn secs_to_datetime(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let days = secs / 86400;
    let rem_secs = secs % 86400;
    let hour = (rem_secs / 3600) as u32;
    let min = ((rem_secs % 3600) / 60) as u32;
    let sec = (rem_secs % 60) as u32;

    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + (if m <= 2 { 1 } else { 0 });

    (y as u32, m as u32, d as u32, hour, min, sec)
}

pub fn prepare_export_pixmap(
    pixmap: &tiny_skia::Pixmap,
    crop_rect: Option<(u32, u32, u32, u32)>,
) -> Result<tiny_skia::Pixmap, String> {
    if let Some((x, y, w, h)) = crop_rect {
        if w == 0 || h == 0 {
            return Err("Selection area is empty".into());
        }
        let int_rect = tiny_skia::IntRect::from_xywh(x as i32, y as i32, w, h)
            .ok_or_else(|| "Invalid crop coordinates".to_string())?;
        pixmap
            .clone_rect(int_rect)
            .ok_or_else(|| "Failed to crop image region".to_string())
    } else {
        Ok(pixmap.clone())
    }
}

fn build_export_path(is_crop: bool) -> std::path::PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let (year, month, day, hour, min, sec) = secs_to_datetime(now);
    let filename = if is_crop {
        format!(
            "Vectrace_Crop_{:04}{:02}{:02}_{:02}{:02}{:02}.png",
            year, month, day, hour, min, sec
        )
    } else {
        format!(
            "Vectrace_{:04}{:02}{:02}_{:02}{:02}{:02}.png",
            year, month, day, hour, min, sec
        )
    };

    let target_dir = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join("Pictures"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    if !target_dir.exists() {
        let _ = std::fs::create_dir_all(&target_dir);
    }

    target_dir.join(filename)
}

pub fn save_export_pixmap(export_pixmap: &tiny_skia::Pixmap, is_crop: bool) -> Result<String, String> {
    let save_path = build_export_path(is_crop);
    export_pixmap
        .save_png(&save_path)
        .map_err(|e| format!("Failed to save PNG image: {}", e))?;
    Ok(save_path.to_string_lossy().to_string())
}

pub fn save_pixmap_to_file(pixmap: &tiny_skia::Pixmap, crop_rect: Option<(u32, u32, u32, u32)>) -> Result<String, String> {
    let export_pixmap = prepare_export_pixmap(pixmap, crop_rect)?;
    save_export_pixmap(&export_pixmap, crop_rect.is_some())
}

pub fn render_crop_selection(pixmap: &mut tiny_skia::Pixmap, x: f32, y: f32, w: f32, h: f32, scale: f32) {
    use tiny_skia::{PathBuilder, Paint, Stroke, Transform};

    let min_x = x.min(x + w);
    let max_x = x.max(x + w);
    let min_y = y.min(y + h);
    let max_y = y.max(y + h);
    let rect_w = max_x - min_x;
    let rect_h = max_y - min_y;

    if rect_w <= 1.0 || rect_h <= 1.0 {
        return;
    }

    let pix_w = pixmap.width() as f32;
    let pix_h = pixmap.height() as f32;
    let mut mask_paint = Paint::default();
    mask_paint.set_color(tiny_skia::Color::from_rgba8(0, 0, 0, 110));

    if let Some(p) = tiny_skia::Rect::from_xywh(0.0, 0.0, pix_w, min_y) {
        pixmap.fill_rect(p, &mask_paint, Transform::identity(), None);
    }
    if let Some(p) = tiny_skia::Rect::from_xywh(0.0, max_y, pix_w, (pix_h - max_y).max(0.0)) {
        pixmap.fill_rect(p, &mask_paint, Transform::identity(), None);
    }
    if let Some(p) = tiny_skia::Rect::from_xywh(0.0, min_y, min_x, rect_h) {
        pixmap.fill_rect(p, &mask_paint, Transform::identity(), None);
    }
    if let Some(p) = tiny_skia::Rect::from_xywh(max_x, min_y, (pix_w - max_x).max(0.0), rect_h) {
        pixmap.fill_rect(p, &mask_paint, Transform::identity(), None);
    }

    let mut border_pb = PathBuilder::new();
    if let Some(rect) = tiny_skia::Rect::from_xywh(min_x, min_y, rect_w, rect_h) {
        border_pb.push_rect(rect);
    }
    if let Some(path) = border_pb.finish() {
        let mut stroke_paint = Paint::default();
        stroke_paint.set_color(tiny_skia::Color::from_rgba8(0, 240, 255, 255));
        stroke_paint.anti_alias = true;

        let mut stroke = Stroke::default();
        stroke.width = 2.0 * scale;
        pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
    }

    let grip_size = 8.0 * scale;
    let mut grip_paint = Paint::default();
    grip_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 255));
    grip_paint.anti_alias = true;

    let mut grip_stroke_paint = Paint::default();
    grip_stroke_paint.set_color(tiny_skia::Color::from_rgba8(0, 200, 255, 255));
    grip_stroke_paint.anti_alias = true;

    let mut grip_stroke = Stroke::default();
    grip_stroke.width = 1.5 * scale;

    let mid_x = min_x + rect_w / 2.0;
    let mid_y = min_y + rect_h / 2.0;

    let handles = [
        (min_x, min_y), (max_x, min_y), (min_x, max_y), (max_x, max_y),
        (mid_x, min_y), (mid_x, max_y), (min_x, mid_y), (max_x, mid_y),
    ];

    for &(cx, cy) in &handles {
        if let Some(rect) = tiny_skia::Rect::from_xywh(cx - grip_size / 2.0, cy - grip_size / 2.0, grip_size, grip_size) {
            pixmap.fill_rect(rect, &grip_paint, Transform::identity(), None);
            let mut gpb = PathBuilder::new();
            gpb.push_rect(rect);
            if let Some(gpath) = gpb.finish() {
                pixmap.stroke_path(&gpath, &grip_stroke_paint, &grip_stroke, Transform::identity(), None);
            }
        }
    }

    let label = format!("{:.0} × {:.0} px", rect_w, rect_h);
    let font_size = (12.0 * scale).round().max(1.0);
    let label_y = if min_y - 24.0 * scale > 0.0 {
        (min_y - 24.0 * scale).round()
    } else {
        (min_y + 8.0 * scale).round()
    };
    let label_x = min_x.round();

    let mut bg_pb = PathBuilder::new();
    if let Some(rect) = tiny_skia::Rect::from_xywh(label_x, label_y, label.len() as f32 * 7.5 * scale, 20.0 * scale) {
        bg_pb.push_rect(rect);
    }
    if let Some(bg_path) = bg_pb.finish() {
        let mut bg_p = Paint::default();
        bg_p.set_color(tiny_skia::Color::from_rgba8(0, 0, 0, 200));
        pixmap.fill_path(&bg_path, &bg_p, tiny_skia::FillRule::Winding, Transform::identity(), None);
    }

    render_text_to_pixmap(
        &label,
        label_x + 4.0 * scale,
        label_y + 3.0 * scale,
        font_size,
        Color::new(0, 240, 255, 255),
        BlendMode::Normal,
        pixmap,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_conversion() {
        let (y, m, d, _h, _min, _s) = secs_to_datetime(1700000000);
        assert_eq!(y, 2023);
        assert_eq!(m, 11);
        assert_eq!(d, 14);
    }
}
