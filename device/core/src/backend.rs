//! Temporary adapter seam. WP02's public API is adapted here at integration, not copied.
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub type Result<T> = std::result::Result<T, &'static str>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    pub display_id: String,
    pub width: u32,
    pub height: u32,
    pub topology_revision: u64,
    pub cursor_embedded: bool,
    pub can_resize: bool,
    pub available_resolutions: Vec<Resolution>,
}
pub struct Frame {
    pub display_id: String,
    pub topology_revision: u64,
    pub captured_at: Instant,
    pub width: u32,
    pub height: u32,
    pub cursor_embedded: bool,
    pub rgba: Vec<u8>,
}
pub enum Input {
    Pointer {
        x: u32,
        y: u32,
        action: String,
        button: String,
    },
    Key {
        usage: u16,
        down: bool,
    },
    Text(String),
}
pub struct ResizeResult {
    pub display: Display,
    pub applied: bool,
    pub reason: &'static str,
}
pub trait ActionGuard {
    fn check(&mut self) -> Result<()>;
}
pub trait Backend: Send {
    fn capabilities(&self) -> Vec<&'static str>;
    fn displays(&mut self) -> Result<Vec<Display>>;
    fn capture_next(&mut self, display: &str, cursor: bool, deadline: Instant) -> Result<Frame>;
    fn inject(
        &mut self,
        revision: u64,
        display: &str,
        input: Input,
        guard: &mut dyn ActionGuard,
    ) -> Result<()>;
    fn resize(
        &mut self,
        revision: u64,
        display: &str,
        requested: Resolution,
        guard: &mut dyn ActionGuard,
    ) -> Result<ResizeResult>;
    fn release_all(&mut self) -> Result<()>;
}
pub struct Unavailable;
impl Backend for Unavailable {
    fn capabilities(&self) -> Vec<&'static str> {
        vec![]
    }
    fn displays(&mut self) -> Result<Vec<Display>> {
        Ok(vec![])
    }
    fn capture_next(&mut self, _: &str, _: bool, _: Instant) -> Result<Frame> {
        Err("unsupported_capability")
    }
    fn inject(&mut self, _: u64, _: &str, _: Input, _: &mut dyn ActionGuard) -> Result<()> {
        Err("unsupported_capability")
    }
    fn resize(
        &mut self,
        _: u64,
        _: &str,
        _: Resolution,
        _: &mut dyn ActionGuard,
    ) -> Result<ResizeResult> {
        Err("unsupported_capability")
    }
    fn release_all(&mut self) -> Result<()> {
        Ok(())
    }
}
