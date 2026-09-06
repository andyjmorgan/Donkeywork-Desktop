# WP04 — .NET broker, Keycloak and session grants

## Outcome and ownership

Implement the co-located single-host control plane in .NET. Own only `broker/`; branch `work/wp04-broker-auth`, separate worktree. WP01 owns Rust IPC and WP06 owns the UI; request root build wiring from the integration owner.

## Scope

- Propose the browser-facing HTTP/WebSocket API as a contract-only PR, reviewed by WP03/WP05/WP06, before implementing endpoints. The existing `contracts/` v0.1.0 draft defines worker wire messages, not yet an HTTP API. Cover local device, capabilities, session creation/closure and streaming-route coordination.
- Integrate Keycloak using the frozen authentication/session model. Validate issuer, audience, signature and expiry; enforce device/operation authorization server-side.
- Issue short-lived grants bound to user, device, session and permitted operations. Enforce replay/reuse policy and expiry exactly as agreed in contracts.
- Connect to local worker IPC; keep frame decoding and encoding outside .NET.
- Apply appropriate origin/CSRF protections for the chosen browser auth model, sanitize error responses and redact credentials/grants from logging.
- Provide a local development identity-provider fixture or documented isolated test realm configuration. Never change the existing lab realm implicitly.
- Authorize actual desktop resolution changes with the explicit `desktop.resize` permission and active control lease. Preserve contract semantics for `display.resize` / `display.resize.result`, `availableResolutions`, topology revisions and decoder `streamGeneration` updates. A mode change must preserve the desktop session identity and terminal lifetime; do not implement it as a disconnect/reconnect. Surface resize failures with the prior mode preserved/restored by the worker.

No device enrollment/fleet rollout, production endpoints, MCP implementation or existing Keycloak changes. Original implementation only: no RustDesk source, assets or translations. Document dependencies and licenses.

## Prerequisites and blockers

Freeze broker API, Keycloak auth flow, cookie/token handling, opaque grant issuance/validation owner, grant replay rules, local IPC and browser transport before merge. The baseline uses opaque one-use grants, not signed browser tokens. Missing decisions require a contract PR. Mock IPC and local test identities permit early tests; actual sessions require WP01. Production secrets/access are not prerequisites for local implementation.

## Acceptance

- Tests cover expired/wrong-audience/wrong-issuer credentials, unauthorized operations, tampered and wrong-session grants, replay policy, worker failure and session cleanup.
- Verify unauthenticated HTTP and streaming upgrade attempts cannot create usable sessions.
- Verify logs and returned errors do not expose credentials, grants or terminal payloads.
- Provide an explicit development configuration and validated configuration-failure behaviour.
- Test unauthorized resize, missing/expired control lease, stale topology input and resize failure; verify the integrated `3840×2160 → 1920×1080 → 3840×2160` sequence needs no desktop reauthorization/reconnect and leaves the terminal running. Unsupported pilot live resize is an M1 blocker.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, commands/results, auth flow exercised, test identity-provider details, dependency/provenance notes and limitations. State whether Keycloak integration used a real isolated instance or mocks. Include integration instructions for WP01/WP03/WP05/WP06 and no production changes.
