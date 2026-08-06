use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::request::{CaptureRequest, CaptureTarget, CursorPolicy};
use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::screenshot::Screenshot;
use ashpd::desktop::PersistMode;
use std::os::fd::OwnedFd;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static PORTAL_RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_portal_runtime() -> &'static Runtime {
    PORTAL_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create persistent Tokio runtime for PortalClient")
    })
}

#[derive(Debug, Clone)]
pub struct PortalStreamInfo {
    pub node_id: u32,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub position_x: Option<i32>,
    pub position_y: Option<i32>,
}

#[derive(Debug)]
pub struct PortalSessionResult {
    pub session_handle: String,
    pub streams: Vec<PortalStreamInfo>,
    pub restore_token: Option<String>,
    pub pipewire_fd: OwnedFd,
}

pub struct PortalClient;

impl PortalClient {
    pub fn new() -> Self {
        Self
    }

    pub fn is_available(&self) -> bool {
        true
    }

    pub fn start_screencast_session(
        &mut self,
        request: &CaptureRequest,
        restore_token: Option<&str>,
    ) -> Result<PortalSessionResult, CaptureError> {
        let rt = get_portal_runtime();

        rt.block_on(async {
            let proxy = Screencast::new().await.map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::PortalUnavailable,
                    format!("Failed to connect to ScreenCast portal: {}", e),
                )
            })?;

            let session = proxy.create_session().await.map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::PortalBackendMissing,
                    format!("Failed to create portal session: {}", e),
                )
            })?;

            let cursor_mode = match request.cursor {
                CursorPolicy::Hidden => CursorMode::Hidden,
                CursorPolicy::Embedded => CursorMode::Embedded,
                CursorPolicy::Metadata => CursorMode::Metadata,
            };

            let multiple = request.target == CaptureTarget::AllMonitors;
            let persist = PersistMode::Application;

            proxy
                .select_sources(
                    &session,
                    cursor_mode,
                    SourceType::Monitor.into(),
                    multiple,
                    restore_token,
                    persist,
                )
                .await
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PortalBackendMissing,
                        format!("SelectSources call failed: {}", e),
                    )
                })?
                .response()
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PermissionDenied,
                        format!("SelectSources denied: {}", e),
                    )
                })?;

            let start_response = proxy
                .start(&session, None)
                .await
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PortalBackendMissing,
                        format!("Start request failed: {}", e),
                    )
                })?
                .response()
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PermissionDenied,
                        format!("Start request denied: {}", e),
                    )
                })?;

            let pipewire_fd = proxy.open_pipe_wire_remote(&session).await.map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::PipeWireUnavailable,
                    format!("Failed to open PipeWire remote FD: {}", e),
                )
            })?;

            let mut streams = Vec::new();
            for stream in start_response.streams() {
                streams.push(PortalStreamInfo {
                    node_id: stream.pipe_wire_node_id(),
                    width: stream.size().map(|(w, _)| w as u32),
                    height: stream.size().map(|(_, h)| h as u32),
                    position_x: stream.position().map(|(x, _)| x),
                    position_y: stream.position().map(|(_, y)| y),
                });
            }

            if streams.is_empty() {
                return Err(CaptureError::new(
                    CaptureErrorKind::InvalidPortalResponse,
                    "ScreenCast portal returned zero streams",
                ));
            }

            Ok(PortalSessionResult {
                session_handle: format!("{:?}", session),
                streams,
                restore_token: start_response.restore_token().map(|t| t.to_string()),
                pipewire_fd: pipewire_fd.into(),
            })
        })
    }

    pub fn take_screenshot() -> Result<tiny_skia::Pixmap, CaptureError> {
        let rt = get_portal_runtime();

        rt.block_on(async {
            let response = Screenshot::request()
                .interactive(false)
                .send()
                .await
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PortalBackendMissing,
                        format!("Screenshot request failed: {}", e),
                    )
                })?
                .response()
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PermissionDenied,
                        format!("Screenshot permission denied: {}", e),
                    )
                })?;

            let uri = response.uri();
            let raw_path = uri.as_str().trim_start_matches("file://");
            let path_str = urlencoding::decode(raw_path).unwrap_or(std::borrow::Cow::Borrowed(raw_path));

            let bytes = std::fs::read(path_str.as_ref()).map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::Io,
                    format!("Failed to read screenshot file from {}: {}", path_str, e),
                )
            })?;

            let _ = std::fs::remove_file(path_str.as_ref());

            tiny_skia::Pixmap::decode_png(&bytes).map_err(|e| {
                CaptureError::new(
                    CaptureErrorKind::InvalidPortalResponse,
                    format!("Failed to decode portal screenshot PNG: {}", e),
                )
            })
        })
    }
}

impl Default for PortalClient {
    fn default() -> Self {
        Self::new()
    }
}
