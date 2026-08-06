use crate::snapshot::request::CursorPolicy;

#[derive(Debug, Clone)]
pub struct CaptureCapabilities {
    pub backend_name: String,
    pub supports_clean_composite: bool,
    pub supports_visible_composition: bool,
    pub supports_desktop_only: bool,
    pub supports_all_monitors: bool,
    pub supported_cursor_policies: Vec<CursorPolicy>,
}

impl Default for CaptureCapabilities {
    fn default() -> Self {
        Self {
            backend_name: "AnnotationOnly".to_string(),
            supports_clean_composite: false,
            supports_visible_composition: false,
            supports_desktop_only: false,
            supports_all_monitors: false,
            supported_cursor_policies: vec![CursorPolicy::Hidden],
        }
    }
}
