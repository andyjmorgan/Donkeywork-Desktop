# WP07 — Integration, performance and security acceptance

## Outcome and ownership

Produce reproducible evidence that the single-host MVP works. Own only `tests/integration/`, `tests/performance/`, `tests/security/` and `docs/validation/`; branch `work/wp07-integration-acceptance`, separate worktree. Component owners fix their paths; request root CI and contract-fixture changes separately.

## Scope

- Define the test matrix and evidence format early, then integrate WP01–06 against the frozen `contracts/` v0.1.0 baseline.
- Exercise login, authorized console and PTY, logout/expiry, disconnect/reconnect, display changes and process cleanup.
- Measure 3840×2160 motion, small coloured text and terminal scrolling. Record delivered frame rate, input-to-display methodology, bandwidth, CPU/GPU load, codec/chroma and dropped/stale frames.
- Compare native-scale fidelity with scaled browser viewing; distinguish 4:4:4, lossless pixels and subjective readability.
- Test bounded queues and recovery under controlled latency, bandwidth limits and packet loss in an isolated environment. Include LAN and agreed WAN scenarios.
- Test authorization, grant scope/expiry/replay, origin boundaries, malformed messages, limits, input after logout and log redaction.
- Verify actual remote desktop mode changes from the web UI `3840×2160 → 1920×1080 → 3840×2160` without reconnecting the desktop session or interrupting a running terminal. Independently inspect OS/display mode, not just video dimensions. Check `availableResolutions`, `desktop.resize` permission, active control lease, `display.resize` / `display.resize.result`, topology revisions and decoder `streamGeneration` changes. Exercise stale-input rejection during transitions and visible failure with the previous mode preserved/restored.

No deployment, port forwarding, real credential capture or changes to lab hosts/Keycloak. Hardware/privileged tests require separately established scope. Write original tests and tooling; no copied RustDesk code/assets. Document dependencies and licenses.

## Prerequisites and blockers

The contract baseline and component builds must be frozen before claiming interoperability. Physical display, approved test-host access, browser versions and measurement equipment/method are explicit prerequisites for real performance claims. If missing, prepare runnable harnesses and mark those checks unverified. Acceptance thresholds proposed in the brief remain proposed until recorded as agreed; never manufacture measurements to meet them.

## Acceptance

- A reproducible procedure and machine-readable results identify source SHAs, environment, command, workload and outcome.
- Functional/security failures have actionable reproductions assigned to the owning package.
- Performance report explicitly states whether 4K60 was achieved, for which path, with latency/fidelity limitations. Mocks and sample playback cannot pass physical end-to-end acceptance.
- Results distinguish pass, fail, blocked and not run; secrets and private screen content are excluded from artifacts.
- Actual active-session resolution changes are a hard M1 acceptance gate. Unsupported pilot capability is blocked/failed acceptance, never an optional skip or a passed scaling test. Evidence records session identity continuity, terminal continuity, actual modes, decoder reconfiguration and failure recovery.

## Handoff

Provide branch, commit SHA, all component/contract SHAs, commands/results, evidence locations, dependency/provenance notes and residual risks. Recommend accept or hold with concrete evidence; the integration owner decides M1 completion. No production changes are authorized.
