use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::core::canvas::{BlendMode, Color, Point, Stroke, StrokeType};


const EMBEDDED_FONT_BYTES: &[u8] = include_bytes!("../../assets/Roboto-Regular.ttf");

static SYSTEM_FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();
static GLYPH_CACHE: OnceLock<Mutex<HashMap<(char, u32), CachedGlyph>>> = OnceLock::new();

struct CachedGlyph {
    width: usize,
    height: usize,
    xmin: f32,
    ymin: f32,
    glyph_h: f32,
    advance: f32,
    bitmap: Vec<u8>,
}

pub fn get_system_font() -> &'static Option<fontdue::Font> {
    SYSTEM_FONT.get_or_init(|| {
        if let Ok(font) = fontdue::Font::from_bytes(EMBEDDED_FONT_BYTES, fontdue::FontSettings::default()) {
            return Some(font);
        }

        let font_paths = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ];

        for path in &font_paths {
            if let Ok(bytes) = std::fs::read(path) {
                if let Ok(font) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                    return Some(font);
                }
            }
        }
        None
    })
}

fn glyph_cache() -> &'static Mutex<HashMap<(char, u32), CachedGlyph>> {
    GLYPH_CACHE.get_or_init(|| Mutex::new(HashMap::with_capacity(256)))
}

fn with_cached_glyph<R>(font: &fontdue::Font, ch: char, font_size: f32, f: impl FnOnce(&CachedGlyph) -> R) -> R {
    let key = (ch, font_size.to_bits());
    let mut cache = glyph_cache().lock().unwrap_or_else(|e| e.into_inner());
    if !cache.contains_key(&key) {
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        if cache.len() > 2048 {
            cache.clear();
        }
        cache.insert(key, CachedGlyph {
            width: metrics.width,
            height: metrics.height,
            xmin: metrics.bounds.xmin,
            ymin: metrics.bounds.ymin,
            glyph_h: metrics.bounds.height,
            advance: metrics.advance_width,
            bitmap,
        });
    }
    f(cache.get(&key).unwrap())
}

/// Advance-width sum for layout (crop size label, tooltips, etc.).
/// Matches `render_text_to_pixmap` rounding so box padding stays symmetric.
pub fn measure_text_width(text: &str, font_size: f32) -> f32 {
    measure_text_ink(text, font_size).0
}

/// Ink bounds for a string as drawn by `render_text_to_pixmap`.
/// Returns `(advance_width, ink_top, ink_bottom)` relative to `start_y`
/// (the same origin passed to `render_text_to_pixmap`).
pub fn measure_text_ink(text: &str, font_size: f32) -> (f32, f32, f32) {
    let font_size = font_size.round().max(1.0);
    let Some(font) = get_system_font() else {
        let w = text.chars().count() as f32 * font_size * 0.55;
        return (w, 0.0, font_size);
    };
    let ascent = font
        .horizontal_line_metrics(font_size)
        .map(|m| m.ascent)
        .unwrap_or(font_size * 0.8);

    let mut width = 0.0f32;
    let mut ink_top = f32::INFINITY;
    let mut ink_bottom = f32::NEG_INFINITY;
    let mut cur_x = 0.0f32;

    for ch in text.chars() {
        let advance = with_cached_glyph(font, ch, font_size, |g| {
            if g.width > 0 && g.height > 0 {
                // Same placement as render_text_to_pixmap:
                // gy = start_y + ascent - ymin - glyph_h
                let top = ascent - g.ymin - g.glyph_h;
                let bottom = top + g.height as f32;
                ink_top = ink_top.min(top);
                ink_bottom = ink_bottom.max(bottom);
            }
            g.advance
        });
        cur_x = (cur_x + advance).round();
        width = cur_x;
    }

    if !ink_top.is_finite() || !ink_bottom.is_finite() || ink_bottom <= ink_top {
        // Spaces / empty ink — fall back to em box.
        (width, 0.0, ascent)
    } else {
        (width, ink_top, ink_bottom)
    }
}

