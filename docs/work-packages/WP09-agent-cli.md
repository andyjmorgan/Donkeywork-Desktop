# WP09 — M1a authenticated local agent CLI

## Outcome and ownership

Implement the original CLI in `cli/`; branch `work/wp09-agent-cli`, separate worktree. This is a hard M1a deliverable and one of three initial lanes with WP01 core and WP02 capture. WP05 adds daemon PTYs next. The CLI owns no capture/auth backend or broker implementation.

## Scope

- Implement the reviewed command surface from `contracts/local-cli.md`, draft v0.2.0: describe/topology, screenshot, coordinate click, keyboard/text input, actual display resize and interactive terminal. Exact flags, envelopes and errors follow the frozen contract; they are not already implemented APIs.
- Connect only to the dedicated service-owned local Unix socket. Daemon-verified peer UID, explicit allowlist and fixed permissions/account profile authorize actions. Do not send self-asserted principal/account/permissions or use raw broker-only IPC.
- Fetch topology/screenshots under `desktop.view` without a control lease. Receive bounded binary PNG data separate from control JSON, validate framing/metadata and write the requested artifact without logging image bytes.
- For input and resize, acquire/renew/release the approved local control lease and bind commands to the screenshot's session/epoch/display/topology revision. Reject missing/stale context visibly; never silently rescale coordinates or replay uncertain clicks/text.
- Expose actual advertised modes and pending/success/failure results; report actual resulting topology when resize fails. Do not present screenshot scaling as an OS mode change.
- Support PTY bytes/resize/exit and the approved retention policy through WP05/WP01. Keep text, clipboard, PTY and credential material out of diagnostics. Restore the user's local terminal mode on exit/error/signals.
- Provide script-friendly structured metadata/errors and exit statuses according to the contract. Long-running scripted control must renew its lease; an abandoned lease must expire cleanly.

## Prerequisites and blockers

Freeze v0.2.0 local command/auth/framing/lease/terminal semantics before merge. Missing command/API decisions require a contract PR. Approved fixture server responses support early parsing/tests; real acceptance requires WP01/WP02/WP05. No .NET, Keycloak, browser grants, remote device-flow login, HTTP API or video encoding in M1a.

## Acceptance

Test allowed/denied identity responses, socket safety expectations, unknown/error replies, screenshot payload limits, stale topology, out-of-bounds coordinates, interrupted lease renewal and ambiguous failure without action replay. A view-only principal observes without control and cannot issue input. Tests distinguish client checks from authoritative daemon enforcement.

On a separately authorized X11 pilot: capture native 4K, click/type a known target, observe effect in a new screenshot, then drive actual `3840×2160 → 1920×1080 → 3840×2160` without reopening the session and while a PTY remains alive. Repeat screenshot/input coordinate checks at both modes. Unsupported pilot mode switching blocks M1a.

## Boundaries and handoff

No live host installations, display changes, credential retrieval or production changes are authorized merely by this issue. Write original code; no RustDesk source/assets/adaptations. Own only `cli/`; shared interfaces/root manifests require integrator coordination. Provide branch, base/commit and contract SHA/version, commands/results, environment, dependency provenance, fixture-versus-real evidence, remaining blockers and exact next integration step. No direct main merge/push.
