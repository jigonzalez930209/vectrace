use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::OwnedFd;

pub struct SharedMemoryBufferReader;

impl SharedMemoryBufferReader {
    pub fn read_frame_bytes(
        fd: &OwnedFd,
        offset: u32,
        stride: u32,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, CaptureError> {
        let width_us = width as usize;
        let height_us = height as usize;
        let stride_us = stride as usize;
        let offset_us = offset as usize;

        if width == 0 || height == 0 || stride < width * 4 {
            return Err(CaptureError::new(
                CaptureErrorKind::UnsupportedBufferType,
                format!(
                    "Invalid frame dimensions or stride: width={}, height={}, stride={}",
                    width, height, stride
                ),
            ));
        }

        let required_min_len = (height_us - 1)
            .checked_mul(stride_us)
            .and_then(|val| val.checked_add(width_us * 4))
            .ok_or_else(|| {
                CaptureError::new(
                    CaptureErrorKind::UnsupportedBufferType,
                    "Overflow calculating required frame buffer size",
                )
            })?;

        let dup_fd = fd.try_clone().map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::Io,
                format!("Failed to clone MemFd file descriptor: {}", e),
            )
        })?;

        let mut file = File::from(dup_fd);

        let file_len = file.seek(SeekFrom::End(0)).map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::Io,
                format!("Failed to seek end of MemFd: {}", e),
            )
        })? as usize;

        if file_len < offset_us + required_min_len {
            return Err(CaptureError::new(
                CaptureErrorKind::UnsupportedBufferType,
                format!(
                    "MemFd size {} is smaller than required offset {} + len {}",
                    file_len, offset_us, required_min_len
                ),
            ));
        }

        file.seek(SeekFrom::Start(offset as u64)).map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::Io,
                format!("Failed to seek offset in MemFd: {}", e),
            )
        })?;

        let mut buffer = vec![0u8; required_min_len];
        file.read_exact(&mut buffer).map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::Io,
                format!("Failed to read frame bytes from MemFd: {}", e),
            )
        })?;

        Ok(buffer)
    }
}
