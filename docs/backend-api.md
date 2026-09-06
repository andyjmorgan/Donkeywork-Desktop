# M1a capture/core Rust seam

Integrator-approved initial implementation seam, reviewed with capture owner. WP02 owns the standalone `device/capture` library (`dwdesktop-capture` crate). WP01 depends on it only at integration; until then use an explicit unavailable backend or test double. No copied RustDesk implementation.

Public types:

```rust
pub struct FrameLimits { pub max_dimension: u32, pub max_rgba_bytes: usize, pub max_png_bytes: usize }
pub struct BackendOptions { pub display: Option<String>, pub limits: FrameLimits }
pub struct Resolution { pub width: u32, pub height: u32 }
pub struct Display {
    pub id: String, pub x: i32, pub y: i32, pub width: u32, pub height: u32,
    pub primary: bool, pub can_resize: bool, pub modes: Vec<Resolution>,
}
pub struct Topology { pub revision: u64, pub displays: Vec<Display> }
pub struct NativeFrame {
    pub display_id: String, pub topology_revision: u64,
    pub captured_at: std::time::Instant, pub width: u32, pub height: u32,
    pub cursor_embedded: bool, pub rgba: Vec<u8>,
}
```

`X11Backend::connect(BackendOptions)` returns a backend or real error. Methods `topology(&mut self)`, `capture_next(&mut self, display_id: &str, include_cursor: bool, deadline: Instant)`, `release_all(&mut self)` return Result of topology/frame/unit respectively. Free `encode_png(&NativeFrame, FrameLimits)` returns bounded PNG bytes.

`inject(&mut self, expected_revision: u64, display_id: &str, action: InputAction, guard: &mut dyn ActionGuard)` and `resize(&mut self, expected_revision: u64, display_id: &str, requested: Resolution, guard: &mut dyn ActionGuard)` validate current OS facts. InputAction mirrors local-cli pointer/HID/text semantics, not arbitrary shell actions. Capture owner defines and documents exact enum spelling in the first public API commit; core owner reviews before wiring. ActionGuard checks cancellation/deadline immediately before every OS input action; core supplies authorization. Guard failure must stop pending actions and release held input. Cleanup/rollback remains allowed after cancellation.

Resize returns applied/rejected reason plus fresh actual topology, including rollback failure. Never guess original geometry after OS failure. A single backend actor serializes capture, injection and mode changes. Core owns UID policy, session epoch, snapshot IDs, sequence validation and conversion from Instant to session-relative capture time. Backend owns observed topology revisions, including external display changes.

Defaults: max dimension 4096; decoded RGBA and encoded PNG separately bounded to 64 MiB. Read topology before/after acquisition and discard raced captures. Actual X11 visual masks, endian, stride and pixel packing must be handled or rejected explicitly. Use one source for stills now/video later. Start with bounded GetImage; SHM is a later optimization, not a second semantic capture path.

Use XTEST with actual keyboard mapping, preflight text before injection, and reject unsupported Unicode/layouts. Cursor inclusion requires real composition or an explicit unsupported result. Initial RandR resize can reject complex clone/transform/repacking cases; preserve full affected state and report actual rollback outcome. A restricted backend does not waive required real pilot 4K/1080p evidence.
