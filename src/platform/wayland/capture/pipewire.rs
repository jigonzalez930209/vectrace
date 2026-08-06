use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::{CapturePixelFormat, CapturedFrame, FrameMemory, OutputTransform};
use crate::snapshot::request::OutputId;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

pub struct PipeWireStreamReader {
    pipewire_fd: OwnedFd,
    node_id: u32,
    sequence: u64,
}

impl PipeWireStreamReader {
    pub fn new(pipewire_fd: OwnedFd, node_id: u32) -> Self {
        pipewire::init();
        Self {
            pipewire_fd,
            node_id,
            sequence: 0,
        }
    }

    pub fn acquire_frame(
        &mut self,
        _deadline: Instant,
        width: u32,
        height: u32,
        format: CapturePixelFormat,
    ) -> Result<CapturedFrame, CaptureError> {
        self.sequence += 1;
        let stride = width * 4;

        let latest_frame: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let frame_clone = latest_frame.clone();

        let mainloop = pipewire::main_loop::MainLoop::new(None)
            .map_err(|e| CaptureError::new(CaptureErrorKind::PipeWireUnavailable, format!("Failed mainloop: {}", e)))?;
        let context = pipewire::context::Context::new(&mainloop)
            .map_err(|e| CaptureError::new(CaptureErrorKind::PipeWireUnavailable, format!("Failed context: {}", e)))?;

        let fd = self.pipewire_fd.try_clone()
            .map_err(|e| CaptureError::new(CaptureErrorKind::PipeWireUnavailable, format!("Failed clone fd: {}", e)))?;
        let core = context.connect_fd(fd, None)
            .map_err(|e| CaptureError::new(CaptureErrorKind::PipeWireUnavailable, format!("Failed connect_fd: {}", e)))?;

        let stream = pipewire::stream::Stream::new(
            &core,
            "vectrace-screen-capture",
            pipewire::properties::properties! {
                *pipewire::keys::MEDIA_TYPE => "Video",
                *pipewire::keys::MEDIA_CATEGORY => "Capture",
                *pipewire::keys::MEDIA_ROLE => "Screen",
            },
        ).map_err(|e| CaptureError::new(CaptureErrorKind::PipeWireUnavailable, format!("Failed stream: {}", e)))?;

        let _listener = stream
            .add_local_listener::<()>()
            .process(move |stream, _| {
                if let Some(mut buffer) = stream.dequeue_buffer() {
                    let datas = buffer.datas_mut();
                    if !datas.is_empty() {
                        let data = &mut datas[0];
                        let chunk = data.chunk();
                        let size = chunk.size() as usize;
                        if size >= (width * height * 4) as usize {
                            if let Some(map) = data.data() {
                                let mut vec = vec![0u8; size];
                                vec.copy_from_slice(&map[..size]);
                                let mut lock = frame_clone.lock().unwrap();
                                *lock = Some(vec);
                            }
                        }
                    }
                }
            })
            .register();

        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(150) {
            let loop_ref = mainloop.loop_();
            let _ = loop_ref.iterate(Duration::from_millis(10));
            let lock = latest_frame.lock().unwrap();
            if lock.is_some() {
                break;
            }
        }

        let raw_bytes = latest_frame.lock().unwrap().take()
            .ok_or_else(|| CaptureError::new(CaptureErrorKind::PipeWireUnavailable, "Timeout waiting for PipeWire frame"))?;

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);

        Ok(CapturedFrame {
            output: OutputId(self.node_id),
            width,
            height,
            stride: stride as usize,
            format,
            memory: FrameMemory::Owned(raw_bytes),
            transform: OutputTransform::Normal,
            sequence: self.sequence,
            timestamp,
            damage: vec![],
        })
    }
}
