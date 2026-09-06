//! Original adapter from the WP02 X11 backend to the UID-authorized core.
use crate::backend::{self, ActionGuard, Backend, Display, Frame, Input, ResizeResult, Resolution};
use dwdesktop_capture as capture;
use std::time::Instant;

pub struct X11Adapter {
    inner: capture::X11Backend,
    resize_supported: bool,
}
impl X11Adapter {
    pub fn connect(display: Option<String>) -> backend::Result<Self> {
        // Xauthority is read from the service environment by x11rb. Never mutate process
        // environment after Tokio has started, and never accept credentials over IPC.
        let mut inner = capture::X11Backend::connect(capture::BackendOptions {
            display,
            limits: capture::FrameLimits::default(),
        })
        .map_err(code)?;
        let topology = inner.topology().map_err(code)?;
        Ok(Self {
            inner,
            resize_supported: topology.displays.iter().any(|d| d.can_resize),
        })
    }
}

fn code(error: capture::BackendError) -> &'static str {
    use capture::ErrorKind::*;
    match error.kind {
        Unavailable => "worker_unavailable",
        Unsupported => "unsupported_capability",
        InvalidArgument => "invalid_argument",
        StaleTopology => "stale_topology",
        NotFound => "not_found",
        ResourceExhausted => "resource_exhausted",
        CaptureFailed => "capture_failed",
        Cancelled | DeadlineExceeded => "expired",
        InputFailed | OsFailed => "worker_unavailable",
    }
}
fn display(raw: &capture::Display, revision: u64) -> backend::Result<Display> {
    // Never silently crop or downsample an unsupported native display.
    if raw.width == 0 || raw.height == 0 || raw.width > 4096 || raw.height > 4096 {
        return Err("unsupported_capability");
    }
    Ok(Display {
        display_id: raw.id.clone(),
        width: raw.width,
        height: raw.height,
        topology_revision: revision,
        cursor_embedded: false,
        can_resize: raw.can_resize,
        available_resolutions: raw
            .modes
            .iter()
            .filter(|m| m.width > 0 && m.height > 0 && m.width <= 4096 && m.height <= 4096)
            .map(|m| Resolution {
                width: m.width,
                height: m.height,
            })
            .collect(),
    })
}
fn input(raw: Input) -> backend::Result<capture::InputAction> {
    Ok(match raw {
        Input::Pointer {
            x,
            y,
            action,
            button,
        } => {
            let button_value = match button.as_str() {
                "left" => Some(capture::Button::Left),
                "middle" => Some(capture::Button::Middle),
                "right" => Some(capture::Button::Right),
                "none" => None,
                _ => return Err("invalid_argument"),
            };
            let action = match action.as_str() {
                "move" if button_value.is_none() => capture::PointerAction::Move,
                "down" => capture::PointerAction::Down(button_value.ok_or("invalid_argument")?),
                "up" => capture::PointerAction::Up(button_value.ok_or("invalid_argument")?),
                "click" => capture::PointerAction::Click(button_value.ok_or("invalid_argument")?),
                _ => return Err("invalid_argument"),
            };
            capture::InputAction::Pointer { x, y, action }
        }
        Input::Key { usage, down } => capture::InputAction::Key { usage, down },
        Input::Text(text) => capture::InputAction::Text(text),
    })
}
struct GuardAdapter<'a>(&'a mut dyn ActionGuard);
impl capture::ActionGuard for GuardAdapter<'_> {
    fn check(&mut self) -> capture::Result<()> {
        self.0.check().map_err(|_| {
            capture::BackendError::new(capture::ErrorKind::Cancelled, "authorization ended")
        })
    }
}
impl Backend for X11Adapter {
    fn capabilities(&self) -> Vec<&'static str> {
        let capabilities = self.inner.capabilities();
        let mut result = Vec::new();
        if capabilities.capture {
            result.push("desktop.view");
        }
        if capabilities.input {
            result.push("desktop.control");
        }
        if self.resize_supported && capabilities.input {
            result.push("desktop.resize");
        }
        result
    }
    fn displays(&mut self) -> backend::Result<Vec<Display>> {
        let topology = self.inner.topology().map_err(code)?;
        self.resize_supported = topology.displays.iter().any(|d| d.can_resize);
        topology
            .displays
            .iter()
            .map(|d| display(d, topology.revision))
            .collect()
    }
    fn capture_next(
        &mut self,
        id: &str,
        cursor: bool,
        deadline: Instant,
    ) -> backend::Result<Frame> {
        let frame = self
            .inner
            .capture_next(id, cursor, deadline)
            .map_err(code)?;
        Ok(Frame {
            display_id: frame.display_id,
            topology_revision: frame.topology_revision,
            captured_at: frame.captured_at,
            width: frame.width,
            height: frame.height,
            cursor_embedded: frame.cursor_embedded,
            rgba: frame.rgba,
        })
    }
    fn inject(
        &mut self,
        revision: u64,
        id: &str,
        action: Input,
        guard: &mut dyn ActionGuard,
    ) -> backend::Result<()> {
        self.inner
            .inject(revision, id, input(action)?, &mut GuardAdapter(guard))
            .map_err(code)
    }
    fn resize(
        &mut self,
        revision: u64,
        id: &str,
        requested: Resolution,
        guard: &mut dyn ActionGuard,
    ) -> backend::Result<ResizeResult> {
        let outcome = self
            .inner
            .resize(
                revision,
                id,
                capture::Resolution {
                    width: requested.width,
                    height: requested.height,
                },
                &mut GuardAdapter(guard),
            )
            .map_err(code)?;
        let actual = outcome
            .topology
            .displays
            .iter()
            .find(|d| d.id == id)
            .ok_or("capture_failed")?;
        let (applied, reason) = match outcome.status {
            capture::ResizeStatus::Applied => (true, "none"),
            capture::ResizeStatus::Rejected(capture::ResizeRejection::UnsupportedMode) => {
                (false, "unsupported_mode")
            }
            capture::ResizeStatus::Rejected(capture::ResizeRejection::OsFailed) => {
                (false, "os_failed")
            }
            capture::ResizeStatus::Rejected(capture::ResizeRejection::RollbackFailed) => {
                (false, "rollback_failed")
            }
        };
        Ok(ResizeResult {
            display: display(actual, outcome.topology.revision)?,
            applied,
            reason,
        })
    }
    fn release_all(&mut self) -> backend::Result<()> {
        self.inner.release_all().map_err(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maps_input_without_changing_coordinates_or_semantics() {
        assert_eq!(
            input(Input::Pointer {
                x: 3839,
                y: 2159,
                action: "click".into(),
                button: "left".into()
            })
            .unwrap(),
            capture::InputAction::Pointer {
                x: 3839,
                y: 2159,
                action: capture::PointerAction::Click(capture::Button::Left)
            }
        );
        assert!(
            input(Input::Pointer {
                x: 0,
                y: 0,
                action: "move".into(),
                button: "left".into()
            })
            .is_err()
        );
        assert_eq!(
            input(Input::Key {
                usage: 4,
                down: true
            })
            .unwrap(),
            capture::InputAction::Key {
                usage: 4,
                down: true
            }
        );
    }
    #[test]
    fn errors_are_stable_and_do_not_expose_backend_details() {
        assert_eq!(
            code(capture::BackendError::new(
                capture::ErrorKind::StaleTopology,
                "private OS detail"
            )),
            "stale_topology"
        );
        assert_eq!(
            code(capture::BackendError::new(
                capture::ErrorKind::Cancelled,
                "private OS detail"
            )),
            "expired"
        );
    }
    #[test]
    fn rejects_native_oversize_and_filters_unrepresentable_modes() {
        let mut raw = capture::Display {
            id: "d".into(),
            x: 0,
            y: 0,
            width: 3840,
            height: 2160,
            primary: true,
            can_resize: true,
            modes: vec![
                capture::Resolution {
                    width: 7680,
                    height: 4320,
                },
                capture::Resolution {
                    width: 1920,
                    height: 1080,
                },
            ],
        };
        assert_eq!(display(&raw, 7).unwrap().available_resolutions.len(), 1);
        raw.width = 7680;
        assert!(display(&raw, 7).is_err());
    }
}
