//! Silent (0-flash) capture via `org.gnome.Mutter.ScreenCast` + PipeWire.
//! Unlike the XDG Screenshot portal, this path does not trigger GNOME's
//! shutter animation or camera sound.

use crate::platform::wayland::capture::pipewire::PipeWireStreamReader;
use crate::snapshot::composition::CompositionEngine;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::CapturePixelFormat;
use futures_lite::stream::StreamExt;
use std::collections::HashMap;
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
pub fn capture_desktop() -> Result<tiny_skia::Pixmap, CaptureError> {
    let monitors = list_logical_monitors()?;
    let rt = crate::platform::wayland::capture::portal::portal_runtime();

    rt.block_on(async {
        let conn = Connection::session().await.map_err(|e| {
            map_err(
                CaptureErrorKind::PortalUnavailable,
                format!("DBus session failed: {}", e),
            )
        })?;

        // Probe availability early.
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
            // cursor-mode: 0 = hidden
            props.insert("cursor-mode", Value::U32(0));
            // is-recording: false → avoid recording indicator when supported (API v4)
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

        // Subscribe to PipeWireStreamAdded before Start.
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
                // Non-blocking poll via timeout.
                match tokio::time::timeout(Duration::from_millis(50), signals.next()).await {
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

        // Capture frames on the default PipeWire session (blocking).
        let mut captured: Vec<(LogicalMonitor, tiny_skia::Pixmap)> = Vec::new();
        for (mon, stream_path) in &stream_paths {
            let node_id = *node_by_stream.get(stream_path).unwrap();
            let hint_w = mon.width.max(1);
            let hint_h = mon.height.max(1);
            let mon_clone = mon.clone();
            let pixmap = tokio::task::spawn_blocking(move || {
                let mut reader = PipeWireStreamReader::from_node(node_id);
                let deadline = Instant::now() + Duration::from_secs(2);
                let frame = reader.acquire_frame(
                    deadline,
                    hint_w,
                    hint_h,
                    CapturePixelFormat::Bgrx8888,
                )?;
                let rgba = CompositionEngine::normalize_frame(&frame)?;
                let mut pm = tiny_skia::Pixmap::new(frame.width, frame.height).ok_or_else(|| {
                    CaptureError::new(
                        CaptureErrorKind::Internal,
                        format!("Pixmap alloc {}x{}", frame.width, frame.height),
                    )
                })?;
                pm.data_mut().copy_from_slice(&rgba);
                Ok::<_, CaptureError>(pm)
            })
            .await
            .map_err(|e| {
                map_err(
                    CaptureErrorKind::Internal,
                    format!("PipeWire capture task failed: {}", e),
                )
            })??;

            captured.push((mon_clone, pixmap));
        }

        let _ = session.stop().await;

        stitch_monitors(&captured)
    })
}

fn stitch_monitors(
    captured: &[(LogicalMonitor, tiny_skia::Pixmap)],
) -> Result<tiny_skia::Pixmap, CaptureError> {
    let mut max_x = 0i32;
    let mut max_y = 0i32;
    for (mon, pm) in captured {
        max_x = max_x.max(mon.x + pm.width() as i32);
        max_y = max_y.max(mon.y + pm.height() as i32);
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

    let paint = tiny_skia::PixmapPaint::default();
    for (mon, pm) in captured {
        desktop.draw_pixmap(
            mon.x,
            mon.y,
            pm.as_ref(),
            &paint,
            tiny_skia::Transform::identity(),
            None,
        );
    }

    println!(
        "Captured desktop via Mutter ScreenCast ({}x{}, {} monitors)!",
        desktop.width(),
        desktop.height(),
        captured.len()
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
