use crate::snapshot::composition::CompositionEngine;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::request::{CaptureRequest, CaptureTarget, CursorPolicy};
use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::screenshot::Screenshot;
use ashpd::desktop::PersistMode;
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static PORTAL_RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub(crate) fn portal_runtime() -> &'static Runtime {
    PORTAL_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create persistent Tokio runtime for PortalClient")
    })
}

fn get_portal_runtime() -> &'static Runtime {
    portal_runtime()
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

            let multiple = matches!(
                request.target,
                CaptureTarget::AllMonitors
            );
            // ExplicitlyRevoked: restore token survives app restarts until the
            // user revokes it in system settings. Application(=transient) only
            // lasts while the process is alive — that caused a picker on every launch.
            let persist = PersistMode::ExplicitlyRevoked;

            let token_note = restore_token
                .map(|t| format!("yes ({} chars)", t.len()))
                .unwrap_or_else(|| "no".into());
            println!(
                "XDG ScreenCast SelectSources: restore_token={}, persist=ExplicitlyRevoked, multiple={}",
                token_note, multiple
            );

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
        Self::take_screenshot_with_path().map(|(pixmap, _)| pixmap)
    }

    /// Same capture chain as [`Self::take_screenshot`], but reports which path won.
    pub fn take_screenshot_with_path() -> Result<
        (
            tiny_skia::Pixmap,
            crate::platform::wayland::capture::probe::CapturePathUsed,
        ),
        CaptureError,
    > {
        use crate::platform::wayland::capture::probe::CapturePathUsed;

        // 1) Mutter ScreenCast: true 0-flash / no camera sound on GNOME.
        match crate::platform::wayland::capture::mutter::capture_desktop() {
            Ok(pixmap) => return Ok((pixmap, CapturePathUsed::MutterScreenCast)),
            Err(e) => {
                println!(
                    "Mutter ScreenCast unavailable ({:?}); trying XDG ScreenCast...",
                    e
                );
            }
        }

        // 2) XDG ScreenCast + restore token (still 0-flash; picker only without token).
        match Self::take_screencast_frame() {
            Ok(pixmap) => return Ok((pixmap, CapturePathUsed::XdgScreenCast)),
            Err(e) => {
                println!("XDG ScreenCast failed: {:?}", e);
            }
        }

        // 3) Screenshot portal flashes on GNOME — only if explicitly allowed.
        if std::env::var_os("VECTRACE_ALLOW_FLASH").is_some() {
            println!("VECTRACE_ALLOW_FLASH set; using Screenshot portal (will flash)...");
            let pixmap = Self::take_portal_screenshot()?;
            return Ok((pixmap, CapturePathUsed::ScreenshotFlash));
        }

        Err(CaptureError::new(
            CaptureErrorKind::PortalUnavailable,
            "Capture failed (flash path disabled)",
        ))
    }

    fn take_screencast_frame() -> Result<tiny_skia::Pixmap, CaptureError> {
        let storage = crate::platform::wayland::capture::RestoreTokenStorage::new();
        let restore_token = storage.load_token();
        if let Some(ref t) = restore_token {
            println!(
                "Loaded portal restore token from {} ({} chars)",
                crate::platform::wayland::capture::RestoreTokenStorage::default_path().display(),
                t.len()
            );
        } else {
            println!(
                "No portal restore token at {} — first grant may show a Share dialog",
                crate::platform::wayland::capture::RestoreTokenStorage::default_path().display()
            );
        }

        let req = CaptureRequest {
            // Match a typical “entire screen” grant so restore tokens apply.
            target: CaptureTarget::AllMonitors,
            cursor: CursorPolicy::Hidden,
            ..Default::default()
        };

        let mut client = PortalClient::new();
        let res = match client.start_screencast_session(&req, restore_token.as_deref()) {
            Ok(r) => r,
            Err(e) => {
                // Only drop the token when the portal explicitly rejects restore.
                // Transient PipeWire/timeouts must NOT clear the token and force a picker.
                let invalid_restore = matches!(
                    e.kind,
                    CaptureErrorKind::PermissionDenied | CaptureErrorKind::UserCancelled
                ) && restore_token.is_some();

                if invalid_restore {
                    println!(
                        "Restore token rejected ({:?}); clearing and retrying interactively...",
                        e.kind
                    );
                    storage.clear_token();
                    client.start_screencast_session(&req, None)?
                } else {
                    return Err(e);
                }
            }
        };

        if let Some(ref new_token) = res.restore_token {
            if storage.save_token(new_token) {
                println!("Saved portal restore token ({} chars)", new_token.len());
            }
        }

        let stream = res.streams.first().ok_or_else(|| {
            CaptureError::new(
                CaptureErrorKind::InvalidPortalResponse,
                "ScreenCast portal returned zero streams",
            )
        })?;

        let node_id = stream.node_id;
        let hint_w = stream.width.unwrap_or(1920).max(1);
        let hint_h = stream.height.unwrap_or(1080).max(1);
        let mut reader =
            crate::platform::wayland::capture::pipewire::PipeWireStreamReader::new(
                res.pipewire_fd,
                node_id,
            );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let frame = reader.acquire_frame(
            deadline,
            hint_w,
            hint_h,
            crate::snapshot::frame::CapturePixelFormat::Bgrx8888,
        )?;

        let rgba = CompositionEngine::normalize_frame(&frame)?;
        let w = frame.width;
        let h = frame.height;
        let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or_else(|| {
            CaptureError::new(
                CaptureErrorKind::Internal,
                format!("Failed to allocate pixmap {}x{}", w, h),
            )
        })?;
        pixmap.data_mut().copy_from_slice(&rgba);
        println!(
            "Captured desktop via XDG ScreenCast PipeWire ({}x{})!",
            w, h
        );
        Ok(pixmap)
    }

    fn take_portal_screenshot() -> Result<tiny_skia::Pixmap, CaptureError> {
        let rt = get_portal_runtime();
        let path: PathBuf = rt.block_on(async {
            let response = Screenshot::request()
                .interactive(false)
                .modal(false)
                .send()
                .await
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PortalUnavailable,
                        format!("Screenshot portal request failed: {}", e),
                    )
                })?
                .response()
                .map_err(|e| {
                    CaptureError::new(
                        CaptureErrorKind::PermissionDenied,
                        format!("Screenshot portal denied: {}", e),
                    )
                })?;

            let uri = response.uri();
            uri.to_file_path().map_err(|_| {
                CaptureError::new(
                    CaptureErrorKind::InvalidPortalResponse,
                    format!("Screenshot portal returned non-file URI: {}", uri),
                )
            })
        })?;

        let pixmap = tiny_skia::Pixmap::load_png(&path).map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::Io,
                format!(
                    "Failed to load Screenshot portal PNG {}: {}",
                    path.display(),
                    e
                ),
            )
        })?;

        println!(
            "Captured desktop via Screenshot portal ({}x{}) — flash path!",
            pixmap.width(),
            pixmap.height()
        );
        Ok(pixmap)
    }
}

impl Default for PortalClient {
    fn default() -> Self {
        Self::new()
    }
}
