use crate::platform::wayland::capture::portal::{PortalClient, PortalSessionResult};
use crate::platform::wayland::capture::restore_token::RestoreTokenStorage;
use crate::snapshot::backend::{CaptureSessionId, ScreenCaptureBackend};
use crate::snapshot::capabilities::CaptureCapabilities;
use crate::snapshot::error::{CaptureError, CaptureErrorKind};
use crate::snapshot::frame::CapturedFrame;
use crate::snapshot::request::{CaptureRequest, CursorPolicy};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaylandCaptureState {
    Idle,
    CreatingSession,
    SelectingSources,
    Starting,
    OpeningRemote,
    Streaming,
    Stopping,
    Closed,
    Failed(CaptureErrorKind),
}

pub struct WaylandPortalBackend {
    portal_client: PortalClient,
    restore_storage: RestoreTokenStorage,
    state: WaylandCaptureState,
    current_session: Option<PortalSessionResult>,
    next_session_id: u64,
}

impl WaylandPortalBackend {
    pub fn new() -> Self {
        Self {
            portal_client: PortalClient::new(),
            restore_storage: RestoreTokenStorage::new(),
            state: WaylandCaptureState::Idle,
            current_session: None,
            next_session_id: 1,
        }
    }

    pub fn state(&self) -> &WaylandCaptureState {
        &self.state
    }
}

impl Default for WaylandPortalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenCaptureBackend for WaylandPortalBackend {
    fn capabilities(&self) -> CaptureCapabilities {
        CaptureCapabilities {
            backend_name: "WaylandPortalPipeWire".to_string(),
            supports_clean_composite: true,
            supports_visible_composition: true,
            supports_desktop_only: true,
            supports_all_monitors: true,
            supported_cursor_policies: vec![
                CursorPolicy::Hidden,
                CursorPolicy::Embedded,
                CursorPolicy::Metadata,
            ],
        }
    }

    fn start(&mut self, request: CaptureRequest) -> Result<CaptureSessionId, CaptureError> {
        self.state = WaylandCaptureState::CreatingSession;

        let restore_token = self.restore_storage.load_token();
        let session_res = self
            .portal_client
            .start_screencast_session(&request, restore_token.as_deref());

        match session_res {
            Ok(session_info) => {
                if let Some(ref new_token) = session_info.restore_token {
                    self.restore_storage.save_token(new_token);
                }

                let session_id = CaptureSessionId(self.next_session_id);
                self.next_session_id += 1;
                self.current_session = Some(session_info);
                self.state = WaylandCaptureState::Streaming;
                Ok(session_id)
            }
            Err(err) => {
                // Only clear + interactive retry when restore was explicitly rejected.
                let invalid_restore = matches!(
                    err.kind,
                    CaptureErrorKind::PermissionDenied | CaptureErrorKind::UserCancelled
                ) && restore_token.is_some();

                if invalid_restore {
                    println!(
                        "Restore token rejected ({:?}); clearing and retrying interactively...",
                        err.kind
                    );
                    self.restore_storage.clear_token();
                    let retry_res = self
                        .portal_client
                        .start_screencast_session(&request, None);

                    if let Ok(session_info) = retry_res {
                        if let Some(ref new_token) = session_info.restore_token {
                            self.restore_storage.save_token(new_token);
                        }
                        let session_id = CaptureSessionId(self.next_session_id);
                        self.next_session_id += 1;
                        self.current_session = Some(session_info);
                        self.state = WaylandCaptureState::Streaming;
                        return Ok(session_id);
                    }
                }

                self.state = WaylandCaptureState::Failed(err.kind);
                Err(err)
            }
        }
    }

    fn next_frame(&mut self, deadline: Instant) -> Result<CapturedFrame, CaptureError> {
        if self.state != WaylandCaptureState::Streaming {
            return Err(CaptureError::new(
                CaptureErrorKind::SessionClosed,
                "Portal session is not in Streaming state",
            ));
        }

        if let Ok(pixmap) = PortalClient::take_screenshot() {
            let width = pixmap.width();
            let height = pixmap.height();
            let raw_bytes = pixmap.data().to_vec();

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO);

            return Ok(CapturedFrame {
                output: crate::snapshot::request::OutputId(1),
                width,
                height,
                stride: (width * 4) as usize,
                format: crate::snapshot::frame::CapturePixelFormat::Rgba8888,
                memory: crate::snapshot::frame::FrameMemory::Owned(raw_bytes),
                transform: crate::snapshot::frame::OutputTransform::Normal,
                sequence: 1,
                timestamp,
                damage: vec![],
            });
        }

        let session = self.current_session.as_ref().ok_or_else(|| {
            CaptureError::new(
                CaptureErrorKind::SessionClosed,
                "No active PipeWire session available",
            )
        })?;

        let node_id = session.streams.first().map(|s| s.node_id).unwrap_or(1);
        let dup_fd = session.pipewire_fd.try_clone().map_err(|e| {
            CaptureError::new(
                CaptureErrorKind::Io,
                format!("Failed to clone PipeWire FD: {}", e),
            )
        })?;

        let mut stream_reader = crate::platform::wayland::capture::pipewire::PipeWireStreamReader::new(dup_fd, node_id);
        stream_reader.acquire_frame(
            deadline,
            1920,
            1080,
            crate::snapshot::frame::CapturePixelFormat::Bgra8888,
        )
    }

    fn stop(&mut self) -> Result<(), CaptureError> {
        self.state = WaylandCaptureState::Stopping;
        self.current_session = None;
        self.state = WaylandCaptureState::Closed;
        Ok(())
    }
}
