# Co-located local web POC — 0.1

User-approved 2026-09-06 scope override: no Keycloak, fleet, enrollment or separate broker. One internal-IP HTTP application serves React and bridges only the preconfigured local `dwdesktop.local` 0.2.0 Unix socket. This is an unauthenticated trusted-LAN POC; anyone able to reach it can access the dedicated desktop. No public/tunnel exposure. A small Node implementation uses the existing JS toolchain for this disposable bridge; the deferred .NET BFF is not required here.

An access session attaches the pre-existing managed GNOME desktop. Destroy closes the daemon access session and releases input; it does not kill GNOME or applications. Desktop provisioning is not claimed.

## HTTP interface (same origin)

All JSON bodies reject unknown fields and are bounded to 8 KiB. All responses no-store. Non-GET requests require exact configured Origin and `X-Desktop-Request: 1` to reject cross-site form/fetch misuse; reject incorrect Host too. These checks are not authentication. No CORS. No arbitrary command, path, URL, executable, UID or socket choice from HTTP.

- `GET /api/local`: `{version:"0.1",mode:"local",description:<daemon describe payload>,session:null|{id,epoch}}`. Real worker failures return 503, never fixtures. One global access session maximum for the POC, shared by trusted-LAN viewers.
- `POST /api/session`, body `{}`: create once, return `{id,epoch}`; if one exists return409 rather than creating another. Persistent socket connection binds the session. If disconnected, fail closed and require explicit create; never automatically replay input.
- `DELETE /api/session`, body `{}`: close current daemon session, release input, clear state; repeated delete is idempotent. Requests serialize with current work; disconnect cleanup is bounded.
- `POST /api/frame`, body `{sessionId,displayId}`: require current session, fresh capture; return image/png and `X-Desktop-Frame` containing JSON `{snapshotId,displayId,topologyRevision,width,height,captureTimeUs,sessionId,sessionEpoch}`. Native PNG unchanged, maximum64MiB. Frontend one frame request/decode at a time, revoke replaced object URLs, stop on close/unmount, ignore obsolete in-flight results.
- `POST /api/click`, body `{sessionId,snapshotId,displayId,topologyRevision,x,y,button}` with button left/right/middle: single down/up click under a fresh control lease; validate snapshot against current session and daemon. No automatic retries.
- `POST /api/key`, body `{sessionId,snapshotId,displayId,topologyRevision,usage,modifiers}`: single HID key press/release with explicit array of modifiers from ctrl/shift/alt/meta, no duplicates; one lease; reverse modifier release and cleanup on failure. No Unicode/text claim.
- `POST /api/resize`, body `{sessionId,displayId,topologyRevision,width,height}`: fresh lease, daemon actual-mode operation; returns daemon `display.resize.result` payload. The browser pauses input/frames during transition, refreshes description and fresh frame on success or rejection, retains session ID. Never treat CSS fit as remote mode.

Click/key responses `{ok:true}` only after actual ack. Errors `{error:<safe code>}` and meaningful non2xx HTTP, never raw exception/credential/input logs. Mutating operations use a short lease acquire/release on the same persistent socket. Bounded request queue (reject overload), strict daemon framing/correlation, connection timeout and pending PNG bound. On daemon disconnect discard session and close connection so held keys release. No free-running renewals needed; describe/frame requests keep an active view alive. Idle timeout must fail visibly rather than pretend connected.

Node process runs as the dedicated daemon UID. Before connecting require socket and private parent owned by configured service UID, correct file types, no symlinks and no group/world access. Node lacks a portable peer-credential API; filesystem protection is this POC's server-identity boundary, not a claim of SO_PEERCRED verification on the client. The Rust daemon still checks the connecting UID. Trusted same-UID processes are not isolated.

## UI

Existing DonkeyWork light/dark themes. One local-machine page, Create session, live captured display, Destroy access session, fullscreen, fit/native, actual available mode selector. No sign-in, fake registration or fleet actions. An explicit LAN warning and lifecycle explanation stay visible. PNG polling is labelled as such. Terminal and video remain future work. Demo available only by explicit development choice, never automatic live-error fallback.
