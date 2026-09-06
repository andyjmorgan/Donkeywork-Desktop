# WP04 — M1b .NET broker, Keycloak and session grants (deferred)

This is M1b work, not an M1a dependency or initial dispatch lane. M1a authenticates its local CLI using verified Unix peer UID, an allowlist and fixed permissions/account profile on a dedicated local API; it requires no Keycloak, broker HTTP API, remote OAuth or browser grants. Do not expose the raw privileged broker socket through the CLI path. Add the reviewed broker trust boundary separately when M1b starts.

## Outcome and ownership

Implement the co-located single-host control plane in .NET. Own only `broker/`; branch `work/wp04-broker-auth`, separate worktree. WP01 owns Rust IPC and WP06 owns the UI; request root build wiring from the integration owner.

## Scope

- Propose the browser-facing HTTP/WebSocket API as a contract-only PR, reviewed by WP01/WP03/WP05/WP06, before implementing endpoints. The v0.2.0 local CLI contract is not an HTTP API; the earlier v0.1.0 browser messages are design inputs, not proof of a ready API. Cover local device, capabilities, session creation/closure and streaming-route coordination.
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
- Test unauthorized resize, missing/expired control lease, stale topology input and resize failure; verify the integrated `3840×2160 → 1920×1080 → 3840×2160` sequence needs no desktop reauthorization/reconnect and leaves the terminal running. Unsupported pilot live resize is an M1b blocker. M1a proves the same device operation locally first.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, commands/results, auth flow exercised, test identity-provider details, dependency/provenance notes and limitations. State whether Keycloak integration used a real isolated instance or mocks. Include integration instructions for WP01/WP03/WP05/WP06 and no production changes.
