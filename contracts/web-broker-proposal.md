# Browser broker boundary — proposal, not an implemented API

2026-09-06. Security-reviewed outline for the first live single-device console. No schema freeze or interoperability claim; the local UI demo must not depend on these endpoint names yet. Before implementation, add exact DTO schemas, fixtures, limits and transport state-machine tests under integrator review.

## Identity

Same-origin .NET 10 BFF, Keycloak authorization code + PKCE, state and nonce. Validate issuer, signature, audience, expiry and nonce with maintained OIDC middleware. Keep provider tokens server-side. Use a Secure, HttpOnly, SameSite=Lax, Path=/ cookie with a `__Host-` prefix and no Domain. Never use browser storage for auth tokens. Configure allowed issuer+subject/device/operation mappings; successful Keycloak login alone grants no desktop access.

Mutating HTTP requests require a session-bound CSRF token; WebSocket upgrades require an exact allowed Origin and valid session before contacting the worker. Default to same-origin with no broad CORS. Return paths must be local and allowlisted. Private API responses use `Cache-Control: no-store`.

## Candidate routes

| Route | Required behavior |
|---|---|
| GET /auth/login | Start bound OIDC flow, no arbitrary redirect destination |
| GET /auth/callback | Validate flow and establish server-side session |
| POST /auth/logout | CSRF check; revoke local attachments/control before provider logout |
| GET /api/me | Identity, effective permissions and CSRF token; no provider tokens |
| GET /api/devices | One configured device; observed capabilities/status, never mocked live health |
| POST /api/sessions | Authorized device/kind; server selects OS profile; bounded idempotency |
| DELETE /api/sessions/{id} | Owner/authorized principal; idempotent cleanup |
| POST /api/sessions/{id}/attachments | One-use grant, <=30 seconds, bound to principal/device/session/epoch/operations |
| GET /api/sessions/{id}/connect | Authorized WebSocket upgrade; redeem grant in first bounded message within five seconds |
| POST /api/sessions/{id}/control | Atomic acquire, no implicit takeover |
| DELETE /api/sessions/{id}/control/{leaseId} | Release caller's lease and held input |

Grant material stays out of URLs, logs and WebSocket subprotocol values. Worker stores only its hash. Sessions/leases are identifiers, not bearer authority. Attachment renewal uses the existing 15-second / <=45-second fail-closed defaults. Logout and local revocation terminate active attachments immediately. Provider-side revocation requires a concrete backchannel-logout or other verified design; offline JWT verification cannot promise immediate Keycloak logout detection.

## Explicit blockers before live connection

- The implemented worker's local socket authenticates Unix UIDs, not Keycloak principals. Implement the separate trusted worker attachment boundary; do not expose generic worker-command HTTP routes or quietly equate the broker UID with user authorization.
- Freeze the first media transport, handshake, allowed message directions, size/queue limits and close/error semantics. PNG stills can be a separately labelled bootstrap, but are not the agreed video implementation or 4K performance proof.
- Define principal/session ownership, lease connection ownership, revocation and disconnect cleanup across HTTP and streaming connections. Do not reconnect a desktop to implement a mode change.

Use stable sanitized errors: 401 unauthenticated, 403 denied, 404 non-visible resource, 409 conflict/stale topology, 413 size limit, 429 quota, 503 worker unavailable. Do not log tokens, grants, screenshot pixels, input text, clipboard or terminal content.

Required negative tests include cross-user references, forged device/profile, wrong Origin/CSRF, expired/reused grants, logged-out sockets, lease expiry, stale topology and worker disconnect without input side effects. Passing this proposal review is not passing those tests.
