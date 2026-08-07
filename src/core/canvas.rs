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
        Self { x, y, pressure, timestamp_ms }
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
            BlendMode::Normal   => tiny_skia::BlendMode::SourceOver,
            BlendMode::Multiply => tiny_skia::BlendMode::Multiply,
            BlendMode::Screen   => tiny_skia::BlendMode::Screen,
            BlendMode::Overlay  => tiny_skia::BlendMode::Overlay,
            BlendMode::Darken   => tiny_skia::BlendMode::Darken,
            BlendMode::Lighten  => tiny_skia::BlendMode::Lighten,
            BlendMode::Clear    => tiny_skia::BlendMode::Clear,
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
                let t_ms = p1.timestamp_ms as f64 + (p2.timestamp_ms as f64 - p1.timestamp_ms as f64) * (t as f64);
                smoothed.push(Point { x, y, pressure, timestamp_ms: t_ms as u64 });
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
    revision: u64,
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
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 { self.revision }
    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn strokes(&self) -> &[Stroke] { &self.strokes }
    pub fn current_stroke(&self) -> Option<&Stroke> { self.current_stroke.as_ref() }
    pub fn current_stroke_mut(&mut self) -> Option<&mut Stroke> { self.current_stroke.as_mut() }

    pub fn set_scale_factor(&mut self, scale: f32) {
        self.scale_factor = scale.max(0.5);
    }

    pub fn cycle_background_mode(&mut self) -> BackgroundMode {
        self.background_mode = match self.background_mode {
            BackgroundMode::Transparent => BackgroundMode::Blackboard,
            BackgroundMode::Blackboard  => BackgroundMode::Whiteboard,
            BackgroundMode::Whiteboard  => BackgroundMode::Transparent,
        };
        self.revision += 1;
        self.background_mode
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn start_stroke(&mut self, stroke: Stroke) {
        if let Some(unfinished) = self.current_stroke.take() {
            if !unfinished.points.is_empty()
                && unfinished.stroke_type != StrokeType::Laser
                && unfinished.stroke_type != StrokeType::Spotlight
            {
                self.strokes.push(unfinished.clone());
                self.undo_stack.push(Command::AddStroke(unfinished));
            }
        }
        self.current_stroke = Some(stroke);
        self.redo_stack.clear();
        self.revision += 1;
    }

    pub fn add_point_to_current_stroke(&mut self, point: Point) {
        if let Some(ref mut stroke) = self.current_stroke {
            stroke.add_point(point);
            self.revision += 1;
        }
    }

    pub fn finish_current_stroke(&mut self) {
        if let Some(stroke) = self.current_stroke.take() {
            if !stroke.points.is_empty()
                && stroke.stroke_type != StrokeType::Laser
                && stroke.stroke_type != StrokeType::Spotlight
            {
                self.strokes.push(stroke.clone());
                self.undo_stack.push(Command::AddStroke(stroke));
            }
        }
        self.revision += 1;
    }

    pub fn cancel_current_stroke(&mut self) {
        self.current_stroke = None;
        self.revision += 1;
    }

    pub fn clear(&mut self) {
        if !self.strokes.is_empty() {
            let removed = std::mem::take(&mut self.strokes);
            self.undo_stack.push(Command::Clear(removed));
            self.redo_stack.clear();
            self.revision += 1;
        }
        self.current_stroke = None;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(cmd) = self.undo_stack.pop() {
            match cmd.clone() {
                Command::AddStroke(_)       => { self.strokes.pop(); }
                Command::Clear(ref strokes) => { self.strokes = strokes.clone(); }
            }
            self.redo_stack.push(cmd);
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(cmd) = self.redo_stack.pop() {
            match cmd.clone() {
                Command::AddStroke(ref stroke) => { self.strokes.push(stroke.clone()); }
                Command::Clear(_)              => { self.strokes.clear(); }
            }
            self.undo_stack.push(cmd);
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn render_background(&self, pixmap: &mut tiny_skia::Pixmap) {
        use crate::core::render::render_spotlight_stroke;
        let is_spotlight = self.current_stroke().map_or(false, |s| s.stroke_type == StrokeType::Spotlight);

        if is_spotlight {
            pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 180));
        } else {
            match self.background_mode {
                BackgroundMode::Transparent => pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0)),
                BackgroundMode::Blackboard  => pixmap.fill(tiny_skia::Color::from_rgba8(24, 24, 28, 255)),
                BackgroundMode::Whiteboard  => pixmap.fill(tiny_skia::Color::from_rgba8(250, 250, 250, 255)),
            }
        }
        let _ = render_spotlight_stroke; // suppress unused warning
    }

    pub fn render(&mut self, pixmap: &mut tiny_skia::Pixmap) {
        self.render_background(pixmap);
        self.render_completed_strokes(pixmap);
        self.render_current_stroke(pixmap);
    }

    pub fn render_completed_strokes(&self, pixmap: &mut tiny_skia::Pixmap) {
        use crate::core::render::render_stroke;
        for stroke in &self.strokes {
            render_stroke(stroke, pixmap);
        }
    }

    pub fn render_current_stroke(&mut self, pixmap: &mut tiny_skia::Pixmap) -> bool {
        use crate::core::render::{render_stroke, render_laser_stroke, render_spotlight_stroke};
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
            Color::new(255, 0, 100, 255), 8.0, BlendMode::Normal, StrokeType::Laser,
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

    #[test]
    fn test_canvas_scale_factor() {
        let mut canvas = Canvas::new(800, 600);
        assert_eq!(canvas.scale_factor, 1.0);
        canvas.set_scale_factor(2.0);
        assert_eq!(canvas.scale_factor, 2.0);
    }
}
