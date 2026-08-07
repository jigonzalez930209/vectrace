/// X11 rendering: redraw_rect, dirty-rect computation, RGBA->BGRA conversion.
use crate::core::{Canvas, StrokeType};
use crate::ui::Toolbar;
use crate::platform::x11::backend::X11Backend;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Rectangle, ImageFormat, ConnectionExt as _};

/// Computes the minimal dirty rectangle for the current stroke being drawn.
/// Returns None when a full-screen redraw is needed.
pub fn get_dirty_rect(
    canvas: &Canvas,
    screen_width: u16,
    screen_height: u16,
    prev_spotlight: Option<crate::core::Point>,
    prev_shape_bounds: Option<(f32, f32, f32, f32)>,
) -> Option<Rectangle> {
    if let Some(stroke) = canvas.current_stroke() {
        if stroke.stroke_type == StrokeType::Spotlight {
            if let Some(&p) = stroke.points.last() {
                let r = stroke.width + 25.0;
                let mut min_x = p.x - r;
                let mut max_x = p.x + r;
                let mut min_y = p.y - r;
                let mut max_y = p.y + r;

                if let Some(prev) = prev_spotlight {
                    min_x = f32::min(min_x, prev.x - r);
                    max_x = f32::max(max_x, prev.x + r);
                    min_y = f32::min(min_y, prev.y - r);
                    max_y = f32::max(max_y, prev.y + r);
                }

                return Some(make_rect(min_x, min_y, max_x, max_y, screen_width, screen_height));
            }
        } else if matches!(stroke.stroke_type, StrokeType::Line | StrokeType::Arrow | StrokeType::Rectangle | StrokeType::Oval) {
            if stroke.points.len() >= 1 {
                let p1 = stroke.points[0];
                let p2 = *stroke.points.last().unwrap();
                let mut min_x = f32::min(p1.x, p2.x);
                let mut max_x = f32::max(p1.x, p2.x);
                let mut min_y = f32::min(p1.y, p2.y);
                let mut max_y = f32::max(p1.y, p2.y);

                if let Some((ox1, oy1, ox2, oy2)) = prev_shape_bounds {
                    min_x = min_x.min(ox1).min(ox2);
                    max_x = max_x.max(ox1).max(ox2);
                    min_y = min_y.min(oy1).min(oy2);
                    max_y = max_y.max(oy1).max(oy2);
                }

                let pad = stroke.width * 4.0 + 35.0;
                return Some(make_rect(min_x - pad, min_y - pad, max_x + pad, max_y + pad, screen_width, screen_height));
            }
        } else if stroke.stroke_type == StrokeType::Freehand {
            let points = &stroke.points;
            let len = points.len();
            if len >= 2 {
                let p_last = points[len - 1];
                let p_prev = points[len - 2];
                let mut min_x = f32::min(p_last.x, p_prev.x);
                let mut max_x = f32::max(p_last.x, p_prev.x);
                let mut min_y = f32::min(p_last.y, p_prev.y);
                let mut max_y = f32::max(p_last.y, p_prev.y);

                if len >= 3 {
                    let p2 = points[len - 3];
                    min_x = f32::min(min_x, p2.x);
                    max_x = f32::max(max_x, p2.x);
                    min_y = f32::min(min_y, p2.y);
                    max_y = f32::max(max_y, p2.y);
                }

                let pad = stroke.width + 25.0;
                return Some(make_rect(min_x - pad, min_y - pad, max_x + pad, max_y + pad, screen_width, screen_height));
            }
        } else {
            let points = &stroke.points;
            if !points.is_empty() {
                let mut min_x = points[0].x;
                let mut max_x = points[0].x;
                let mut min_y = points[0].y;
                let mut max_y = points[0].y;

                for p in points {
                    min_x = f32::min(min_x, p.x);
                    max_x = f32::max(max_x, p.x);
                    min_y = f32::min(min_y, p.y);
                    max_y = f32::max(max_y, p.y);
                }

                let mut pad = stroke.width + 25.0;
                if stroke.stroke_type == StrokeType::Text {
                    if let Some(ref text) = stroke.text_content {
                        pad = pad.max(text.len() as f32 * stroke.font_size + 40.0);
                    }
                }

                return Some(make_rect(min_x - pad, min_y - pad, max_x + pad, max_y + pad, screen_width, screen_height));
            }
        }
    }
    None
}

fn make_rect(min_x: f32, min_y: f32, max_x: f32, max_y: f32, sw: u16, sh: u16) -> Rectangle {
    let x1 = min_x.max(0.0).floor() as i16;
    let y1 = min_y.max(0.0).floor() as i16;
    let x2 = max_x.min(sw as f32).ceil() as i16;
    let y2 = max_y.min(sh as f32).ceil() as i16;
    Rectangle { x: x1, y: y1, width: (x2 - x1).max(1) as u16, height: (y2 - y1).max(1) as u16 }
}

