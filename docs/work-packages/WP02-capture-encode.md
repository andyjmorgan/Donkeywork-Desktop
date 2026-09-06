# WP02 — Linux capture and 4K encoding proof

## Outcome and ownership

Prove an original capture/encode backend with measurable 3840×2160 behaviour. Own only `device/capture/`; branch `work/wp02-capture-encode`, separate worktree. WP01 owns worker lifecycle; WP03 owns browser rendering. Coordinate through contracts, not cross-directory edits.

## Scope

- Propose the in-process capture/encoder interface with WP01 as a contract amendment, then implement the approved interface. The v0.1.0 draft describes wire metadata, not Rust traits. Preserve explicit display identity, dimensions, scale, timestamps and codec capability reporting.
- Establish one viable Linux capture path first. Identify X11, Wayland, permissions and headless limitations rather than claiming universal support.
- Compare available browser-compatible codec paths for 4K motion and coloured text. Record actual chroma format, software/hardware encoder and fallback behaviour.
- Support keyframe requests, bounded queues and capability/error reporting as agreed in contracts. Treat cursor capture/compositing consistently with the separate-cursor contract.
- Keep lossless refinement an explicit later proposal unless the frozen contract includes it. 4:4:4 alone is not proof of losslessness.
- Enumerate actual supported desktop modes as `availableResolutions` and implement live display mode changes through the approved interface behind `display.resize` / `display.resize.result`. A changed capture size, encoder size or browser scale alone is insufficient. Coordinate topology revision and decoder `streamGeneration` updates with WP01/WP03; reject stale input during the transition through the worker boundary. Preserve or restore the previous display mode and report a failed change visibly.

Use original code and appropriately licensed dependencies; no RustDesk code, assets or translation. This package does not authorize software installation, graphical-session changes, driver changes or privileged capture on Spark or any lab host. Prepare a reviewable test invocation and identify the exact host access prerequisite first. No production changes.

## Prerequisites and blockers

Capture/encoder interface, timestamp semantics, codec bitstream format and keyframe/error messages must be frozen before implementation merges. Missing decisions require a contract PR. Synthetic generators can run locally while access to an approved display/encoder is unavailable. WP01 and WP03 are required for live end-to-end results.

## Acceptance

- Deterministic tests cover resolution metadata, capability fallback, frame lifetime, queue limits and keyframe signalling.
- Record codec/profile/chroma, dimensions, frame rate, bitrate, encode timing, CPU/GPU load and workload. Mark synthetic measurements separately from physical-display capture.
- Exercise terminal scrolling, small coloured text and motion; retain artifacts without credentials or private desktop content.
- State whether 4K60 is achieved, under which conditions, and what prevents it elsewhere. Never infer hardware encoding from GPU presence.
- On the approved pilot, verify actual OS/display resolution changes `3840×2160 → 1920×1080 → 3840×2160` during one desktop session, without reconnecting or interrupting its terminal. Publish evidence of actual display modes and stream reconfiguration. Unsupported live resize on the pilot is an M1 blocker, not an optional capability that can be skipped.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, commands/results, environment, benchmark artifacts, dependency/provenance notes, limitations and explicit blockers. Include the interface integration procedure for WP01 and decodable samples for WP03 using approved fixtures. If hardware access is missing, deliver the implementation and test plan with that result explicitly unverified.
