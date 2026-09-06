# Architecture and open decisions

## Boundaries

### M1a decision — supersedes browser-first sequencing

Agent CLI -> dedicated Unix socket -> Rust daemon -> shared X11 capture/input backend and PTY. The CLI socket is NOT the trusted broker IPC socket described below. Kernel peer UID plus an explicit administrator-owned allowlist maps callers to capabilities and fixed OS profiles. Deny unmapped UIDs, check socket ownership/permissions, and never trust a caller-supplied UID/account. Run with the minimum required graphical-session privileges; do not give shell users the supervisor's root identity.

The local CLI contract is separately versioned in contracts/local-cli.md. Existing 0.1.0 broker/media contracts below remain M1b drafts, not the local CLI authentication protocol. Snapshot observation requires desktop.view, not a control lease; input and mode changes require explicit control. Do not lengthen fail-closed deadlines for CLI convenience.

Capture supplies one canonical native framebuffer source for still PNGs now and video later. Screenshot binary payloads have their own bounded size; the 16 MiB video limit is not a PNG guarantee. Screenshot identity/time/topology describe the observation; stale topology is rejected before input, but unchanged topology does not guarantee an application target has not moved. No automatic replay of uncertain clicks/text.

M1a targets X11 with RandR mode support, not generic Wayland. Spark is only a proposed pilot. Read-only mode inventory and mutating mode-switch tests are distinct tasks; neither lab access nor display changes are authorized by delegation. M1a completion requires real hardware evidence, not fixtures.

### M1b broker/browser route

Browser UI -> browser-session/browser-terminal packages -> authenticated broker attachment -> device worker. The local worker connection is a Unix-domain socket; a future remote adapter must preserve the same authorization semantics.

The broker owns Keycloak identity and policy. The worker enforces permitted operations and owns OS resources. Watching a session, attaching to a session and controlling input are separate capabilities.

M1's reference security route redeems a one-use attachment grant through the authenticated broker, then authorizes the worker connection over trusted IPC. Media may initially pass through the broker as opaque encoded bytes; it must not decode/re-encode them. A direct media adapter requires worker-side authenticated binding to that attachment, with its own reviewed transport design. Do not expose a raw media socket and rely on the portal login.

## Decisions

- .NET 10 broker, Rust worker, React/TypeScript browser.
- IPC control: length-prefixed UTF-8 JSON.
- Exact draft contract: 0.1.0. An implementation cannot silently accept a different version.
- Capture/encode library and media transport are not selected by schema presence.
- M1b full-display video; no hybrid PNG/video rectangle compositor. M1a PNG stills are not a video codec.
- Lossless convergence remains a quality option if baseline video fails; 4:4:4 is not lossless.
- No B-frame/reordered-frame support in v0: decode and presentation order must match.
- Local physical input is outside the broker lease. Remote takeover does not prevent a person using the device keyboard.

## Cross-component seams

WP01 defines component hooks without requiring a capture implementation: describe capabilities, start/stop an authorized session, publish ordered events. Capture produces stream configuration plus encoded access units; input consumes validated display-local actions. PTY produces byte chunks plus exit and accepts ordered input/resize. Component implementations must not make policy choices.

WP02 and WP03 must agree on one tested codec/profile/bitstream pair before claiming interoperability. Codec configuration changes reset decoder state via a new stream generation.

WP04 and WP01 must agree on IPC serialization and reconnect/authorization behavior before live attachment. WP05 should deliver an independently importable browser-terminal package; WP06 owns wiring it into web/.

## M1 auth defaults (reviewable baseline)

Use a dedicated Keycloak client/audience. Browser login uses code+PKCE, state/nonce, server-side session and Secure/HttpOnly cookies. Protect state changes against CSRF and WebSocket upgrades with Origin checks. Do not clone legacy frontend token storage or treat azp alone as a generic substitute for audience validation.

An opaque random 256-bit attachment grant is valid for at most 30 seconds, one use, principal/session/epoch/permissions bound. Store only its SHA-256 hash. The grant must stay out of URLs and logs. Its expiry is measured against the worker's monotonic deadline established at prepare.

Active attachment authorization renews every 15 seconds with a maximum 45-second worker deadline. Revocation closes promptly; missed renewal fails closed at deadline. These are contract defaults, not production tuning evidence.

Terminal disconnect retention: at most 120 seconds; 4 MiB replay buffer maximum. Explicit revocation terminates the process group immediately. A reconnect needs new authorization and must not blindly resend unacknowledged input.

## Unresolved decisions

Capture/encoder availability, wire media adapter (WebRTC media vs framed stream), browsers beyond Chromium, final performance thresholds, outbound licence, real minigpu inventory, Wayland/headless strategy. Resolve using bounded spikes and publish evidence; do not use stub success responses.

## Live desktop resolution changes

Hard M1 requirement: an authorized UI resize changes the host display mode during the active session. Enumerate supported resolutions, require desktop.resize and the current control lease, bind the request to display/topology, and expose applied/rejected results with actual resulting dimensions. Release held inputs and suspend coordinate-bearing input during transitions. Successful resize increments topologyRevision, then reconfigures affected streams with a new streamGeneration and keyframe. Preserve sessionId/sessionEpoch and PTY. On failure preserve or restore the previous mode; return the actual state and a visible error. A pilot without a usable mode-switch implementation is not an acceptable M1 completion.
