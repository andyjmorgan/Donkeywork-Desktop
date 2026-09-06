# Local daemon core — WP01 initial slice

Standalone Rust package implementing the local `dwdesktop.local` 0.2.0 contract. Original implementation, with no RustDesk source dependency. Outbound project licence remains undecided.

Build/test from the repository root:

```sh
cargo test --manifest-path device/core/Cargo.toml
cargo run --manifest-path device/core/Cargo.toml -- --config /absolute/path/to/config.json
```

Configuration is a service-user-owned regular file, not group/world writable, at most 64 KiB. Example shape (replace the UID, UUID and paths explicitly):

```json
{
  "socket_path": "/run/user/1000/dwdesktop/cli.sock",
  "worker_id": "11111111-1111-4111-8111-111111111111",
  "backend": { "kind": "x11", "display": ":0" },
  "policies": [
    { "uid": 1000, "profile": "pilot", "permissions": ["desktop.view", "desktop.control", "desktop.resize"] }
  ]
}
```

Create the dedicated parent directory separately with service ownership and mode 0700. The server refuses unsafe parents and existing targets; it does not delete stale sockets on startup. Created sockets have mode 0600. This first package consequently supports the same service UID through filesystem access; another mapped UID is still blocked by socket permissions. No root-account switching, remote port, broker trust or implicit root exemption is provided. The fixed profile is retained in authorization state, but OS-account dispatch remains the responsibility of the integrated backend.

Select the real WP02 adapter with `backend: {"kind":"x11","display":":0"}`. It connects to the configured local X11 display and uses the service's preconfigured `XAUTHORITY` path (or the X11 library's normal user authority lookup). If display is null, the service's `DISPLAY` is used. Set these in the launching service environment; the daemon does not mutate environment variables after threads start and clients cannot supply them. Startup fails on unavailable/unauthorized X11; there is no silent mock fallback. Use includeCursor=false because the current capture backend explicitly does not implement cursor composition. Inspect `device/capture/README.md` for RandR, keyboard and pixel-format limitations.

Omitting backend or setting `backend: {"kind":"unavailable"}` explicitly runs the transport-only backend: describe reports no supported capabilities and session creation fails unsupported_capability. The core encodes validated native RGBA frames using the png library with a bounded writer; WP02 supplies acquisition and real OS state. WP05 terminal implementation is not wired; terminal requests fail unsupported_capability.

Implemented: actual Linux peer credentials, canonical offline JSON Schema validation, bounded framed IO, session/epoch/UID checks, replay-safe lifecycle/resize result caching, exclusive connection-bound control, fresh snapshot metadata and geometry validation, cursor agreement, bounded RGBA/PNG encoding, guarded backend input, real-mode confirmation, idle lease maintenance and disconnect cleanup. The backend must honour capture deadlines and check ActionGuard immediately before each OS action. A failed release faults control closed. The server serializes backend work using one core mutex; only 16 connections are admitted and each connection processes one request at a time. This intentionally stays below the protocol's in-flight maximum. A 30-second incomplete/idle request deadline and five-second write deadline bound stalled peers.

Tests use a deliberately synthetic 4×4 backend for capture/input/resize. They verify PNG decoding to exact pixels, fresh-frame rejection, stale geometry, replay rejection and session invariants. Unix socket tests verify the actual current peer UID, denial of an unmapped UID, framing rejection, unsafe-directory refusal and preservation of existing files. These are not X11, 4K performance or hardware acceptance tests.

Known integration limits: policy is loaded at startup; the library exposes revoke_uid, but live configuration reload and a management revocation endpoint are not wired. Terminal cleanup is absent because no PTY exists. Input cancellation monitors peer closure and checks backend deadlines; buffering another request can postpone observing EOF until queued bytes are drained, so no hostile-pipelining cancellation guarantee is claimed in this first slice. Capture backend calls must remain bounded; Rust cannot safely preempt an arbitrary blocking implementation. Policy updates should be wired to cancellation before taking the core lock during integration. Ctrl-C and SIGTERM trigger graceful connection cancellation, input release and owned-socket cleanup.

Dependencies: serde/serde_json (MIT OR Apache-2.0), tokio (MIT), uuid (Apache-2.0 OR MIT), jsonschema (MIT), png (MIT OR Apache-2.0), libc (MIT OR Apache-2.0), tempfile for tests (MIT OR Apache-2.0). Cargo.lock records resolved versions; transitive licence auditing remains a release task. No source from the reference project was copied.
