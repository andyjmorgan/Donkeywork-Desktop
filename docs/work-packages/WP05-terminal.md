# WP05 — Rust PTY and browser terminal package

## Outcome and ownership

Deliver terminal access under the same session authorization as desktop access. Own only `device/terminal/` and `browser-terminal/`; branch `work/wp05-terminal`, separate worktree. WP01 owns lifecycle integration and WP06 owns React page composition. Export an integration API; do not edit their paths.

## Scope

- Implement local Linux PTY creation, byte streaming, resize, exit and cleanup using `contracts/` v0.1.0 draft terminal semantics.
- Run as the explicitly configured local identity. Do not implement arbitrary user switching, root escalation or accept an uncontrolled shell/environment from the browser.
- Build an importable xterm.js adapter with bounded buffers, resize handling, connection state and the approved reconnect behaviour.
- Respect byte-stream encoding and terminal control sequences; terminal output is data, not HTML.
- Require authorized terminal capability. Keep grant validation and worker ownership aligned with WP01/WP04.
- Define process-group cleanup and distinguish reconnect to an existing authorized PTY from creation of a new process according to the contract.

No SSH fleet discovery, MCP shell tools, secret delivery or production changes. Write original code; no RustDesk source or translations. Record xterm.js and other dependency licenses.

## Prerequisites and blockers

Freeze terminal framing, maximum sizes, flow control, identity, resize, reconnect and disconnect lifetime before merge. Missing semantics need a contract PR. Local PTY and fixture browser tests can proceed before WP01/WP04; end-to-end authorization cannot.

## Acceptance

- Tests cover UTF-8 split across chunks, binary/control bytes, resize, process exit, disconnect, repeated close and process-group cleanup.
- Bound memory under high output and slow consumers; expose defined errors rather than silently truncating unless contract permits it.
- Verify denied/expired/wrong-session access cannot attach to or write a PTY through integrated authorization tests.
- Test browser disposal, safe rendering and resize without injecting production commands or secrets.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, commands/results, environment, dependency/provenance notes, known shell/platform limitations and package APIs for WP01/WP06. Separate local PTY tests from broker-authorized end-to-end results. No physical-host installation or production changes are authorized.
