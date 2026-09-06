//! Original X11 capture/input backend. Calls must be serialized by one owner.

mod image;
mod keyboard;
mod resize;
mod transport;
mod x11;

pub use image::encode_png;
use std::{fmt, time::Instant};
pub use x11::X11Backend;

#[derive(Clone, Copy, Debug)]
pub struct FrameLimits {
    pub max_dimension: u32,
    pub max_rgba_bytes: usize,
    pub max_png_bytes: usize,
}
impl Default for FrameLimits {
    fn default() -> Self {
        Self {
            max_dimension: 4096,
            max_rgba_bytes: 64 * 1024 * 1024,
            max_png_bytes: 64 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct BackendOptions {
    pub display: Option<String>,
    pub limits: FrameLimits,
}
#[derive(Clone, Copy, Debug)]
pub struct BackendCapabilities {
    pub capture: bool,
    pub input: bool,
    pub cursor_composition: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Display {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    pub can_resize: bool,
    pub modes: Vec<Resolution>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topology {
    pub revision: u64,
    pub displays: Vec<Display>,
}
#[derive(Debug)]
pub struct NativeFrame {
    pub display_id: String,
    pub topology_revision: u64,
    pub captured_at: Instant,
    pub width: u32,
    pub height: u32,
    pub cursor_embedded: bool,
    pub rgba: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerAction {
    Move,
    Down(Button),
    Up(Button),
    Click(Button),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputAction {
    Pointer {
        x: u32,
        y: u32,
        action: PointerAction,
    },
    Key {
        usage: u16,
        down: bool,
    },
    Text(String),
}
/// Invoked immediately before each OS input action. Core owns authorization.
pub trait ActionGuard {
    fn check(&mut self) -> Result<()>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeRejection {
    UnsupportedMode,
    OsFailed,
    RollbackFailed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeStatus {
    Applied,
    Rejected(ResizeRejection),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResizeOutcome {
    pub status: ResizeStatus,
    pub topology: Topology,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Unavailable,
    Unsupported,
    InvalidArgument,
    StaleTopology,
    NotFound,
    ResourceExhausted,
    CaptureFailed,
    InputFailed,
    Cancelled,
    DeadlineExceeded,
    OsFailed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendError {
    pub kind: ErrorKind,
    pub message: String,
}
impl BackendError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    pub(crate) fn os(error: impl fmt::Display) -> Self {
        Self::new(ErrorKind::OsFailed, error.to_string())
    }
}
impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for BackendError {}
pub type Result<T> = std::result::Result<T, BackendError>;
