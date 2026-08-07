use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::{CapturePixelFormat, CapturedFrame, FrameMemory, OutputTransform};
use crate::snapshot::request::OutputId;
use pipewire::spa::param::video::VideoFormat;
use pipewire::spa::pod::Pod;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

struct NegotiatedFormat {
    width: u32,
    height: u32,
    format: CapturePixelFormat,
}

struct CapturedBuffer {
    bytes: Vec<u8>,
    stride: usize,
}

fn video_format_to_capture(format: VideoFormat) -> Option<CapturePixelFormat> {
    match format {
        VideoFormat::BGRA => Some(CapturePixelFormat::Bgra8888),
        VideoFormat::BGRx => Some(CapturePixelFormat::Bgrx8888),
        VideoFormat::RGBA => Some(CapturePixelFormat::Rgba8888),
        VideoFormat::RGBx => Some(CapturePixelFormat::Rgbx8888),
        _ => None,
    }
}

enum PipeWireRemote {
    /// XDG ScreenCast portal remote FD.
    PortalFd(OwnedFd),
    /// Default PipeWire session (Mutter ScreenCast publishes nodes here).
    Default,
}

pub struct PipeWireStreamReader {
    remote: PipeWireRemote,
    node_id: u32,
    sequence: u64,
}

impl PipeWireStreamReader {
    pub fn new(pipewire_fd: OwnedFd, node_id: u32) -> Self {
        pipewire::init();
        Self {
            remote: PipeWireRemote::PortalFd(pipewire_fd),
            node_id,
            sequence: 0,
        }
    }

    /// Connect to a PipeWire node on the default session bus (Mutter ScreenCast).
    pub fn from_node(node_id: u32) -> Self {
        pipewire::init();
        Self {
            remote: PipeWireRemote::Default,
            node_id,
            sequence: 0,
        }
    }

