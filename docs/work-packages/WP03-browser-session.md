# WP03 — M1b browser desktop session package (deferred)

This package starts in M1b after M1a proves the local daemon/CLI capture, input, live resize and PTY path. It is not an M1a blocker or initial dispatch lane. Reuse the proven capture source; do not create a parallel screenshot/capture path. The v0.2.0 local CLI contract does not define browser APIs or transport.

## Outcome and ownership

Build an original importable TypeScript browser session package. Own only `browser-session/`; branch `work/wp03-browser-session`, separate worktree. WP06 owns the application UI. Export the agreed API instead of editing `web/`.

## Scope

- Implement session negotiation, decoder configuration, frame handling and errors only after a reviewed M1b contract amendment. The v0.1.0 browser draft is prior design, not a frozen implementation API; use the recorded current baseline and coordinate changes with WP01/WP02/WP04.
- Use browser codec capability detection and explicit unsupported-codec feedback. Prefer WebCodecs for the initial approved encoded path; keep transport implementation aligned with the contract decision.
- Render frames at correct source dimensions, support browser scaling and display selection, and translate pointer coordinates correctly across letterboxing and DPI changes.
- Handle keyboard, pointer, cursor, clipboard permissions and reconnect through approved messages and browser security constraints.
- Bound decoder/render queues and discard stale work according to contract semantics. Prevent old session/display frames from contaminating the current view.
- Keep TypeScript responsible for browser APIs. Introduce Rust/WASM only for a justified, licensed, portable component.
- Expose actual mode choices from `availableResolutions` and an API for `display.resize`, requiring the `desktop.resize` permission and active control lease. Process `display.resize.result`, topology revision and decoder `streamGeneration` updates in place; release obsolete decoder/render resources, reject stale input and resume with updated coordinates. Keep desktop resolution distinct from browser scaling. Report failures while retaining/restoring the prior mode, without reconnecting or disturbing the terminal.

No copied RustDesk browser engine, layout, assets or source. No production changes, fleet UI or speculative PNG refinement protocol. Record dependency licenses.

## Prerequisites and blockers

Freeze public package API, media framing/timestamps, decoder configuration, input coordinates, session grants and transport before merge. Missing decisions need a contract PR. Approved fixtures allow package development before WP02 and WP04; real streaming requires their integration. Label fixture demos explicitly and do not claim real media readiness from drawn placeholder frames.

## Acceptance

- Browser tests cover codec rejection, ordering/session reset, queue bounds, display resize and pointer mapping at native/scaled 4K dimensions.
- Tests cover focus loss, key release, disconnect, denied clipboard permissions and malformed control messages.
- A package-local harness consumes approved encoded samples; record browser/version and whether decoding is hardware or software only when observable.
- Provide public API examples and lifecycle/disposal documentation for WP06.
- Test the active-session `3840×2160 → 1920×1080 → 3840×2160` transition, reordered old/new frames, stale input and failed resize. Fixture tests establish handling only; WP07 must verify actual desktop modes. An unsupported pilot cannot pass M1b live-resolution acceptance. Compare decoded video with stills from the same captured source/frame within codec tolerance.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, changed paths, commands/results, browser environment, sample/demo classification, dependency/provenance notes and outstanding integration blockers. Live 4K latency and fidelity remain WP07 acceptance results, not assumptions from sample playback.
