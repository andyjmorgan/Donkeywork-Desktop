# WP07 — M1a acceptance, then M1b streaming acceptance

## Outcome and ownership

Own `tests/integration/`, `tests/performance/`, `tests/security/` and `docs/validation/`; branch `work/wp07-integration-acceptance`, separate worktree. Component owners fix their paths; contracts/root CI belong to the integrator. Prepare independent evidence for distinct M1a and M1b gates.

## M1a scope and hard acceptance

Use the frozen `contracts/local-cli.md` v0.2.0 profile with WP01/WP02/WP05/WP09. No broker, Keycloak, browser, remote OAuth or video prerequisite.

- Verify OS peer UID allowlist, socket ownership/permissions, fixed permissions/account profile and rejection of client identity spoofing/raw broker commands. Check denial before backend side effects.
- A `desktop.view` principal can inspect topology/take screenshots without a control lease, but cannot acquire unauthorized control. Input/resize require current lease and correct topology; expiry/revoke releases held input and interrupts further actions.
- Decode a real `3840×2160` PNG and verify exact native dimensions, capture metadata, topology, cursor policy and bounded binary framing. Test oversize/undeclared bytes and malformed payloads; never accept truncation or downsampling as success.
- Perform screenshot → click/type known target → screenshot confirming action. Validate extreme corner coordinates at 4K and 1080p, stale pre-resize topology, out-of-bounds coordinates and no automatic replay of ambiguous actions.
- Independently verify actual X11/RandR modes `3840×2160 → 1920×1080 → 3840×2160` from CLI while desktop session/epoch and a running PTY survive. Record actual modes, terminal process continuity and failure/rollback reporting. Unsupported pilot resize blocks M1a.
- Exercise PTY bytes, resize, cleanup, slow-consumer limits and logs without screenshot/input/PTY/credential bodies.

The pilot requires X11/RandR and advertised 4K/1080p modes; Spark remains unverified until evidence exists. This assignment authorizes no live display changes or installation. Missing approved host access means blocked physical acceptance, not a passed fixture substitute.

## M1b deferred scope

Add WP03/WP04/WP06 and video/browser halves of WP02/WP05. Verify Keycloak, broker grants, origin/CSRF/upgrade boundaries, browser input, live resolution selector, decoder generation reconfiguration and terminal continuity.

Measure 4K motion/text, codec/chroma, frame rate, bounded queue behavior, bandwidth, input latency methodology and CPU/GPU load on LAN and isolated impaired-network scenarios. Compare screenshot versus decoded video from the same capture source/frame within codec tolerance. 4K60 is a measured target on a declared capable baseline, not an M1a still-image gate or universal hardware claim.

## Prerequisites, evidence and handoff

Freeze exact contract/component SHAs per run. Missing API/interface decisions need contract amendments; no invented endpoints. Results identify command, workload, OS/display/browser as relevant, artifact and pass/fail/blocked/not-run status. Synthetic fixtures cannot establish physical capture, security or end-to-end performance.

Original tests only; no RustDesk code/assets/adaptations. No deployment, port forwards, credential capture, production/Keycloak changes or host modifications without separately scoped authorization. Provide branch, base/commit/component/contract SHAs, commands/results, safe artifacts, dependency provenance, limitations and accept/hold recommendation per milestone. The integrator decides completion.
