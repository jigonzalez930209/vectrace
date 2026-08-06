use crate::snapshot::frame::CapturePixelFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaVideoFormat {
    Bgra,
    Bgrx,
    Rgba,
    Rgbx,
    Unknown(u32),
}

impl SpaVideoFormat {
    pub const SPA_VIDEO_FORMAT_BGRA: u32 = 6;
    pub const SPA_VIDEO_FORMAT_BGRX: u32 = 7;
    pub const SPA_VIDEO_FORMAT_RGBA: u32 = 12;
    pub const SPA_VIDEO_FORMAT_RGBX: u32 = 14;

    pub fn from_spa_id(id: u32) -> Self {
        match id {
            Self::SPA_VIDEO_FORMAT_BGRA => Self::Bgra,
            Self::SPA_VIDEO_FORMAT_BGRX => Self::Bgrx,
            Self::SPA_VIDEO_FORMAT_RGBA => Self::Rgba,
            Self::SPA_VIDEO_FORMAT_RGBX => Self::Rgbx,
            other => Self::Unknown(other),
        }
    }

    pub fn to_capture_format(&self) -> Option<CapturePixelFormat> {
        match self {
            Self::Bgra => Some(CapturePixelFormat::Bgra8888),
            Self::Bgrx => Some(CapturePixelFormat::Bgrx8888),
            Self::Rgba => Some(CapturePixelFormat::Rgba8888),
            Self::Rgbx => Some(CapturePixelFormat::Rgbx8888),
            Self::Unknown(_) => None,
        }
    }
}
