# Internal manager HTTP API — first slice

Supersedes earlier Keycloak/.NET requirements only for the internal pilot.
See docs/milestones/M3-internal-manager.md for agreed architecture and boundaries.
Not the existing local CLI or Python managed-session HTTP protocol.

- Verified TLS, exact configured Host. Supplied Origin must match exactly.
- Operator POSTs require Origin. No human authentication: LAN callers have
  administrator authority. Enrollment clients authenticate through the code.
- POST bodies: application/json, max 24 KiB, one JSON object, unknown fields
  rejected. CSR max 16 KiB, signed P-256-or-stronger ECDSA or RSA >=3072 bits.
- GET /api/v1/devices -> 200 `{devices: Device[]}`.
- POST /api/v1/devices `{name, description}` -> 201 `{device: Device, code}`.
  Name is trimmed, nonempty, max 100 bytes; description max 2000 bytes.
- Device: id UUID, name, description, createdAt, codeExpiresAt, claimedAt
  nullable, revokedAt nullable. No online status is fabricated in this slice.
- POST /api/v1/enroll `{code, csrPEM}` -> 201
  `{deviceId, certificatePEM, deviceCAPEM}`. Code is case-insensitive; the one
  middle hyphen is optional. Only the approved 32-character alphabet is accepted.
- Expired, revoked, consumed and unknown codes all return 403 enrollment rejected.
  Certificate issue/commit must complete atomically before 201. Claim code digest
  is cleared on commit. Requested CSR subject/SAN/extensions are not copied.
- 24-hour expiry uses PostgreSQL time and is rechecked during claim update.
  Concurrent claims serialize on the database row. Exactly one wins.
- Enrollment limited to 5 attempts per direct source IP per minute, 60 global.
  429 Retry-After:60. Limits are process-local and reset on restart; distributed
  rate limiting is required before multi-replica deployment. No X-Forwarded-For.
- 400 malformed request, 403 origin/enrollment denied, 404 unknown route,
  503 service failure. Responses never echo supplied credentials or database errors.

Additional operator routes (same Origin/body rules):
- POST /api/v1/devices/<uuid>/delete `{}` permanently removes an already revoked
  record, including its enrollment/certificate metadata; returns `{state:"deleted"}`.
  Unrevoked or unknown devices return 409. UI requires explicit confirmation.
  No device uninstall occurs. Future certificate authentication MUST require an
  existing non-revoked database identity; CA signature alone is never sufficient.
- POST /api/v1/devices/<uuid>/code `{}` replaces a pending device's code and
  returns `{device,code}`. Old code immediately dies; expiry restarts at 24h.
  Claimed or revoked devices cannot regenerate (409).
- POST /api/v1/devices/<uuid>/revoke `{}` atomically revokes and clears any
  pending code. Idempotent for existing devices. Returns `{state:"revoked"}`.

Executable conformance and transactional cases: manager/http_test.go and
manager/enrollment_test.go. Browser proof: tests/integration/manager-registration.mjs.
Renewal and device mTLS
protocol contracts follow in subsequent slices, not implicit success stubs.