/// Renders a text string onto a pixmap using alpha blending directly into the RGBA buffer.
/// PERFORMANCE: Avoids per-pixel `fill_rect` calls by compositing directly into the pixel buffer
/// using Porter-Duff SourceOver alpha blending. This eliminates thousands of tiny-skia drawcalls.
pub fn render_text_to_pixmap(
    text: &str,
    start_x: f32,
    start_y: f32,
    font_size: f32,
    color: Color,
    _blend_mode: BlendMode,
    pixmap: &mut tiny_skia::Pixmap,
) {
    if let Some(font) = get_system_font() {
        let font_size = font_size.round().max(1.0);
        let mut cur_x = start_x.round();
        let pix_w = pixmap.width() as i32;
        let pix_h = pixmap.height() as i32;
        let data = pixmap.data_mut();

        let font_metrics = font.horizontal_line_metrics(font_size);
        let ascent = font_metrics.map(|m| m.ascent).unwrap_or(font_size * 0.8);
        let baseline_y = start_y + ascent;

        for ch in text.chars() {
            let advance = with_cached_glyph(font, ch, font_size, |g| {
                if g.width > 0 && g.height > 0 {
                    let gx = (cur_x + g.xmin).round() as i32;
                    let gy = (baseline_y - g.ymin - g.glyph_h).round() as i32;

                    for row in 0..g.height as i32 {
                        let py = gy + row;
                        if py < 0 || py >= pix_h {
                            continue;
                        }
                        for col in 0..g.width as i32 {
                            let px = gx + col;
                            if px < 0 || px >= pix_w {
                                continue;
                            }
                            let alpha_coverage = g.bitmap[(row as usize) * g.width + col as usize];
                            if alpha_coverage == 0 {
                                continue;
                            }
                            let src_a = ((color.a as u32 * alpha_coverage as u32) / 255) as u8;
                            if src_a == 0 {
                                continue;
                            }
                            let dst_idx = ((py * pix_w + px) as usize) * 4;
                            let inv_a = 255u32 - src_a as u32;
                            let src_a32 = src_a as u32;
                            data[dst_idx]     = ((color.r as u32 * src_a32 + data[dst_idx] as u32 * inv_a) / 255) as u8;
                            data[dst_idx + 1] = ((color.g as u32 * src_a32 + data[dst_idx + 1] as u32 * inv_a) / 255) as u8;
                            data[dst_idx + 2] = ((color.b as u32 * src_a32 + data[dst_idx + 2] as u32 * inv_a) / 255) as u8;
                            data[dst_idx + 3] = (src_a32 + data[dst_idx + 3] as u32 * inv_a / 255) as u8;
                        }
                    }
                }
                g.advance
            });
            cur_x = (cur_x + advance).round();
        }
    }
}

pub fn render_stroke(stroke: &Stroke, pixmap: &mut tiny_skia::Pixmap) {
    if stroke.points.is_empty() {
        return;
    }

    if stroke.stroke_type == StrokeType::Text {
        if let Some(ref text) = stroke.text_content {
            let start_p = stroke.points[0];
            render_text_to_pixmap(text, start_p.x, start_p.y, stroke.font_size, stroke.color, stroke.blend_mode, pixmap);
        }
        return;
    }

    let mut pb = tiny_skia::PathBuilder::new();

    match stroke.stroke_type {
        StrokeType::Freehand => {
            // Prefer incremental smooth cache (O(1) append per point).
            let owned;
            let smoothed = if !stroke.smooth_cache.is_empty() {
                stroke.smooth_cache.as_slice()
            } else {
                owned = stroke.smoothed_points(3);
                owned.as_slice()
            };
            if smoothed.is_empty() {
                return;
            }
            pb.move_to(smoothed[0].x, smoothed[0].y);
            for pt in &smoothed[1..] {
                pb.line_to(pt.x, pt.y);
            }
        }
        StrokeType::Line => {
            let p1 = stroke.points[0];
            let p2 = *stroke.points.last().unwrap();
            pb.move_to(p1.x, p1.y);
            pb.line_to(p2.x, p2.y);
        }
        StrokeType::Arrow => {
            let p1 = stroke.points[0];
            let p2 = *stroke.points.last().unwrap();
            pb.move_to(p1.x, p1.y);
            pb.line_to(p2.x, p2.y);

            let dx = p2.x - p1.x;
            let dy = p2.y - p1.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 2.0 {
                let arrow_len = (stroke.width * 4.0).max(14.0);
                let angle = dy.atan2(dx);

                let angle1 = angle + std::f32::consts::PI * 0.85;
                let angle2 = angle - std::f32::consts::PI * 0.85;

                let x1 = p2.x + arrow_len * angle1.cos();
                let y1 = p2.y + arrow_len * angle1.sin();
                let x2 = p2.x + arrow_len * angle2.cos();
                let y2 = p2.y + arrow_len * angle2.sin();

                pb.move_to(p2.x, p2.y);
                pb.line_to(x1, y1);
                pb.move_to(p2.x, p2.y);
                pb.line_to(x2, y2);
            }
        }
        StrokeType::Rectangle => {
            let p1 = stroke.points[0];
            let p2 = *stroke.points.last().unwrap();
            let x = f32::min(p1.x, p2.x).round();
            let y = f32::min(p1.y, p2.y).round();
            let w = (p1.x - p2.x).abs().round().max(1.0);
            let h = (p1.y - p2.y).abs().round().max(1.0);
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
                pb.push_rect(rect);
            }
        }
        StrokeType::Oval => {
            let p1 = stroke.points[0];
            let p2 = *stroke.points.last().unwrap();
            let x = f32::min(p1.x, p2.x);
            let y = f32::min(p1.y, p2.y);
            let w = (p1.x - p2.x).abs();
            let h = (p1.y - p2.y).abs();
            if w > 0.5 && h > 0.5 {
                let cx = x + w / 2.0;
                let cy = y + h / 2.0;
                let rx = w / 2.0;
                let ry = h / 2.0;
                let kappa = 0.55228475;
                let ox = rx * kappa;
                let oy = ry * kappa;
                pb.move_to(cx - rx, cy);
                pb.cubic_to(cx - rx, cy - oy, cx - ox, cy - ry, cx, cy - ry);
                pb.cubic_to(cx + ox, cy - ry, cx + rx, cy - oy, cx + rx, cy);
                pb.cubic_to(cx + rx, cy + oy, cx + ox, cy + ry, cx, cy + ry);
                pb.cubic_to(cx - ox, cy + ry, cx - rx, cy + oy, cx - rx, cy);
                pb.close();
            }
        }
        StrokeType::Text | StrokeType::Laser | StrokeType::Spotlight => {}
    }

    if let Some(path) = pb.finish() {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(tiny_skia::Color::from_rgba8(
            stroke.color.r,
            stroke.color.g,
            stroke.color.b,
            stroke.color.a,
        ));
        paint.blend_mode = stroke.blend_mode.into();
        paint.anti_alias = true;

        let mut skia_stroke = tiny_skia::Stroke::default();
        skia_stroke.width = stroke.width;
        let is_shape = matches!(
            stroke.stroke_type,
            StrokeType::Rectangle | StrokeType::Oval | StrokeType::Line | StrokeType::Arrow
        );
        if is_shape {
            skia_stroke.line_cap = tiny_skia::LineCap::Butt;
            skia_stroke.line_join = tiny_skia::LineJoin::Miter;
            skia_stroke.miter_limit = 4.0;
        } else {
            skia_stroke.line_cap = tiny_skia::LineCap::Round;
            skia_stroke.line_join = tiny_skia::LineJoin::Round;
        }

        pixmap.stroke_path(&path, &paint, &skia_stroke, tiny_skia::Transform::identity(), None);
    }
}

