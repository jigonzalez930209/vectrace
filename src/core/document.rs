use crate::core::canvas::{BackgroundMode, Canvas, Stroke, StrokeType};
use crate::core::render::render_stroke;
use tiny_skia::Pixmap;

/// An immutable, point-in-time snapshot of the vector document for rendering and export.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub background_mode: BackgroundMode,
    pub strokes: Vec<Stroke>,
    pub revision: u64,
}

impl DocumentSnapshot {
    /// Renders the document background onto a pixmap.
    pub fn render_background(&self, pixmap: &mut Pixmap) {
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

    /// Renders annotations onto the provided tiny-skia Pixmap.
    /// If `include_background` is false, the canvas is initialized with transparent alpha.
    pub fn render(&self, pixmap: &mut Pixmap, include_background: bool) {
        if include_background {
            self.render_background(pixmap);
        } else {
            pixmap.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 0));
        }

        for stroke in &self.strokes {
            // Exclude transient laser and spotlight effects from clean snapshots by default
            if stroke.stroke_type != StrokeType::Laser && stroke.stroke_type != StrokeType::Spotlight {
                render_stroke(stroke, pixmap);
            }
        }
    }
}

impl Canvas {
    /// Creates an immutable point-in-time snapshot of the current canvas state.
    pub fn snapshot(&self) -> DocumentSnapshot {
        let mut snapshot_strokes = self.strokes().to_vec();
        if let Some(curr) = self.current_stroke() {
            if !curr.points.is_empty()
                && curr.stroke_type != StrokeType::Laser
                && curr.stroke_type != StrokeType::Spotlight
            {
                snapshot_strokes.push(curr.clone());
            }
        }

        DocumentSnapshot {
            width: self.width(),
            height: self.height(),
            scale_factor: self.scale_factor,
            background_mode: self.background_mode,
            strokes: snapshot_strokes,
            revision: self.revision(),
        }
    }
}
