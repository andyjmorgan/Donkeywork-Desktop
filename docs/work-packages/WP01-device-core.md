# WP01 — Rust device/session core and local IPC

## Outcome and ownership

Build the original Rust worker core for a single Linux host. Own only `device/core/`; use branch `work/wp01-device-core` in a separate worktree. Do not edit root manifests, contracts, capture or terminal implementations. Request shared wiring from the integration owner.

## Scope

- Implement worker startup/shutdown, capability reporting and session lifecycle against `contracts/` protocol v0.1.0 draft.
- Provide local authenticated/permission-restricted IPC for the co-located .NET broker, with explicit connection ownership, cancellation and bounded resources.
- Consume the approved capture/encoder and terminal interfaces through test doubles. Keep capture buffers, encoded frames and PTY bytes out of unrelated broker metadata messages.
- Enforce grant-bound operations through the contract's approved authorization boundary; reject missing, invalid, expired or wrong-session authorization rather than trusting browser claims.
- Define cleanup for disconnect, process termination, component failure and repeat close. Report unsupported capabilities honestly.
- Coordinate authorized `display.resize` requests and `display.resize.result` responses for actual live desktop mode changes. Require `desktop.resize` and the active control lease, expose `availableResolutions`, serialize mode transitions, update topology revisions and propagate decoder `streamGeneration` changes. Reject stale input during transition. On failure, preserve or restore the old mode and report the failure; do not reconnect the desktop or interrupt the independent terminal.

No fleet enrollment, unattended host installation, MCP or production changes. No RustDesk code, assets or line-by-line translations; document independent dependency licenses.

## Prerequisites and blockers

Fixture work may use an identified draft commit. Before implementation merges, the integration owner must freeze the baseline. Missing IPC framing, grant-validation responsibility, session state machine, capture/encoder interface or terminal interface blocks the respective integration. Raise a contract PR request; never invent shared fields privately. WP02 and WP05 supply real backends; until then report worker validation as synthetic.

## Acceptance

- Tests cover valid lifecycle, invalid transitions, duplicate close, malformed/oversized IPC, unauthorized operations, cancellation and slow-consumer limits.
- A fake capture backend and fake terminal demonstrate explicit capability negotiation and resource cleanup without requiring a graphical host.
- Unit tests verify no stale session survives shutdown and no unauthorized request reaches a backend.
- Document local development invocation and known platform limitations inside the owned directory.
- Verify resize lifecycle/authorization and failure recovery with fixtures, then integrate the live `3840×2160 → 1920×1080 → 3840×2160` sequence. An unsupported pilot mode-changing capability blocks M1; a synthetic test or scaling operation cannot satisfy actual resolution acceptance.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, changed paths, commands/results, environment, dependency/provenance notes, remaining blockers and WP02/WP04/WP05 integration instructions. Do not claim real desktop performance from fake frames. No physical host or production changes are authorized by this package.
