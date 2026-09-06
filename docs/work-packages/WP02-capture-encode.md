# WP02 — M1a Linux capture/stills/input; M1b video

## Outcome and ownership

Own `device/capture/` on `work/wp02-capture-encode` in a separate worktree. M1a implements one original real capture source, lossless still encoding, input injection and actual mode switching. M1b adds video encoding to **the same source**, not an independent screenshot path. WP01 owns authorization/lifecycle; WP09 owns CLI commands.

## M1a scope

- Propose and freeze the in-process capture/still/input/resize interface with WP01 through a contract PR. `contracts/local-cli.md` draft v0.2.0 supplies wire semantics, not implemented Rust traits.
- Target an X11 pilot with RandR 1.2+, both required modes and usable capture/input permissions. Spark is a candidate, not a validated baseline. Document Wayland/headless limitations.
- Capture native display frames and return a PNG from that source with accurate post-rotation dimensions, display identity, capture timestamp, cursor policy and topology revision. Respect contract payload caps and explicit oversize failure; never silently downsample.
- Inject coordinate pointer, keyboard and text actions through the approved interface. Preserve physical pixel coordinates and reject out-of-bounds/stale topology at the worker boundary.
- Enumerate supported modes and change actual OS display resolution in place. Serialize transition with capture/input, preserve/restore prior mode on failure where possible, and always report the actual resulting topology.
- Do not shell out to a separate screenshot utility or add a second capture implementation for stills.

## M1b deferred scope

Add browser-compatible video encoding, keyframes, bounded queues, codec/chroma metadata and generation reconfiguration around the proven source. Compare hardware video and software full-chroma paths on measured hardware. Browser 4K60 and codecs are M1b proof, not M1a prerequisites. PNG refinement is a separate proposal. Never infer encoding support from GPU presence.

## Prerequisites and acceptance

Freeze the v0.2.0 still/input interface and limits before merge. Real acceptance needs separately scoped access to an X11 display exposing `3840×2160` and `1920×1080`; missing capability is an M1a blocker. Test locally with synthetic frames until authorized; label that evidence.

Decode native-4K screenshots and verify exact reported dimensions, cursor policy, topology and bounded payload behavior. Use a known test pattern for corner-pixel input at both modes. Prove `3840×2160 → 1920×1080 → 3840×2160` independently from OS mode evidence, without reopening desktop or disrupting PTY. Test unsupported modes and failure/rollback reporting.

In M1b, compare a screenshot and decoded video from the same captured frame within codec tolerance, using matching source dimensions/cursor semantics. Record frame rate, bitrate, latency method and CPU/GPU load; no synthetic-to-hardware claims.

## Boundaries and handoff

No host installation, driver/display modifications, privileged access or production changes are authorized by this package. Original code only; no RustDesk source/assets/adaptations. Provide branch, base/commit and contract SHA/version, commands/results, environment, artifacts, dependency provenance, actual-vs-synthetic evidence and explicit blockers. Coordinate WP01/WP09 first and WP03 only for M1b; own no other paths.
