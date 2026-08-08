//! Silent (0-flash) capture via `org.gnome.Mutter.ScreenCast` + PipeWire.
//! Unlike the XDG Screenshot portal, this path does not trigger GNOME's
//! shutter animation or camera sound.

use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use futures_lite::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{proxy, Connection};

#[derive(Debug, Clone)]
pub struct LogicalMonitor {
    pub connector: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

#[proxy(
    default_service = "org.gnome.Mutter.DisplayConfig",
    interface = "org.gnome.Mutter.DisplayConfig",
    default_path = "/org/gnome/Mutter/DisplayConfig"
)]
trait MutterDisplayConfig {
    #[zbus(name = "GetCurrentState")]
    fn get_current_state(
        &self,
    ) -> zbus::Result<(
        u32,
        Vec<(
            (String, String, String, String),
            Vec<(
                String,
                i32,
                i32,
                f64,
                f64,
                Vec<f64>,
                HashMap<String, OwnedValue>,
            )>,
            HashMap<String, OwnedValue>,
        )>,
        Vec<(
            i32,
            i32,
            f64,
            u32,
            bool,
            Vec<(String, String, String, String)>,
            HashMap<String, OwnedValue>,
        )>,
        HashMap<String, OwnedValue>,
    )>;
}

#[proxy(
    default_service = "org.gnome.Mutter.ScreenCast",
    interface = "org.gnome.Mutter.ScreenCast",
    default_path = "/org/gnome/Mutter/ScreenCast"
)]
trait MutterScreenCast {
    #[zbus(name = "CreateSession")]
    fn create_session(
        &self,
        properties: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    default_service = "org.gnome.Mutter.ScreenCast",
    interface = "org.gnome.Mutter.ScreenCast.Session"
)]
trait MutterScreenCastSession {
    #[zbus(name = "RecordMonitor")]
    fn record_monitor(
        &self,
        connector: &str,
        properties: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    #[zbus(name = "Start")]
    fn start(&self) -> zbus::Result<()>;

    #[zbus(name = "Stop")]
    fn stop(&self) -> zbus::Result<()>;
}

#[proxy(
    default_service = "org.gnome.Mutter.ScreenCast",
    interface = "org.gnome.Mutter.ScreenCast.Stream"
)]
trait MutterScreenCastStream {
    #[zbus(signal, name = "PipeWireStreamAdded")]
    fn pipewire_stream_added(&self, node_id: u32) -> zbus::Result<()>;

