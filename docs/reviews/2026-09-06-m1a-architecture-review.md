# Architecture review: M1a/M1b readiness (Fable, 2026-09-06)

Independent review of implementation readiness against the superseding scope:

- M1a: Linux daemon plus authenticated agent CLI — native-4K screenshots, coordinate clicks and keyboard input, actual live resolution changes, terminal access.
- M1b: browser streaming and the web console.

This scope supersedes the current docs, which still describe a browser-first M1. Review basis: docs/project-brief.md, docs/architecture.md, docs/acceptance.md, contracts/ draft 0.1.0 (all four schemas, 46 message types, fixtures, model.mjs), all eight work packages, AGENTS.md.

## Verdict

The contract set is strong for M1b: session lifecycle, attachment grants, media framing, input, resize and terminal semantics are unusually well specified for a draft. But M1a is not implementable from this repo today. The agent-CLI plane — its authentication, its API surface, and the screenshot operation itself — has no contract at all, and the work-package graph still assigns ownership as if the browser ships first. Fix the eight items below (the first four are hard blockers) before splitting implementation.

## Ranked blockers and findings

### 1. No screenshot contract exists (blocker)

The schemas define 46 message types; none captures a still image. "png" appears once, only as a cursor-shape format (media.schema.json, ~line 1070). There is also a hard framing conflict: control IPC caps messages at 1 MiB, and a native 4K PNG runs 5 to 20 MiB. A screenshot reply physically cannot ride the control channel as currently framed.

Amendment: add `screenshot.request` / `screenshot.result`.

- request binds displayId, format (png first), includeCursor, and requires the `desktop.view` permission
- result reports width, height, format, cursorEmbedded, topologyRevision, sessionId, sessionEpoch, captureTimeUs and payloadBytes
- image bytes travel on the framed media adapter (its 16 MiB frame cap fits), or as an HTTP response body once the broker API exists; never base64 inside control JSON

Acceptance tests: a 3840×2160 screenshot decodes to exactly the reported dimensions; topologyRevision matches the current `display.topology`; an oversized or undeclared payload closes the stream rather than truncating; a request without `desktop.view` is rejected.

### 2. No CLI authentication flow (blocker)

Every auth decision in architecture.md is browser-shaped: code+PKCE, state/nonce, server-side session, Secure/HttpOnly cookies, CSRF and Origin checks. None of it applies to a headless CLI. The 30-second one-use attachment grant and cookie session cannot be driven from a script.

Amendment: add a CLI auth section to architecture.md and the contract README.

- local path (M1a phase 1): CLI connects to the worker's service-owned Unix socket; identity is the verified peer UID plus socket permissions, which the contract already requires validating
- remote path (M1a phase 2): OAuth 2.0 device authorization grant (RFC 8628) against a dedicated Keycloak client and audience — same issuer/audience/expiry validation rules as the browser; bearer token to the broker over HTTPS
- token cache on disk with 0600 permissions; tokens never in argv, environment listings or logs; refresh handled by the CLI, revocation fails closed

Acceptance tests: expired token, wrong audience, wrong issuer, replayed grant and revoked session each fail closed with a machine-readable error; no credential material appears in broker or worker logs under debug verbosity.

### 3. Broker HTTP/WebSocket API is undefined but now gates M1a (blocker)

WP04 explicitly defers the browser-facing HTTP/WebSocket API to a future contract-only PR. Under the old scope only the browser needed it; under the new scope the remote CLI needs it in M1a. Nothing in contracts/ describes an HTTP surface, error mapping, or how attachment semantics translate to HTTP/WebSocket.

Amendment: decide the M1a transport split explicitly, then write the smaller contract first.

- recommended: M1a phase 1 is local-socket only (CLI on the pilot host, no broker in the loop); phase 2 adds a minimal broker HTTP API — authenticate, list capabilities, create/close session, screenshot, input batch, resize, terminal WebSocket
- map worker error codes 1:1 to HTTP problem responses; do not invent a second error taxonomy

Acceptance tests: unauthenticated requests to every endpoint fail without side effects; a WebSocket upgrade without a valid session cookie or bearer token is refused before any worker IPC occurs.

### 4. Screenshot-to-click binding is unspecified (blocker)

The streaming contract binds input to topologyRevision, display-local physical pixels and the post-rotation raster — good. But an agent acts on a screenshot taken seconds earlier. Nothing says a click can, or must, be rejected when the topology changed after the screenshot, and nothing defines whether screenshot pixels include the cursor.

Amendment:

- `screenshot.result` carries everything a subsequent click needs: displayId, width, height, topologyRevision, cursorEmbedded
- agent-context pointer/key/text input binds the topologyRevision it believes current; the worker rejects mismatches with `stale_topology` and never rescales coordinates silently
- clicks and typed text are never auto-replayed after an ambiguous failure (extend the existing input rule to the CLI explicitly)

Acceptance tests: click each extreme corner pixel at 4K and at 1080p and verify landing position with an on-screen test pattern; a click carrying the pre-resize topologyRevision is rejected; a click at coordinates outside current dimensions is rejected by semantics, not just schema bounds.

### 5. Live resolution change pins the pilot to X11, and Spark is unverified (blocker)

The hard resize requirement (3840×2160 → 1920×1080 → 3840×2160, session and PTY preserved) is generically achievable only on X11 via RandR. Wayland has no cross-compositor client protocol for changing output modes; it needs compositor-specific interfaces or a virtual output. The docs already list Wayland/headless as unresolved — the resize requirement quietly resolves it: M1 targets X11. Say so. Separately, Spark's mode list, EDID situation and 4K behaviour are unverified, and RandR only offers modes the connected monitor (or a forced/dummy configuration) advertises. 1920×1080 may be absent on some panels.