pub fn render_laser_stroke(stroke: &Stroke, now_ms: u64, pixmap: &mut tiny_skia::Pixmap) {
    if stroke.points.len() < 2 {
        return;
    }

    let max_age = 1200.0; // 1.2 seconds decay
    let points = &stroke.points;

    let mut pb = tiny_skia::PathBuilder::new();
    let mut prev_pt: Option<Point> = None;

    for pt in points.iter() {
        let age = (now_ms.saturating_sub(pt.timestamp_ms)) as f32;
        if age > max_age {
            prev_pt = None;
            continue;
        }

        if let Some(p0) = prev_pt {
            pb.move_to(p0.x, p0.y);
            pb.line_to(pt.x, pt.y);
        }
        prev_pt = Some(*pt);
    }

    if let Some(path) = pb.finish() {
        let mut glow_paint = tiny_skia::Paint::default();
        glow_paint.set_color(tiny_skia::Color::from_rgba8(stroke.color.r, stroke.color.g, stroke.color.b, 120));
        glow_paint.anti_alias = true;

        let mut glow_stroke = tiny_skia::Stroke::default();
        glow_stroke.width = stroke.width * 2.2;
        glow_stroke.line_cap = tiny_skia::LineCap::Round;
        glow_stroke.line_join = tiny_skia::LineJoin::Round;

        pixmap.stroke_path(&path, &glow_paint, &glow_stroke, tiny_skia::Transform::identity(), None);

        let mut core_paint = tiny_skia::Paint::default();
        core_paint.set_color(tiny_skia::Color::from_rgba8(255, 255, 255, 240));
        core_paint.anti_alias = true;

        let mut core_stroke = tiny_skia::Stroke::default();
        core_stroke.width = stroke.width * 0.7;
        core_stroke.line_cap = tiny_skia::LineCap::Round;
        core_stroke.line_join = tiny_skia::LineJoin::Round;

        pixmap.stroke_path(&path, &core_paint, &core_stroke, tiny_skia::Transform::identity(), None);
    }
}

pub fn render_spotlight_stroke(stroke: &Stroke, pixmap: &mut tiny_skia::Pixmap) {
    if let Some(&p) = stroke.points.last() {
        let cx = p.x;
        let cy = p.y;
        let r = stroke.width;

        let mut cpb = tiny_skia::PathBuilder::new();
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
            let mut clear_paint = tiny_skia::Paint::default();
            clear_paint.blend_mode = tiny_skia::BlendMode::Clear;
            clear_paint.anti_alias = true;
            pixmap.fill_path(&cpath, &clear_paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);

            let mut ring_paint = tiny_skia::Paint::default();
            ring_paint.set_color(tiny_skia::Color::from_rgba8(50, 130, 245, 230));
            ring_paint.anti_alias = true;

            let mut ring_stroke = tiny_skia::Stroke::default();
            ring_stroke.width = 3.0;
            pixmap.stroke_path(&cpath, &ring_paint, &ring_stroke, tiny_skia::Transform::identity(), None);
        }
    }
}