    #[zbus(property, name = "Parameters")]
    fn parameters(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
}

fn map_err(kind: CaptureErrorKind, msg: impl Into<String>) -> CaptureError {
    CaptureError::new(kind, msg)
}

/// Discover logical monitors (connector + layout) from Mutter DisplayConfig.
pub fn list_logical_monitors() -> Result<Vec<LogicalMonitor>, CaptureError> {
    let rt = crate::platform::wayland::capture::portal::portal_runtime();
    rt.block_on(async {
        let conn = Connection::session().await.map_err(|e| {
            map_err(
                CaptureErrorKind::PortalUnavailable,
                format!("DBus session failed: {}", e),
            )
        })?;
        let proxy = MutterDisplayConfigProxy::new(&conn).await.map_err(|e| {
            map_err(
                CaptureErrorKind::PortalUnavailable,
                format!("DisplayConfig proxy failed: {}", e),
            )
        })?;

        let (_serial, physical, logical, _props) = proxy.get_current_state().await.map_err(|e| {
            map_err(
                CaptureErrorKind::PortalUnavailable,
                format!("GetCurrentState failed: {}", e),
            )
        })?;

        // Map connector -> preferred mode size from physical monitor list.
        let mut sizes: HashMap<String, (u32, u32)> = HashMap::new();
        for ((connector, _, _, _), modes, _) in &physical {
            let mut w = 1920u32;
            let mut h = 1080u32;
            for mode in modes {
                let (_id, mw, mh, _refresh, _scale_pref, _scales, mode_props) = mode;
                let is_current = mode_props
                    .get("is-current")
                    .and_then(|v| bool::try_from(v.try_clone().ok()?).ok())
                    .unwrap_or(false);
                if is_current {
                    w = (*mw).max(0) as u32;
                    h = (*mh).max(0) as u32;
                    break;
                }
            }
            sizes.insert(connector.clone(), (w, h));
        }

        let mut out = Vec::new();
        for (x, y, _scale, _transform, primary, monitors, _) in &logical {
            let Some((connector, _, _, _)) = monitors.first() else {
                continue;
            };
            let (w, h) = sizes.get(connector).copied().unwrap_or((1920, 1080));
            out.push(LogicalMonitor {
                connector: connector.clone(),
                x: *x,
                y: *y,
                width: w,
                height: h,
                primary: *primary,
            });
        }

        if out.is_empty() {
            return Err(map_err(
                CaptureErrorKind::PortalUnavailable,
                "Mutter DisplayConfig returned no logical monitors",
            ));
        }
        Ok(out)
    })
}

/// Capture the full logical desktop via Mutter ScreenCast (no flash / no sound).
///
/// Uses a **warm session**: CreateSession/Start runs once, then each Save only
/// pulls PipeWire frames from cached node IDs (~ms instead of ~seconds).
pub fn capture_desktop() -> Result<tiny_skia::Pixmap, CaptureError> {
    let t0 = Instant::now();

    // Clone pump handle under a short lock, then do the heavy work unlocked.
    let (pump, streams) = {
        let guard = warm_slot().lock().map_err(|_| {
            CaptureError::new(CaptureErrorKind::Internal, "warm mutter lock poisoned")
        })?;
        if let Some(ref warm) = *guard {
            (
                Arc::clone(&warm.pump),
                warm.streams.clone(),
            )
        } else {
            drop(guard);
            invalidate_warm();
            let warm = open_warm_session()?;
            let pump = Arc::clone(&warm.pump);
            let streams = warm.streams.clone();
            let mut guard = warm_slot().lock().map_err(|_| {
                CaptureError::new(CaptureErrorKind::Internal, "warm mutter lock poisoned")
            })?;
            *guard = Some(warm);
            (pump, streams)
        }
    };

    match grab_from_pump(&pump, &streams) {
        Ok(pm) => {
            println!(
                "Warm capture {}x{} in {:.1}ms",
                pm.width(),
                pm.height(),
                t0.elapsed().as_secs_f64() * 1000.0
            );
            Ok(pm)
        }
        Err(e) => {
            println!("Warm grab failed ({:?}); recreating session...", e);
            invalidate_warm();
            let warm = open_warm_session()?;
            let pump = Arc::clone(&warm.pump);
            let streams = warm.streams.clone();
            if let Ok(mut guard) = warm_slot().lock() {
                *guard = Some(warm);
            }
            let pm = grab_from_pump(&pump, &streams)?;
            println!(
                "Cold→warm capture {}x{} in {:.1}ms",
                pm.width(),
                pm.height(),
                t0.elapsed().as_secs_f64() * 1000.0
            );
            Ok(pm)
        }
    }
}

/// Pre-create the Mutter session so the first Save is also fast.
pub fn ensure_warm() -> Result<(), CaptureError> {
    let mut guard = warm_slot().lock().map_err(|_| {
        CaptureError::new(CaptureErrorKind::Internal, "warm mutter lock poisoned")
    })?;
    if guard.is_some() {
        return Ok(());
    }
    let warm = open_warm_session()?;
    *guard = Some(warm);
    Ok(())
}

pub fn invalidate_warm() {
    if let Ok(mut guard) = warm_slot().lock() {
        if let Some(warm) = guard.take() {
            warm.stop();
        }
    }
}

#[derive(Clone)]
struct WarmStream {
    monitor: LogicalMonitor,
    node_id: u32,
}

struct WarmSession {
    conn: Connection,
    session_path: OwnedObjectPath,
    streams: Vec<WarmStream>,
    /// Keeps PipeWire streams connected; grab is a fast stitch of latest buffers.
    pump: Arc<crate::platform::wayland::capture::pw_pump::PersistentPipeWirePump>,
}

impl WarmSession {
    fn stop(&self) {
        let rt = crate::platform::wayland::capture::portal::portal_runtime();
        let conn = self.conn.clone();
        let path = self.session_path.clone();
        let _ = rt.block_on(async move {
            let Ok(builder) = MutterScreenCastSessionProxy::builder(&conn).path(&path) else {
                return;
            };
            let Ok(session) = builder.build().await else {
                return;
            };
            let _ = session.stop().await;
        });
    }
}

fn warm_slot() -> &'static Mutex<Option<WarmSession>> {
    static SLOT: OnceLock<Mutex<Option<WarmSession>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn open_warm_session() -> Result<WarmSession, CaptureError> {
    let monitors = list_logical_monitors()?;
    let rt = crate::platform::wayland::capture::portal::portal_runtime();

    rt.block_on(async {
        let conn = Connection::session().await.map_err(|e| {
            map_err(
                CaptureErrorKind::PortalUnavailable,
                format!("DBus session failed: {}", e),
            )
        })?;

        let screencast = MutterScreenCastProxy::new(&conn).await.map_err(|e| {
            map_err(
                CaptureErrorKind::PortalUnavailable,
                format!("Mutter ScreenCast unavailable: {}", e),
            )
        })?;

        let session_path = screencast
            .create_session(HashMap::new())
            .await
            .map_err(|e| {
                map_err(
                    CaptureErrorKind::PortalUnavailable,
                    format!("CreateSession failed: {}", e),
                )
            })?;

        let session = MutterScreenCastSessionProxy::builder(&conn)
            .path(&session_path)
            .map_err(|e| {
                map_err(
                    CaptureErrorKind::PortalUnavailable,
                    format!("Invalid session path: {}", e),
                )
            })?
            .build()
            .await
            .map_err(|e| {
                map_err(
                    CaptureErrorKind::PortalUnavailable,
                    format!("Session proxy failed: {}", e),
                )
            })?;

        let mut stream_paths: Vec<(LogicalMonitor, OwnedObjectPath)> = Vec::new();
        for mon in &monitors {
            let mut props: HashMap<&str, Value<'_>> = HashMap::new();
            props.insert("cursor-mode", Value::U32(0));
            props.insert("is-recording", Value::Bool(false));

            let stream_path = session
                .record_monitor(&mon.connector, props)
                .await
                .map_err(|e| {
                    map_err(
                        CaptureErrorKind::PortalUnavailable,
                        format!("RecordMonitor({}) failed: {}", mon.connector, e),
                    )
                })?;
            stream_paths.push((mon.clone(), stream_path));
        }

        let mut node_by_stream: HashMap<OwnedObjectPath, u32> = HashMap::new();
        let mut signal_streams = Vec::new();
        for (_mon, stream_path) in &stream_paths {
            let stream_proxy = MutterScreenCastStreamProxy::builder(&conn)
                .path(stream_path)
                .map_err(|e| {
                    map_err(
                        CaptureErrorKind::PortalUnavailable,
                        format!("Invalid stream path: {}", e),
                    )
                })?
                .build()
                .await
                .map_err(|e| {
                    map_err(
                        CaptureErrorKind::PortalUnavailable,
                        format!("Stream proxy failed: {}", e),
                    )
                })?;

            let stream_added = stream_proxy
                .receive_pipewire_stream_added()
                .await
                .map_err(|e| {
                    map_err(
                        CaptureErrorKind::PipeWireUnavailable,
                        format!("Failed to subscribe PipeWireStreamAdded: {}", e),
                    )
                })?;
            signal_streams.push((stream_path.clone(), stream_added));
        }

        session.start().await.map_err(|e| {
            map_err(
                CaptureErrorKind::PortalUnavailable,
                format!("ScreenCast Start failed: {}", e),
            )
        })?;

        let deadline = Instant::now() + Duration::from_secs(3);
        while node_by_stream.len() < stream_paths.len() && Instant::now() < deadline {
            for (path, signals) in signal_streams.iter_mut() {
                if node_by_stream.contains_key(path) {
                    continue;
                }
                match tokio::time::timeout(Duration::from_millis(20), signals.next()).await {
                    Ok(Some(signal)) => {
                        let node_id = signal.args().map(|a| a.node_id).unwrap_or(0);
                        if node_id != 0 {
                            node_by_stream.insert(path.clone(), node_id);
                        }
                    }
                    _ => {}
                }
            }
        }

        if node_by_stream.len() < stream_paths.len() {
            let _ = session.stop().await;
            return Err(map_err(
                CaptureErrorKind::PipeWireUnavailable,
                format!(
                    "Timed out waiting for PipeWire nodes ({}/{})",
                    node_by_stream.len(),
                    stream_paths.len()
                ),
            ));
        }

        let streams = stream_paths
            .into_iter()
            .map(|(monitor, path)| WarmStream {
                monitor,
                node_id: *node_by_stream.get(&path).unwrap(),
            })
            .collect::<Vec<_>>();

        let pw_nodes: Vec<(u32, u32, u32)> = streams
            .iter()
            .map(|s| (s.node_id, s.monitor.width.max(1), s.monitor.height.max(1)))
            .collect();

        let pump = Arc::new(
            crate::platform::wayland::capture::pw_pump::PersistentPipeWirePump::start(pw_nodes)?,
        );

        println!(
            "Warm Mutter ScreenCast ready ({} monitor(s)) — PipeWire pump attached",
            streams.len()
        );

        Ok(WarmSession {
            conn,
            session_path,
            streams,
            pump,
        })
    })
}

fn grab_from_pump(
    pump: &crate::platform::wayland::capture::pw_pump::PersistentPipeWirePump,
    streams: &[WarmStream],
) -> Result<tiny_skia::Pixmap, CaptureError> {
    let t_wait = Instant::now();
    // Wait briefly for a frame newer than unmap (≤1 display refresh).
    let not_before = Instant::now();
    let raw_frames = pump.snapshot(Some(not_before), Duration::from_millis(50))?;
    let wait_ms = t_wait.elapsed().as_secs_f64() * 1000.0;

    if raw_frames.len() != streams.len() {
        return Err(CaptureError::new(
            CaptureErrorKind::PipeWireUnavailable,
            format!(
                "Frame/stream mismatch {} vs {}",
                raw_frames.len(),
                streams.len()
            ),
        ));
    }

    let t_stitch = Instant::now();
    let desktop = stitch_raw_frames(streams, &raw_frames)?;
    let stitch_ms = t_stitch.elapsed().as_secs_f64() * 1000.0;
    println!(
        "Warm grab breakdown: wait={:.1}ms stitch={:.1}ms",
        wait_ms, stitch_ms
    );
    Ok(desktop)
}

/// Single-pass BGRX/BGRA/RGBX/RGBA → RGBA desktop stitch (one alloc, no per-monitor Pixmap).
/// Uses `chunks_exact` so release builds auto-vectorize the BGR↔RGB swap.
fn stitch_raw_frames(
    streams: &[WarmStream],
    frames: &[std::sync::Arc<crate::platform::wayland::capture::pw_pump::RawFrame>],
) -> Result<tiny_skia::Pixmap, CaptureError> {
    use crate::snapshot::frame::CapturePixelFormat;

    let mut max_x = 0i32;
    let mut max_y = 0i32;
    for (stream, frame) in streams.iter().zip(frames.iter()) {
        max_x = max_x.max(stream.monitor.x + frame.width as i32);
        max_y = max_y.max(stream.monitor.y + frame.height as i32);
    }
    if max_x <= 0 || max_y <= 0 {
        return Err(map_err(
            CaptureErrorKind::Internal,
            "Invalid desktop bounds from Mutter capture",
        ));
    }

    let mut desktop = tiny_skia::Pixmap::new(max_x as u32, max_y as u32).ok_or_else(|| {
        map_err(
            CaptureErrorKind::Internal,
            format!("Failed to allocate desktop pixmap {}x{}", max_x, max_y),
        )
    })?;
    let dst_w = max_x as usize;
    let dst_h = max_y as usize;

    // Convert each monitor in parallel (dual-HDMI: 2×1920×1080), then blit.
    // Parallelism matters more in debug; release still benefits on large desktops.
    let converted: Vec<(usize, usize, u32, u32, Vec<u8>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = streams
            .iter()
            .zip(frames.iter())
            .filter_map(|(stream, frame)| {
                let ox = stream.monitor.x;
                let oy = stream.monitor.y;
                if ox < 0 || oy < 0 {
                    return None;
                }
                let ox = ox as usize;
                let oy = oy as usize;
                let frame = Arc::clone(frame);
                Some(scope.spawn(move || {
                    let w = frame.width as usize;
                    let h = frame.height as usize;
                    let row_bytes = w * 4;
                    let mut rgba = vec![0u8; row_bytes * h];
                    let src = frame.bytes.as_ref();
                    let stride = frame.stride;
                    match frame.format {
                        CapturePixelFormat::Rgba8888 if stride == row_bytes => {
                            let n = row_bytes * h;
                            rgba.copy_from_slice(&src[..n.min(src.len())]);
                        }
                        CapturePixelFormat::Rgbx8888 if stride == row_bytes => {
                            let n = row_bytes * h;
                            rgba.copy_from_slice(&src[..n.min(src.len())]);
                            for px in rgba.chunks_exact_mut(4) {
                                px[3] = 255;
                            }
                        }
                        CapturePixelFormat::Rgba8888 | CapturePixelFormat::Rgbx8888 => {
                            let opaque = matches!(frame.format, CapturePixelFormat::Rgbx8888);
                            for y in 0..h {
                                let s = y * stride;
                                let d = y * row_bytes;
                                if s + row_bytes > src.len() {
                                    break;
                                }
                                rgba[d..d + row_bytes].copy_from_slice(&src[s..s + row_bytes]);
                                if opaque {
                                    for px in rgba[d..d + row_bytes].chunks_exact_mut(4) {
                                        px[3] = 255;
                                    }
                                }
                            }
                        }
                        CapturePixelFormat::Bgra8888 | CapturePixelFormat::Bgrx8888 => {
                            let opaque = matches!(frame.format, CapturePixelFormat::Bgrx8888);
                            for y in 0..h {
                                let s = y * stride;
                                let d = y * row_bytes;
                                if s + row_bytes > src.len() {
                                    break;
                                }
                                for (sp, dp) in src[s..s + row_bytes]
                                    .chunks_exact(4)
                                    .zip(rgba[d..d + row_bytes].chunks_exact_mut(4))
                                {
                                    dp[0] = sp[2];
                                    dp[1] = sp[1];
                                    dp[2] = sp[0];
                                    dp[3] = if opaque { 255 } else { sp[3] };
                                }
                            }
                        }
                    }
                    (ox, oy, frame.width, frame.height, rgba)
                }))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("monitor convert thread"))
            .collect()
    });

    let dst = desktop.data_mut();
    for (ox, oy, width, height, rgba) in converted {
        let w = width as usize;
        let h = height as usize;
        let row_bytes = w * 4;
        for row in 0..h {
            if oy + row >= dst_h || ox + w > dst_w {
                break;
            }
            let src_off = row * row_bytes;
            let dst_off = ((oy + row) * dst_w + ox) * 4;
            dst[dst_off..dst_off + row_bytes]
                .copy_from_slice(&rgba[src_off..src_off + row_bytes]);
        }
    }

    println!(
        "Captured desktop via Mutter ScreenCast ({}x{}, {} monitors)!",
        desktop.width(),
        desktop.height(),
        streams.len()
    );
    Ok(desktop)
}

pub fn is_available() -> bool {
    let rt = crate::platform::wayland::capture::portal::portal_runtime();
    rt.block_on(async {
        let Ok(conn) = Connection::session().await else {
            return false;
        };
        MutterScreenCastProxy::new(&conn).await.is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires live Mutter/PipeWire session"]
    fn mutter_capture_smoke() {
        let monitors = list_logical_monitors().expect("list monitors");
        assert!(!monitors.is_empty());
        println!("monitors: {:?}", monitors);
        let pm = capture_desktop().expect("capture");
        assert!(pm.width() >= 1920);
        assert!(pm.height() >= 1080);
        println!("captured {}x{}", pm.width(), pm.height());
    }
}
