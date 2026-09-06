# X11 capture/input component

Original `dwdesktop-capture` Rust library for M1a local contract 0.2.0. This is a tested component, not a complete daemon or hardware-validated M1 result. No RustDesk source, assets or implementation were copied or adapted.

## Integration seam

`src/lib.rs` defines the exact public types agreed in `docs/backend-api.md`. One owner serializes `X11Backend` calls. Core supplies authorization and an `ActionGuard`; the backend never accepts user IDs, shell commands, grants or wire messages.

```rust,no_run
use dwdesktop_capture::{BackendOptions, X11Backend, encode_png, FrameLimits};
use std::time::{Duration, Instant};
let mut backend = X11Backend::connect(BackendOptions::default())?;
let topology = backend.topology()?;
let display = topology.displays.first().ok_or("no active display")?;
let frame = backend.capture_next(&display.id, false, Instant::now() + Duration::from_secs(5))?;
let png = encode_png(&frame, FrameLimits::default())?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Do not run this example without authorization to observe the selected console. `BackendOptions.display` and Xauthority come from trusted service configuration. Only local Unix X11 sockets are accepted; TCP/SSH-forwarded X displays are excluded. An empty display option uses the service's DISPLAY. Missing connection, extensions, modes or input capability returns an error, never a synthetic desktop.

Public action enums:

- `InputAction::Pointer { x, y, action }`, `PointerAction::{Move, Down(Button), Up(Button), Click(Button)}`, `Button::{Left, Middle, Right}`.
- `InputAction::Key { usage, down }`: USB HID page 7 usage resolved through actual XKB physical key names.
- `InputAction::Text(String)`: preflighted group-zero keysyms, neutral modifiers required. Unsupported characters/layout state reject the entire plan before injection.
- `ActionGuard::check() -> Result<(), BackendError>`: called immediately before every input action and every requested mode-mutation step. Core must recheck the live lease/deadline/cancellation state in this callback. It must not wait for another thread blocked by the backend owner.
- `ResizeOutcome { status, topology }`, where status is `Applied` or `Rejected(UnsupportedMode | OsFailed | RollbackFailed)`.

Core maps backend errors into the wire error taxonomy. `NativeFrame.captured_at` is a process monotonic `Instant` taken immediately before requesting acquisition, not an exact scanout timestamp. Core converts it using the session's monotonic origin and owns all snapshot/session identifiers. Core must keep frame, PNG and input bodies out of logs.

## Implemented boundaries

Capture uses one full native display source (`GetImage`) intended for later stream encoding, converts actual endian/pixel stride/TrueColor masks into packed RGBA8, and rejects non-contiguous/overlapping masks, channels wider than eight bits and unsupported formats. No scaling, PNG tiling or separate screenshot utility. 16/24/32-bit storage is supported; HDR/10-bit channels are explicitly unsupported.

Frames are independently bounded to 4096 pixels per dimension, 64 MiB decoded RGBA, and 64 MiB encoded PNG. A capped writer enforces encoded size during generation. Topology is read before/after acquisition and stale captures are rejected. Notifications and configuration timestamps invalidate observations, including geometry changes away and back.

Cursor composition is not implemented: callers must request `include_cursor=false`; true returns Unsupported. The captured root raster excludes the X11 hardware cursor. Application-painted cursor imagery is part of the application framebuffer.

Input checks display-local physical coordinates without rescaling. Each OS action rechecks topology and the supplied guard. Held keys/buttons are tracked before sending because failed delivery is ambiguous; any injection failure attempts release, and explicit `release_all`/drop also attempts cleanup. Failed cleanup propagates an error; core must fault control closed. This does not prevent a physically present user changing focus or typing concurrently.

Text resolves group-zero symbols for neutral and Shift-only state through the live XKB key types and symbols, using a verified physical Shift modifier. It rejects locked/latched/active modifiers, other groups and key types depending on virtual modifiers. Arbitrary Unicode composition, IMEs, wheel injection and clipboard are not implemented. Physical HID keys include common letters/digits/punctuation/modifiers/navigation/F1–F12; unsupported usages fail explicitly.

Resize supports an existing advertised mode on one active output/CRTC at origin zero, unrotated, no transform or panning. Multi-output repacking, clones, mode creation and driver configuration are unsupported. It saves CRTC/output/rotation/root size and physical dimensions, grows the framebuffer before increasing the mode, shrinks it after reducing the mode, confirms OS results and attempts restoration on failure. Cleanup after cancellation is allowed; it is not renewed authorization for new user input. PTY/session lifecycle belongs to core and is untouched by this library.

Local X11 waits use absolute poll/read/write deadlines. Capture is capped to the supplied deadline or five seconds, whichever is earlier. Topology/input and resize stages use bounded waits; cleanup has its own two-second allowance. A failed or stalled X server can prevent rollback or key release, which is reported rather than hidden. X11 is a trusted local OS service: x11rb's own setup/reply parser allocations are not a hostile-X-server sandbox. Selected output geometry and pixel packing are checked before requesting image data; malicious X11 peers are outside this trust boundary.

## Validation

```sh
cargo fmt --check --manifest-path device/capture/Cargo.toml
cargo test --manifest-path device/capture/Cargo.toml
cargo clippy --manifest-path device/capture/Cargo.toml --all-targets -- -D warnings
```

Tests use synthetic image buffers, Unix socket pairs and fake mode transactions. The native 3840×2160 PNG round-trip is byte-exact synthetic evidence, not proof of hardware capture, encode throughput, input latency or monitor behaviour. No X server is contacted by tests. Physical corner-click accuracy, Spark compatibility, live 4K→1080p→4K rollback and PTY continuity require the authorized integration run.

No browser video, terminal implementation, daemon installation, Keycloak or fleet deployment is included.