Amendment: record "M1 pilot requires an X11 session with RandR 1.2+" in architecture.md. Authorize a bounded, read-only spike on the pilot: capture `xrandr --query` output, confirm both required modes exist, measure mode-switch time, and confirm the session and running clients survive the switch. Publish the evidence before freezing the baseline.

Acceptance tests: independently captured xrandr output before and after each transition matches `display.resize.result`; the PTY started before the sequence still accepts input after it; an unsupported requested mode returns rejected with the actual current mode.

### 6. The agent CLI owns no work package (delegation conflict)

The ownership table has no home for the CLI: WP01 owns device/core, WP04 owns broker/, WP06 owns web/. Under the new scope, WP03 and WP06 move to M1b, WP02 splits — its capture half is M1a, its encode half is M1b — and both WP01 ("capability reporting") and WP02 ("capture") could plausibly claim screenshots. That is exactly the ownership race the repo's rules exist to prevent.

Amendment: re-cut the ledger before assignment.

- add WP09 (agent CLI) owning `cli/`, depending on the screenshot/input/auth amendments above
- split WP02 into WP02a capture and still-frame encode (M1a) and WP02b video encode (M1b); screenshots are WP02a behind the WP01 interface
- mark WP03 and WP06 as M1b in the work-packages README and the GitHub milestone
- update project-brief.md, README.md and acceptance.md to the M1a/M1b split, and mirror it to the Obsidian record — the brief itself says the two must not diverge

### 7. Screenshots must come from the streaming capture path

The cheap M1a implementation — shell out to a screenshot tool or a one-off X grab — creates a second capture path with its own dimension, rotation and cursor semantics. M1b then builds the real pipeline and the two disagree, invalidating every agent behaviour tuned against M1a screenshots.

Amendment: define `screenshot.result` in the contract as one frame from the same capture source that will later feed `stream.configure`, with identical sourceWidth/sourceHeight and cursor policy. The capture interface WP01/WP02 agree on should expose "give me the next frame as a still" from day one.

Acceptance test (deferred to M1b integration): a screenshot taken during an active stream matches a decoded keyframe of the same instant within codec tolerance.

### 8. Lease and grant machinery is wrong-shaped for a one-shot CLI

The 15-second renewal, 45-second deadline and 30-second one-use grants assume a persistently attached browser. A CLI running one `screenshot` command would need prepare → open stream → redeem → acquire lease → act → release for a single still, or run a background renewal thread — both are unnecessary complexity for M1a.

Amendment:

- M1a local IPC: `desktop.view` operations (screenshot, topology) are per-request, no lease; input and resize still require the control lease
- add an agent lease profile as a contract default (for discussion: renew 60 seconds, deadline 180 seconds) so a scripted sequence of clicks does not need a 15-second heartbeat; revocation semantics unchanged and still fail closed
- keep the one-use attachment-grant machinery M1b/browser-only; do not make the CLI implement it

Acceptance tests: a lease that misses its renewal releases held input at the deadline; explicit revoke interrupts an in-flight input batch; a view-only principal cannot acquire control.

## What is right and should not change

- the invariants already written — epoch binding, fail-closed deadlines, no silent resolution substitution, no auto-replay of input, hash-only grant storage, bounded everything — apply cleanly to the CLI and should be inherited, not re-derived
- the terminal contract needs no M1a changes; the CLI can speak it over the local socket as-is
- keeping media/streamGeneration machinery out of M1a entirely is correct; nothing in the M1a plane should reference streams except the screenshot transport framing

## Smallest real-hardware vertical slice

One X11 host, no Keycloak, no .NET broker, no video encode:

1. Rust daemon: xcb/SHM capture of the real display, PNG still, XTEST pointer/key injection, XRandR mode enumeration and switch, one PTY. Unix socket with peer-UID validation.
2. CLI: `describe`, `screenshot`, `click x y`, `type`, `resize WxH`, `term`.
3. Proof: screenshot → click a known target → screenshot confirming the effect, then the full 4K → 1080p → 4K resize with the same loop repeated at both modes and the PTY streaming throughout.

This exercises every high-risk seam — capture fidelity at native 4K, coordinate ground truth, topology binding, live mode switching, PTY survival — with the minimum code. Phase 2 adds the broker, device-flow auth and remote HTTP without touching the proven daemon internals. Note the temporary trust decision explicitly: phase 1 authorizes by peer UID only, which is acceptable on the pilot but is not the M1a completion state.

## Consolidated acceptance additions for M1a

- screenshot dimensions, topology binding and oversize rejection (item 1)
- CLI auth negative matrix: expired/wrong audience/wrong issuer/replay/revoked (item 2)
- unauthenticated HTTP and WebSocket attempts produce no worker IPC (item 3)
- corner-pixel click accuracy at both modes; stale-topology and out-of-bounds click rejection (item 4)
- independently verified xrandr evidence around the resize sequence; PTY continuity (item 5)
- lease deadline release, revoke interruption, view-only cannot control (item 8)
- existing acceptance.md resolution test remains authoritative; M1a runs it from the CLI instead of the web UI

## Explicitly out of scope for this review

No implementation, no deployment, no host changes were made. The Spark spike in item 5 needs Andrew's authorization before anyone touches the host.
