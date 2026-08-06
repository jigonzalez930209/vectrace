use crate::snapshot::request::OutputId;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTransform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    FlippedRotate90,
    FlippedRotate180,
    FlippedRotate270,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePixelFormat {
    Rgba8888,
    Rgbx8888,
    Bgra8888,
    Bgrx8888,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

use std::os::fd::OwnedFd;

#[derive(Debug)]
pub struct DmaBufFrame {
    pub fd: OwnedFd,
    pub drm_format: u32,
    pub modifier: u64,
    pub plane_offsets: Vec<usize>,
    pub plane_strides: Vec<usize>,
}

#[derive(Debug)]
pub enum FrameMemory {
    Owned(Vec<u8>),
    DmaBuf(DmaBufFrame),
}

#[derive(Debug)]
pub struct CapturedFrame {
    pub output: OutputId,
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub format: CapturePixelFormat,
    pub memory: FrameMemory,
    pub transform: OutputTransform,
    pub sequence: u64,
    pub timestamp: Duration,
    pub damage: Vec<PixelRect>,
}
