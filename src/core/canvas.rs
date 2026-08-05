use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    0.5 * (
        (2.0 * p1) +
        (-p0 + p2) * t +
        (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t +
        (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t
    )
}

static SYSTEM_FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();

fn get_system_font() -> &'static Option<fontdue::Font> {
    SYSTEM_FONT.get_or_init(|| {
        let font_paths = [
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/cantarell/Cantarell-Regular.otf",
            "/usr/share/fonts/TTF/LiberationSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundMode {
    Transparent,
    Blackboard,
    Whiteboard,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
    pub timestamp_ms: u64,
}

impl Point {
    pub fn new(x: f32, y: f32, pressure: f32, timestamp_ms: u64) -> Self {
        Self {
            x,
            y,
            pressure,
            timestamp_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Clear,
}

impl From<BlendMode> for tiny_skia::BlendMode {
    fn from(mode: BlendMode) -> Self {
        match mode {
            BlendMode::Normal => tiny_skia::BlendMode::SourceOver,
            BlendMode::Multiply => tiny_skia::BlendMode::Multiply,
            BlendMode::Screen => tiny_skia::BlendMode::Screen,
            BlendMode::Overlay => tiny_skia::BlendMode::Overlay,
            BlendMode::Darken => tiny_skia::BlendMode::Darken,
            BlendMode::Lighten => tiny_skia::BlendMode::Lighten,
            BlendMode::Clear => tiny_skia::BlendMode::Clear,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeType {
    Freehand,
    Line,
    Arrow,
    Rectangle,
    Oval,
    Text,
    Laser,
    Spotlight,
}

#[derive(Debug, Clone)]
pub struct Stroke {
    pub points: Vec<Point>,
    pub color: Color,
    pub width: f32,
    pub blend_mode: BlendMode,
    pub stroke_type: StrokeType,
    pub text_content: Option<String>,
    pub font_size: f32,
}

impl Stroke {
    pub fn new(color: Color, width: f32, blend_mode: BlendMode) -> Self {
        Self {
            points: Vec::new(),
            color,
            width,
            blend_mode,
            stroke_type: StrokeType::Freehand,
            text_content: None,
            font_size: 24.0,
        }
    }

    pub fn new_shape(color: Color, width: f32, blend_mode: BlendMode, stroke_type: StrokeType) -> Self {
        Self {
            points: Vec::new(),
            color,
            width,
            blend_mode,
            stroke_type,
            text_content: None,
            font_size: 24.0,
        }
    }

    pub fn new_text(color: Color, position: Point, text: String, font_size: f32) -> Self {
        Self {
            points: vec![position],
            color,
            width: 2.0,
            blend_mode: BlendMode::Normal,
            stroke_type: StrokeType::Text,
            text_content: Some(text),
            font_size,
        }
    }

    pub fn add_point(&mut self, point: Point) {
        self.points.push(point);
    }

    pub fn prune_laser_points(&mut self, now_ms: u64, max_age_ms: u64) {
        if self.stroke_type == StrokeType::Laser {
            self.points.retain(|p| now_ms >= p.timestamp_ms && (now_ms - p.timestamp_ms) <= max_age_ms);
        }
    }

    pub fn smoothed_points(&self, steps_per_segment: usize) -> Vec<Point> {
        if self.points.len() < 3 {
            return self.points.clone();
        }

        let n = self.points.len();
        let mut smoothed = Vec::new();

        for i in 0..n - 1 {
            let p0 = if i == 0 {
                Point {
                    x: 2.0 * self.points[0].x - self.points[1].x,
                    y: 2.0 * self.points[0].y - self.points[1].y,
                    pressure: 2.0 * self.points[0].pressure - self.points[1].pressure,
                    timestamp_ms: self.points[0].timestamp_ms,
                }
            } else {
                self.points[i - 1]
            };

            let p1 = self.points[i];
            let p2 = self.points[i + 1];

            let p3 = if i == n - 2 {
                Point {
                    x: 2.0 * self.points[n - 1].x - self.points[n - 2].x,
                    y: 2.0 * self.points[n - 1].y - self.points[n - 2].y,
                    pressure: 2.0 * self.points[n - 1].pressure - self.points[n - 2].pressure,
                    timestamp_ms: self.points[n - 1].timestamp_ms,
                }
            } else {
                self.points[i + 2]
            };

            for step in 0..steps_per_segment {
                let t = step as f32 / steps_per_segment as f32;
                let x = catmull_rom(p0.x, p1.x, p2.x, p3.x, t);
                let y = catmull_rom(p0.y, p1.y, p2.y, p3.y, t);
                let pressure = catmull_rom(p0.pressure, p1.pressure, p2.pressure, p3.pressure, t).clamp(0.0, 1.0);

                let t_ms = p1.timestamp_ms as f64
                    + (p2.timestamp_ms as f64 - p1.timestamp_ms as f64) * (t as f64);

                smoothed.push(Point {
                    x,
                    y,
                    pressure,
                    timestamp_ms: t_ms as u64,
                });
            }
        }

        if let Some(&last) = self.points.last() {
            smoothed.push(last);
        }

        smoothed
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    AddStroke(Stroke),
    Clear(Vec<Stroke>),
}

pub struct Canvas {
    width: u32,
    height: u32,
    strokes: Vec<Stroke>,
    current_stroke: Option<Stroke>,
    undo_stack: Vec<Command>,
    redo_stack: Vec<Command>,
    pub background_mode: BackgroundMode,
    pub scale_factor: f32,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            strokes: Vec::new(),
            current_stroke: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            background_mode: BackgroundMode::Transparent,
            scale_factor: 1.0,
        }
    }

    pub fn set_scale_factor(&mut self, scale: f32) {
        self.scale_factor = scale.max(0.5);
    }

    pub fn cycle_background_mode(&mut self) -> BackgroundMode {
        self.background_mode = match self.background_mode {
            BackgroundMode::Transparent => BackgroundMode::Blackboard,
            BackgroundMode::Blackboard => BackgroundMode::Whiteboard,
            BackgroundMode::Whiteboard => BackgroundMode::Transparent,
        };
        self.background_mode
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn start_stroke(&mut self, stroke: Stroke) {
        if let Some(unfinished) = self.current_stroke.take() {
            if !unfinished.points.is_empty() && unfinished.stroke_type != StrokeType::Laser && unfinished.stroke_type != StrokeType::Spotlight {
                self.strokes.push(unfinished.clone());
                self.undo_stack.push(Command::AddStroke(unfinished));
            }
        }
        self.current_stroke = Some(stroke);
        self.redo_stack.clear();
    }

    pub fn add_point_to_current_stroke(&mut self, point: Point) {
        if let Some(ref mut stroke) = self.current_stroke {
            stroke.add_point(point);
        }
    }

    pub fn finish_current_stroke(&mut self) {
        if let Some(stroke) = self.current_stroke.take() {
            if !stroke.points.is_empty() && stroke.stroke_type != StrokeType::Laser && stroke.stroke_type != StrokeType::Spotlight {
                self.strokes.push(stroke.clone());
                self.undo_stack.push(Command::AddStroke(stroke));
            }
        }
    }

    pub fn cancel_current_stroke(&mut self) {
        self.current_stroke = None;
    }

    pub fn clear(&mut self) {
        if !self.strokes.is_empty() {
            let removed = std::mem::take(&mut self.strokes);
            self.undo_stack.push(Command::Clear(removed));
            self.redo_stack.clear();
        }
        self.current_stroke = None;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(cmd) = self.undo_stack.pop() {
            match cmd.clone() {
                Command::AddStroke(_) => {
                    self.strokes.pop();
                }
                Command::Clear(ref strokes) => {
                    self.strokes = strokes.clone();
                }
            }
            self.redo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(cmd) = self.redo_stack.pop() {
            match cmd.clone() {
                Command::AddStroke(ref stroke) => {
                    self.strokes.push(stroke.clone());
                }
                Command::Clear(_) => {
                    self.strokes.clear();
                }
            }
            self.undo_stack.push(cmd);
            true
        } else {
            false
        }
    }

    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }

    pub fn current_stroke(&self) -> Option<&Stroke> {
        self.current_stroke.as_ref()
    }

    pub fn current_stroke_mut(&mut self) -> Option<&mut Stroke> {
        self.current_stroke.as_mut()
    }

    pub fn render_background(&self, pixmap: &mut tiny_skia::Pixmap) {
        let is_spotlight = self.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Spotlight);

        if is_spotlight {
            pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 180));
        } else {
            match self.background_mode {
                BackgroundMode::Transparent => {
                    pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0));
                }
                BackgroundMode::Blackboard => {
                    pixmap.fill(tiny_skia::Color::from_rgba8(24, 24, 28, 255));
                }
                BackgroundMode::Whiteboard => {
                    pixmap.fill(tiny_skia::Color::from_rgba8(250, 250, 250, 255));
                }
            }
        }
    }

    pub fn render(&mut self, pixmap: &mut tiny_skia::Pixmap) {
        self.render_background(pixmap);
        self.render_completed_strokes(pixmap);
        self.render_current_stroke(pixmap);
    }

    pub fn render_completed_strokes(&self, pixmap: &mut tiny_skia::Pixmap) {
        for stroke in &self.strokes {
            render_stroke(stroke, pixmap);
        }
    }

    pub fn render_current_stroke(&mut self, pixmap: &mut tiny_skia::Pixmap) -> bool {
        let now_ms = current_time_ms();
        let mut has_active_laser = false;

        if let Some(ref mut stroke) = self.current_stroke {
            if stroke.stroke_type == StrokeType::Laser {
                stroke.prune_laser_points(now_ms, 1200);
                has_active_laser = !stroke.points.is_empty();
                render_laser_stroke(stroke, now_ms, pixmap);
            } else if stroke.stroke_type == StrokeType::Spotlight {
                render_spotlight_stroke(stroke, pixmap);
            } else {
                render_stroke(stroke, pixmap);
            }
        }
        has_active_laser
    }
}

fn render_stroke(stroke: &Stroke, pixmap: &mut tiny_skia::Pixmap) {
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
            let smoothed = stroke.smoothed_points(5);
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
            let x = f32::min(p1.x, p2.x);
            let y = f32::min(p1.y, p2.y);
            let w = (p1.x - p2.x).abs();
            let h = (p1.y - p2.y).abs();
            if w > 0.5 && h > 0.5 {
                pb.move_to(x, y);
                pb.line_to(x + w, y);
                pb.line_to(x + w, y + h);
                pb.line_to(x, y + h);
                pb.close();
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
        skia_stroke.line_cap = tiny_skia::LineCap::Round;
        skia_stroke.line_join = tiny_skia::LineJoin::Round;

        pixmap.stroke_path(&path, &paint, &skia_stroke, tiny_skia::Transform::identity(), None);
    }
}

fn render_laser_stroke(stroke: &Stroke, now_ms: u64, pixmap: &mut tiny_skia::Pixmap) {
    if stroke.points.len() < 2 {
        return;
    }

    let max_age = 1200.0; // 1.2 seconds decay
    let points = &stroke.points;

    let mut pb = tiny_skia::PathBuilder::new();
    let mut prev_pt: Option<Point> = None;

    for i in 0..points.len() {
        let pt = points[i];
        let age = (now_ms.saturating_sub(pt.timestamp_ms)) as f32;
        if age > max_age {
            prev_pt = None;
            continue;
        }

        if let Some(p0) = prev_pt {
            pb.move_to(p0.x, p0.y);
            pb.line_to(pt.x, pt.y);
        }
        prev_pt = Some(pt);
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

fn render_spotlight_stroke(stroke: &Stroke, pixmap: &mut tiny_skia::Pixmap) {
    if let Some(&p) = stroke.points.last() {
        let cx = p.x;
        let cy = p.y;
        let r = stroke.width;

        // Cut out circular spotlight
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

            // Ring border outline
            let mut ring_paint = tiny_skia::Paint::default();
            ring_paint.set_color(tiny_skia::Color::from_rgba8(50, 130, 245, 230));
            ring_paint.anti_alias = true;

            let mut ring_stroke = tiny_skia::Stroke::default();
            ring_stroke.width = 3.0;
            pixmap.stroke_path(&cpath, &ring_paint, &ring_stroke, tiny_skia::Transform::identity(), None);
        }
    }
}

fn render_text_to_pixmap(
    text: &str,
    start_x: f32,
    start_y: f32,
    font_size: f32,
    color: Color,
    blend_mode: BlendMode,
    pixmap: &mut tiny_skia::Pixmap,
) {
    if let Some(font) = get_system_font() {
        let mut cur_x = start_x;
        let baseline_y = start_y + font_size * 0.8;

        for ch in text.chars() {
            let (metrics, bitmap) = font.rasterize(ch, font_size);
            if metrics.width > 0 && metrics.height > 0 {
                let gx = cur_x + metrics.bounds.xmin;
                let gy = baseline_y - metrics.bounds.ymin - metrics.height as f32;

                for row in 0..metrics.height {
                    for col in 0..metrics.width {
                        let alpha_coverage = bitmap[row * metrics.width + col];
                        if alpha_coverage > 0 {
                            let px = gx + col as f32;
                            let py = gy + row as f32;

                            if px >= 0.0 && px < pixmap.width() as f32 && py >= 0.0 && py < pixmap.height() as f32 {
                                let combined_a = ((color.a as u16 * alpha_coverage as u16) / 255) as u8;
                                if combined_a > 0 {
                                    let mut paint = tiny_skia::Paint::default();
                                    paint.set_color(tiny_skia::Color::from_rgba8(color.r, color.g, color.b, combined_a));
                                    paint.blend_mode = blend_mode.into();

                                    if let Some(rect) = tiny_skia::Rect::from_xywh(px, py, 1.0, 1.0) {
                                        pixmap.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cur_x += metrics.advance_width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_creation_and_resize() {
        let mut canvas = Canvas::new(800, 600);
        assert_eq!(canvas.width(), 800);
        assert_eq!(canvas.height(), 600);

        canvas.resize(1024, 768);
        assert_eq!(canvas.width(), 1024);
        assert_eq!(canvas.height(), 768);
    }

    #[test]
    fn test_drawing_and_undo_redo() {
        let mut canvas = Canvas::new(800, 600);

        let stroke = Stroke::new(Color::new(255, 0, 0, 255), 5.0, BlendMode::Normal);
        canvas.start_stroke(stroke);
        canvas.add_point_to_current_stroke(Point::new(10.0, 10.0, 1.0, 0));
        canvas.add_point_to_current_stroke(Point::new(20.0, 20.0, 1.0, 10));
        canvas.finish_current_stroke();

        assert_eq!(canvas.strokes().len(), 1);
        assert!(canvas.current_stroke().is_none());

        assert!(canvas.undo());
        assert_eq!(canvas.strokes().len(), 0);

        assert!(canvas.redo());
        assert_eq!(canvas.strokes().len(), 1);
    }

    #[test]
    fn test_laser_decay_pruning() {
        let mut stroke = Stroke::new_shape(
            Color::new(255, 0, 100, 255),
            8.0,
            BlendMode::Normal,
            StrokeType::Laser,
        );

        stroke.add_point(Point::new(10.0, 10.0, 1.0, 1000));
        stroke.add_point(Point::new(20.0, 20.0, 1.0, 2000));
        stroke.add_point(Point::new(30.0, 30.0, 1.0, 3000));

        stroke.prune_laser_points(4000, 1200);

        assert_eq!(stroke.points.len(), 1);
        assert_eq!(stroke.points[0].timestamp_ms, 3000);
    }

    #[test]
    fn test_background_mode_cycling() {
        let mut canvas = Canvas::new(800, 600);
        assert_eq!(canvas.background_mode, BackgroundMode::Transparent);

        assert_eq!(canvas.cycle_background_mode(), BackgroundMode::Blackboard);
        assert_eq!(canvas.cycle_background_mode(), BackgroundMode::Whiteboard);
        assert_eq!(canvas.cycle_background_mode(), BackgroundMode::Transparent);
    }
}