    pub fn acquire_frame(
        &mut self,
        deadline: Instant,
        width: u32,
        height: u32,
        format: CapturePixelFormat,
    ) -> Result<CapturedFrame, CaptureError> {
        self.sequence += 1;

        let latest_frame: Arc<Mutex<Option<CapturedBuffer>>> = Arc::new(Mutex::new(None));
        let negotiated: Arc<Mutex<Option<NegotiatedFormat>>> = Arc::new(Mutex::new(None));
        let frame_clone = latest_frame.clone();
        let negotiated_clone = negotiated.clone();

        let mainloop = pipewire::main_loop::MainLoop::new(None).map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::PipeWireUnavailable,
                format!("Failed mainloop: {}", e),
            )
        })?;
        let context = pipewire::context::Context::new(&mainloop).map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::PipeWireUnavailable,
                format!("Failed context: {}", e),
            )
        })?;

        let core = match &self.remote {
            PipeWireRemote::PortalFd(owned) => {
                let fd = owned.try_clone().map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PipeWireUnavailable,
                        format!("Failed clone fd: {}", e),
                    )
                })?;
                context.connect_fd(fd, None).map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PipeWireUnavailable,
                        format!("Failed connect_fd: {}", e),
                    )
                })?
            }
            PipeWireRemote::Default => context.connect(None).map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::PipeWireUnavailable,
                    format!("Failed connect to default PipeWire: {}", e),
                )
            })?,
        };

        let stream = pipewire::stream::Stream::new(
            &core,
            "vectrace-screen-capture",
            pipewire::properties::properties! {
                *pipewire::keys::MEDIA_TYPE => "Video",
                *pipewire::keys::MEDIA_CATEGORY => "Capture",
                *pipewire::keys::MEDIA_ROLE => "Screen",
            },
        )
        .map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::PipeWireUnavailable,
                format!("Failed stream: {}", e),
            )
        })?;

        let _listener = stream
            .add_local_listener::<()>()
            .param_changed(move |_stream, _user_data, id, param| {
                let Some(param) = param else {
                    return;
                };
                if id != pipewire::spa::param::ParamType::Format.as_raw() {
                    return;
                }

                let (media_type, media_subtype) =
                    match pipewire::spa::param::format_utils::parse_format(param) {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                if media_type != pipewire::spa::param::format::MediaType::Video
                    || media_subtype != pipewire::spa::param::format::MediaSubtype::Raw
                {
                    return;
                }

                let mut info = pipewire::spa::param::video::VideoInfoRaw::default();
                if info.parse(param).is_err() {
                    return;
                }

                let Some(pixel_format) = video_format_to_capture(info.format()) else {
                    return;
                };

                let size = info.size();
                if size.width == 0 || size.height == 0 {
                    return;
                }

                if let Ok(mut lock) = negotiated_clone.lock() {
                    *lock = Some(NegotiatedFormat {
                        width: size.width,
                        height: size.height,
                        format: pixel_format,
                    });
                }
            })
            .process(move |stream, _| {
                if let Some(mut buffer) = stream.dequeue_buffer() {
                    let datas = buffer.datas_mut();
                    if !datas.is_empty() {
                        let data = &mut datas[0];
                        let chunk = data.chunk();
                        let size = chunk.size() as usize;
                        let stride = chunk.stride().max(0) as usize;
                        if size > 0 {
                            if let Some(map) = data.data() {
                                let copy_len = size.min(map.len());
                                if copy_len > 0 {
                                    let mut bytes = vec![0u8; copy_len];
                                    bytes.copy_from_slice(&map[..copy_len]);
                                    if let Ok(mut lock) = frame_clone.lock() {
                                        *lock = Some(CapturedBuffer { bytes, stride });
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .register()
            .map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::PipeWireUnavailable,
                    format!("Failed to register stream listener: {}", e),
                )
            })?;

        let obj = pipewire::spa::pod::object!(
            pipewire::spa::utils::SpaTypes::ObjectParamFormat,
            pipewire::spa::param::ParamType::EnumFormat,
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaType,
                Id,
                pipewire::spa::param::format::MediaType::Video
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::MediaSubtype,
                Id,
                pipewire::spa::param::format::MediaSubtype::Raw
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::VideoFormat,
                Choice,
                Enum,
                Id,
                pipewire::spa::param::video::VideoFormat::BGRx,
                pipewire::spa::param::video::VideoFormat::BGRx,
                pipewire::spa::param::video::VideoFormat::BGRA,
                pipewire::spa::param::video::VideoFormat::RGBx,
                pipewire::spa::param::video::VideoFormat::RGBA,
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::VideoSize,
                Choice,
                Range,
                Rectangle,
                pipewire::spa::utils::Rectangle {
                    width: width.max(1),
                    height: height.max(1),
                },
                pipewire::spa::utils::Rectangle {
                    width: 1,
                    height: 1,
                },
                pipewire::spa::utils::Rectangle {
                    width: 8192,
                    height: 8192,
                }
            ),
            pipewire::spa::pod::property!(
                pipewire::spa::param::format::FormatProperties::VideoFramerate,
                Choice,
                Range,
                Fraction,
                pipewire::spa::utils::Fraction { num: 30, denom: 1 },
                pipewire::spa::utils::Fraction { num: 0, denom: 1 },
                pipewire::spa::utils::Fraction {
                    num: 1000,
                    denom: 1
                }
            ),
        );

        let values: Vec<u8> = pipewire::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &pipewire::spa::pod::Value::Object(obj),
        )
        .map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::PipeWireNegotiationFailed,
                format!("Failed to serialize EnumFormat: {}", e),
            )
        })?
        .0
        .into_inner();

        let mut params = [Pod::from_bytes(&values).ok_or_else(|| {
            CaptureError::new(
                CaptureErrorKind::PipeWireNegotiationFailed,
                "Failed to build EnumFormat pod",
            )
        })?];

        stream
            .connect(
                pipewire::spa::utils::Direction::Input,
                Some(self.node_id),
                pipewire::stream::StreamFlags::AUTOCONNECT | pipewire::stream::StreamFlags::MAP_BUFFERS,
                &mut params,
            )
            .map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::PipeWireUnavailable,
                    format!("Failed to connect PipeWire stream to node {}: {}", self.node_id, e),
                )
            })?;

        // Mutter is silent; for XDG portal we still settle briefly in case a picker was visible.
        let settle = match self.remote {
            PipeWireRemote::Default => Duration::from_millis(80),
            PipeWireRemote::PortalFd(_) => Duration::from_millis(350),
        };
        let mut first_frame_at: Option<Instant> = None;

        while Instant::now() < deadline {
            let loop_ref = mainloop.loop_();
            let _ = loop_ref.iterate(Duration::from_millis(10));
            let has_frame = latest_frame.lock().map(|lock| lock.is_some()).unwrap_or(false);
            if has_frame {
                let first = *first_frame_at.get_or_insert_with(Instant::now);
                if Instant::now().saturating_duration_since(first) >= settle {
                    break;
                }
            }
        }

        let captured = latest_frame.lock().unwrap().take().ok_or_else(|| {
            CaptureError::new(
                CaptureErrorKind::PipeWireUnavailable,
                "Timeout waiting for PipeWire frame",
            )
        })?;

        let (out_width, out_height, out_format) = negotiated
            .lock()
            .ok()
            .and_then(|lock| lock.as_ref().map(|n| (n.width, n.height, n.format)))
            .unwrap_or((width, height, format));

        let default_stride = (out_width as usize).saturating_mul(4);
        let stride = if captured.stride >= default_stride {
            captured.stride
        } else {
            default_stride
        };

        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);

        Ok(CapturedFrame {
            output: OutputId(self.node_id),
            width: out_width,
            height: out_height,
            stride,
            format: out_format,
            memory: FrameMemory::Owned(captured.bytes),
            transform: OutputTransform::Normal,
            sequence: self.sequence,
            timestamp,
            damage: vec![],
        })
    }
}