/// Always returns a full-screen dirty rect for crop selection redraws
/// (the surrounding dimming changes over the whole screen).
pub fn compute_crop_dirty_rect(
    _old: Option<(f32, f32, f32, f32)>,
    _new: Option<(f32, f32, f32, f32)>,
    screen_w: u16,
    screen_h: u16,
    _scale: f32,
) -> Option<Rectangle> {
    if screen_w == 0 || screen_h == 0 {
        None
    } else {
        Some(Rectangle { x: 0, y: 0, width: screen_w, height: screen_h })
    }
}

impl X11Backend {
    pub fn redraw_rect(
        &mut self,
        conn: &impl Connection,
        win_id: u32,
        gc_id: u32,
        canvas: &mut Canvas,
        toolbar: &Toolbar,
        rect: Option<Rectangle>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let w = self.width as u32;
        let h = self.height as u32;

        let expected_len = (w * h * 4) as usize;
        if self.x11_pixels.len() != expected_len {
            self.x11_pixels = vec![0u8; expected_len];
            self.base_pixmap = Some(tiny_skia::Pixmap::new(w, h).unwrap());
            self.active_pixmap = Some(tiny_skia::Pixmap::new(w, h).unwrap());
            self.completed_strokes_dirty = true;
        }

        let base = self.base_pixmap.as_mut().unwrap();
        let active = self.active_pixmap.as_mut().unwrap();

        let blit_rect = rect.unwrap_or(Rectangle { x: 0, y: 0, width: self.width, height: self.height });

        let rx = (blit_rect.x as u32).min(w - 1);
        let ry = (blit_rect.y as u32).min(h - 1);
        let rw = (blit_rect.width as u32).min(w - rx);
        let rh = (blit_rect.height as u32).min(h - ry);

        if rw == 0 || rh == 0 {
            return Ok(());
        }

        if self.completed_strokes_dirty {
            canvas.render_background(base);
            canvas.render_completed_strokes(base);
            self.completed_strokes_dirty = false;
            active.data_mut().copy_from_slice(base.data());
        } else {
            // Restore only the dirty region from the base layer
            for row in 0..rh {
                let row_start = ((ry + row) * w + rx) as usize * 4;
                let len = rw as usize * 4;
                active.data_mut()[row_start..row_start + len]
                    .copy_from_slice(&base.data()[row_start..row_start + len]);
            }
        }

        // Render current active stroke
        if let Some(stroke) = canvas.current_stroke() {
            if stroke.stroke_type == StrokeType::Text {
                let cur_text = stroke.text_content.as_deref().unwrap_or("");
                let mut temp_stroke = stroke.clone();
                temp_stroke.text_content = Some(format!("{}|", cur_text));

                let mut temp_canvas = Canvas::new(w, h);
                temp_canvas.start_stroke(temp_stroke);
                temp_canvas.render_current_stroke(active);
            } else {
                canvas.render_current_stroke(active);
            }
        }

        let has_crop_selection = self.crop_start.is_some() && self.crop_current.is_some();
        if !self.is_hidden {
            toolbar.draw(
                active,
                self.active_tool,
                self.passthrough,
                canvas.background_mode,
                self.show_settings_menu,
                self.show_color_menu,
                self.monitor_mode,
                has_crop_selection,
            );
        } else {
            active.fill(tiny_skia::Color::TRANSPARENT);
        }

        if let (Some((sx, sy)), Some((cx, cy))) = (self.crop_start, self.crop_current) {
            crate::core::render_crop_selection(active, sx, sy, cx - sx, cy - sy, self.scale_factor);
        }

        if let Some(ref toast) = self.toast_notification {
            if !toast.is_expired() {
                toast.draw(active, self.width as f32, self.scale_factor);
            } else {
                self.toast_notification = None;
            }
        }

        // PERFORMANCE: RGBA->BGRA swizzle using chunks_exact instead of individual byte assignments.
        // chunks_exact enables SIMD auto-vectorization by the compiler.
        let src = active.data();
        let mut sub_pixels = vec![0u8; (rw * rh * 4) as usize];

        for row in 0..rh {
            let src_row_start = ((ry + row) * w + rx) as usize * 4;
            let dst_row_start = (row * rw) as usize * 4;

            let src_row = &src[src_row_start..src_row_start + rw as usize * 4];
            let dst_row = &mut sub_pixels[dst_row_start..dst_row_start + rw as usize * 4];

            for (src_px, dst_px) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
                dst_px[0] = src_px[2]; // B <- R
                dst_px[1] = src_px[1]; // G
                dst_px[2] = src_px[0]; // R <- B
                dst_px[3] = src_px[3]; // A
            }
        }

        conn.put_image(
            ImageFormat::Z_PIXMAP,
            win_id,
            gc_id,
            rw as u16,
            rh as u16,
            rx as i16,
            ry as i16,
            0,
            32,
            &sub_pixels,
        )?;
        conn.flush()?;
        Ok(())
    }
}
