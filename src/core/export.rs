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

/// Map a crop rect from overlay-local coords into desktop-capture coords.
///
/// Handles primary-overlay + full-desktop capture (offset) and same-aspect
/// resolution mismatches (uniform scale). Never applies independent X/Y scales
/// that would distort the selection.
pub fn map_overlay_crop_to_desktop(
    crop: (u32, u32, u32, u32),
    overlay_w: u32,
    overlay_h: u32,
    overlay_x: i32,
    overlay_y: i32,
    desktop_w: u32,
    desktop_h: u32,
) -> (u32, u32, u32, u32) {
    let (cx, cy, cw, ch) = crop;
    if overlay_w == 0 || overlay_h == 0 || desktop_w == 0 || desktop_h == 0 {
        return crop;
    }
    if desktop_w == overlay_w && desktop_h == overlay_h {
        return crop;
    }

    let sx = desktop_w as f32 / overlay_w as f32;
    let sy = desktop_h as f32 / overlay_h as f32;

    // Full desktop larger than overlay (typical primary mode): offset 1:1.
    if desktop_w >= overlay_w
        && desktop_h >= overlay_h
        && (sx - 1.0).abs() < 0.02
        && (sy - 1.0).abs() < 0.02
    {
        let x = (overlay_x.max(0) as u32).saturating_add(cx).min(desktop_w.saturating_sub(1));
        let y = (overlay_y.max(0) as u32).saturating_add(cy).min(desktop_h.saturating_sub(1));
        let w = cw.min(desktop_w.saturating_sub(x));
        let h = ch.min(desktop_h.saturating_sub(y));
        return (x, y, w, h);
    }

    // Same aspect (within 5%): uniform scale from overlay space → desktop space.
    if (sx - sy).abs() / sx.max(sy) < 0.05 {
        let scale = sx;
        let x = ((overlay_x.max(0) as f32 + cx as f32) * scale)
            .round()
            .max(0.0) as u32;
        let y = ((overlay_y.max(0) as f32 + cy as f32) * scale)
            .round()
            .max(0.0) as u32;
        let w = ((cw as f32) * scale).round().max(1.0) as u32;
        let h = ((ch as f32) * scale).round().max(1.0) as u32;
        let x = x.min(desktop_w.saturating_sub(1));
        let y = y.min(desktop_h.saturating_sub(1));
        return (x, y, w.min(desktop_w - x), h.min(desktop_h - y));
    }

    // Fallback: offset only (better than distorting).
    let x = (overlay_x.max(0) as u32).saturating_add(cx).min(desktop_w.saturating_sub(1));
    let y = (overlay_y.max(0) as u32).saturating_add(cy).min(desktop_h.saturating_sub(1));
    let w = cw.min(desktop_w.saturating_sub(x));
    let h = ch.min(desktop_h.saturating_sub(y));
    (x, y, w, h)
}

/// Build the final snapshot pixmap without distorting capture aspect ratio.
///
/// Dual-monitor portals often return 3840×1080 while a primary overlay is
/// 1920×1080. Stretching independently on X/Y squashes the desktop — we keep
/// the capture's native pixels and blit annotations at the overlay origin.
pub fn compose_desktop_with_strokes(
    desktop: Option<tiny_skia::Pixmap>,
    strokes: &[crate::core::Stroke],
    overlay_w: u32,
    overlay_h: u32,
    overlay_x: i32,
    overlay_y: i32,
    render_fallback_bg: impl FnOnce(&mut tiny_skia::Pixmap),
) -> tiny_skia::Pixmap {
    let Some(mut desktop) = desktop else {
        let mut pixmap = tiny_skia::Pixmap::new(overlay_w.max(1), overlay_h.max(1)).unwrap();
        render_fallback_bg(&mut pixmap);
        for stroke in strokes {
            crate::core::render::render_stroke(stroke, &mut pixmap);
        }
        return pixmap;
    };

    let dw = desktop.width();
    let dh = desktop.height();

    if dw == overlay_w && dh == overlay_h {
        for stroke in strokes {
            crate::core::render::render_stroke(stroke, &mut desktop);
        }
        return desktop;
    }

    println!(
        "Snapshot keep native capture {}x{} (overlay {}x{}+{}+{}) — no aspect squash",
        dw, dh, overlay_w, overlay_h, overlay_x, overlay_y
    );

    if strokes.is_empty() {
        return desktop;
    }

    // Render annotations in overlay space, then blit onto the desktop at origin.
    if let Some(mut layer) = tiny_skia::Pixmap::new(overlay_w.max(1), overlay_h.max(1)) {
        for stroke in strokes {
            crate::core::render::render_stroke(stroke, &mut layer);
        }
        let paint = tiny_skia::PixmapPaint::default();
        desktop.draw_pixmap(
            overlay_x,
            overlay_y,
            layer.as_ref(),
            &paint,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    desktop
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
    render_crop_selection_ex(pixmap, x, y, w, h, scale, true);
}

/// Crop UI. When `dim_outside` is false, only the border/grips are drawn (caller
/// already applied a dim veil — used for fast dirty-rect crop dragging).
pub fn render_crop_selection_ex(
    pixmap: &mut tiny_skia::Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    dim_outside: bool,
) {
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

    if dim_outside {
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
    let pad_x = 6.0 * scale;
    let pad_y = 3.0 * scale;
    let text_w = crate::core::render::measure_text_width(&label, font_size);
    let text_h = font_size + 2.0 * scale;
    let label_w = (text_w + pad_x * 2.0).ceil().max(1.0);
    let label_h = (text_h + pad_y * 2.0).ceil().max(1.0);
    let label_y = if min_y - label_h - 4.0 * scale > 0.0 {
        (min_y - label_h - 4.0 * scale).round()
    } else {
        (min_y + 8.0 * scale).round()
    };
    let label_x = min_x.round();

    let mut bg_pb = PathBuilder::new();
    if let Some(rect) = tiny_skia::Rect::from_xywh(label_x, label_y, label_w, label_h) {
        bg_pb.push_rect(rect);
    }
    if let Some(bg_path) = bg_pb.finish() {
        let mut bg_p = Paint::default();
        bg_p.set_color(tiny_skia::Color::from_rgba8(0, 0, 0, 200));
        pixmap.fill_path(&bg_path, &bg_p, tiny_skia::FillRule::Winding, Transform::identity(), None);
    }

    render_text_to_pixmap(
        &label,
        label_x + pad_x,
        label_y + pad_y,
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
