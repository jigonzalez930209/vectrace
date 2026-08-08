//! Persistent PipeWire capture pump — keeps streams connected between Saves.
//!
//! Reconnecting MainLoop+Stream per screenshot costs ~1–2s per monitor and is
//! what made warm Mutter sessions still feel like a ~4s freeze.

use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::CapturePixelFormat;
use pipewire::spa::param::video::VideoFormat;
use pipewire::spa::pod::Pod;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

fn video_format_to_capture(format: VideoFormat) -> Option<CapturePixelFormat> {
    match format {
        VideoFormat::BGRA => Some(CapturePixelFormat::Bgra8888),
        VideoFormat::BGRx => Some(CapturePixelFormat::Bgrx8888),
        VideoFormat::RGBA => Some(CapturePixelFormat::Rgba8888),
        VideoFormat::RGBx => Some(CapturePixelFormat::Rgbx8888),
        _ => None,
    }
}

#[derive(Clone)]
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub format: CapturePixelFormat,
    pub bytes: Arc<[u8]>,
    pub captured_at: Instant,
}

struct SlotState {
    latest: Option<Arc<RawFrame>>,
}

/// Background PipeWire mainloop owning one Input stream per ScreenCast node.
pub struct PersistentPipeWirePump {
    slots: Vec<Arc<Mutex<SlotState>>>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl PersistentPipeWirePump {
    /// Connect to `nodes` (node_id, hint_w, hint_h) and keep streaming until dropped.
    pub fn start(nodes: Vec<(u32, u32, u32)>) -> Result<Self, CaptureError> {
        if nodes.is_empty() {
            return Err(CaptureError::new(
                CaptureErrorKind::PipeWireUnavailable,
                "No PipeWire nodes to attach",
            ));
        }

        pipewire::init();

        let slots: Vec<Arc<Mutex<SlotState>>> = nodes
            .iter()
            .map(|_| Arc::new(Mutex::new(SlotState { latest: None })))
            .collect();
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let slots_thread = slots.clone();
        let stop_thread = stop.clone();
        let nodes_thread = nodes;

        let join = thread::Builder::new()
            .name("vectrace-pw-pump".into())
            .spawn(move || {
                if let Err(e) = run_pump(nodes_thread, slots_thread, stop_thread, ready_tx) {
                    // ready_tx may already have fired Ok; ignore secondary send errors.
                    let _ = e;
                }
            })
            .map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::PipeWireUnavailable,
                    format!("Failed to spawn PipeWire pump: {}", e),
                )
            })?;

        match ready_rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                stop.store(true, Ordering::SeqCst);
                let _ = join.join();
                return Err(CaptureError::new(CaptureErrorKind::PipeWireUnavailable, e));
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                let _ = join.join();
                return Err(CaptureError::new(
                    CaptureErrorKind::PipeWireUnavailable,
                    "Timeout starting PipeWire pump",
                ));
            }
        }

        // Wait briefly for the first buffers so the first Save is also warm.
        let boot = Instant::now();
        while boot.elapsed() < Duration::from_millis(800) {
            if slots.iter().all(|s| {
                s.lock()
                    .map(|g| g.latest.is_some())
                    .unwrap_or(false)
            }) {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        println!(
            "PipeWire pump live ({} stream(s)) — frames update continuously",
            slots.len()
        );

        Ok(Self {
            slots,
            stop,
            join: Some(join),
        })
    }

    /// Snapshot latest frames. Uses `Arc` clones (cheap) — no full buffer copies.
    /// If `not_before` is set, wait until each stream has a frame at/after that instant.
    pub fn snapshot(
        &self,
        not_before: Option<Instant>,
        max_wait: Duration,
    ) -> Result<Vec<Arc<RawFrame>>, CaptureError> {
        let deadline = Instant::now() + max_wait;
        loop {
            let mut frames = Vec::with_capacity(self.slots.len());
            let mut all_ok = true;
            for slot in &self.slots {
                let guard = slot.lock().map_err(|_| {
                    CaptureError::new(CaptureErrorKind::Internal, "pw slot lock poisoned")
                })?;
                match guard.latest.as_ref() {
                    Some(f)
                        if not_before
                            .map(|t| f.captured_at >= t)
                            .unwrap_or(true) =>
                    {
                        frames.push(Arc::clone(f));
                    }
                    _ => {
                        all_ok = false;
                        break;
                    }
                }
            }
            if all_ok && frames.len() == self.slots.len() {
                return Ok(frames);
            }
            if Instant::now() >= deadline {
                let mut fallback = Vec::with_capacity(self.slots.len());
                for slot in &self.slots {
                    let guard = slot.lock().map_err(|_| {
                        CaptureError::new(CaptureErrorKind::Internal, "pw slot lock poisoned")
                    })?;
                    if let Some(f) = guard.latest.as_ref() {
                        fallback.push(Arc::clone(f));
                    }
                }
                if fallback.len() == self.slots.len() {
                    return Ok(fallback);
                }
                return Err(CaptureError::new(
                    CaptureErrorKind::PipeWireUnavailable,
                    format!(
                        "Timeout waiting for PipeWire frames ({}/{})",
                        fallback.len(),
                        self.slots.len()
                    ),
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Drop for PersistentPipeWirePump {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_pump(
    nodes: Vec<(u32, u32, u32)>,
    slots: Vec<Arc<Mutex<SlotState>>>,
    stop: Arc<AtomicBool>,
    ready_tx: mpsc::Sender<Result<(), String>>,
) -> Result<(), ()> {
    let mainloop = match pipewire::main_loop::MainLoop::new(None) {
        Ok(m) => m,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("Failed mainloop: {}", e)));
            return Err(());
        }
    };
    let context = match pipewire::context::Context::new(&mainloop) {
        Ok(c) => c,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("Failed context: {}", e)));
            return Err(());
        }
    };
    let core = match context.connect(None) {
        Ok(c) => c,
        Err(e) => {
            let _ = ready_tx.send(Err(format!("Failed connect PipeWire: {}", e)));
            return Err(());
        }
    };

    // Keep streams + listeners alive for the whole pump lifetime.
    let mut _streams = Vec::new();
    let mut _listeners = Vec::new();

    for (idx, &(node_id, hint_w, hint_h)) in nodes.iter().enumerate() {
        let slot = slots[idx].clone();
        let negotiated: Arc<Mutex<Option<(u32, u32, CapturePixelFormat)>>> =
            Arc::new(Mutex::new(None));
        let negotiated_cb = negotiated.clone();

        let stream = match pipewire::stream::Stream::new(
            &core,
            &format!("vectrace-pw-{}", node_id),
            pipewire::properties::properties! {
                *pipewire::keys::MEDIA_TYPE => "Video",
                *pipewire::keys::MEDIA_CATEGORY => "Capture",
                *pipewire::keys::MEDIA_ROLE => "Screen",
            },
        ) {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Failed stream: {}", e)));
                return Err(());
            }
        };

        let listener = match stream
            .add_local_listener::<()>()
            .param_changed(move |_stream, _ud, id, param| {
                let Some(param) = param else { return };
                if id != pipewire::spa::param::ParamType::Format.as_raw() {
                    return;
                }
                let Ok((media_type, media_subtype)) =
                    pipewire::spa::param::format_utils::parse_format(param)
                else {
                    return;
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
                if let Ok(mut lock) = negotiated_cb.lock() {
                    *lock = Some((size.width, size.height, pixel_format));
                }
            })
            .process(move |stream, _| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let datas = buffer.datas_mut();
                if datas.is_empty() {
                    return;
                }
                let data = &mut datas[0];
                let chunk = data.chunk();
                let size = chunk.size() as usize;
                let stride = chunk.stride().max(0) as usize;
                if size == 0 {
                    return;
                }
                let Some(map) = data.data() else {
                    return;
                };
                let copy_len = size.min(map.len());
                if copy_len == 0 {
                    return;
                }
                let mut bytes = vec![0u8; copy_len];
                bytes.copy_from_slice(&map[..copy_len]);

                let (width, height, format) = negotiated
                    .lock()
                    .ok()
                    .and_then(|g| *g)
                    .unwrap_or((hint_w, hint_h, CapturePixelFormat::Bgrx8888));

                if let Ok(mut lock) = slot.lock() {
                    lock.latest = Some(Arc::new(RawFrame {
                        width,
                        height,
                        stride: if stride > 0 {
                            stride
                        } else {
                            (width as usize).saturating_mul(4)
                        },
                        format,
                        bytes: Arc::<[u8]>::from(bytes),
                        captured_at: Instant::now(),
                    }));
                }
            })
            .register()
        {
            Ok(l) => l,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Failed listener: {}", e)));
                return Err(());
            }
        };

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
                    width: hint_w.max(1),
                    height: hint_h.max(1),
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

        let values: Vec<u8> = match pipewire::spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &pipewire::spa::pod::Value::Object(obj),
        ) {
            Ok(v) => v.0.into_inner(),
            Err(e) => {
                let _ = ready_tx.send(Err(format!("Serialize EnumFormat: {}", e)));
                return Err(());
            }
        };

        let mut params = [match Pod::from_bytes(&values) {
            Some(p) => p,
            None => {
                let _ = ready_tx.send(Err("Failed to build EnumFormat pod".into()));
                return Err(());
            }
        }];

        if let Err(e) = stream.connect(
            pipewire::spa::utils::Direction::Input,
            Some(node_id),
            pipewire::stream::StreamFlags::AUTOCONNECT
                | pipewire::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        ) {
            let _ = ready_tx.send(Err(format!(
                "connect node {}: {}",
                node_id, e
            )));
            return Err(());
        }

        _listeners.push(listener);
        _streams.push(stream);
    }

    let _ = ready_tx.send(Ok(()));

    while !stop.load(Ordering::SeqCst) {
        let loop_ref = mainloop.loop_();
        let _ = loop_ref.iterate(Duration::from_millis(5));
    }

    Ok(())
}
