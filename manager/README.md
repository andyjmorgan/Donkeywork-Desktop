# Internal manager — first implementation slice

[Agreed design](../docs/milestones/M3-internal-manager.md).
Go + PostgreSQL. No Keycloak. No public exposure. Default bind is loopback.

Implemented: HTTPS JSON API for device creation/listing and single-use CSR
enrollment, PostgreSQL persistence, HMAC-protected code digests, manager-assigned
client certificate identity, 24-hour expiry, bounded request bodies, origin
checks and global/per-peer enrollment attempt limits. Thirty-day certificates.

Now deployed on attic: themed React registration UI, revoke/regenerate endpoints,
and persistent PostgreSQL. Preview: https://192.168.10.11:30443 (private preview CA).
See [deployment details](../deploy/manager/README.md).

Installer and certificate-authenticated outbound WebSocket connectivity are now
wired, including live online/offline status and revocation. See
[device packaging](../packaging/agent/README.md). Installed and verified on
Easternkingdoms. The existing managed broker now exposes a restricted Unix socket
to the agent; existing desktop processes are preserved.

Session inventory, environment selection, create/reconnect/end, resize and
H.264/input gateway relay are implemented for configured managed-session pilots.
See [bridge contract](../contracts/manager-session-bridge.md). The agent uses its
existing outbound mTLS connection for both commands and relayed media/input.
Browser WebRTC terminates at the manager; no gateway decode/re-encode.

Minigpu additionally supports an existing Wayland console through the
[in-session portal adapter](desktopportal/README.md), alongside managed desktops.
This is a current GNOME46 session pilot, not universal unattended Wayland support.

Not implemented yet: certificate renewal, arbitrary-user authentication/PAM
launch, or automatic desktop-backend installation.
Validate other host installs individually before a fleet rollout.

## Run

Requires PostgreSQL and separately provisioned TLS server certificate/key,
device-signing CA certificate/key, and a persistent random enrollment HMAC secret
file (at least 32 bytes). Protect all private files; never commit them. The
server certificate must match the configured origin and be trusted by clients.
Do not disable certificate verification. Configure DATABASE_URL via the process
environment, using deployment-secret tooling rather than command-line secrets.

```sh
go run ./cmd/manager \
  --listen 127.0.0.1:8443 \
  --origin https://desktops.donkeywork.dev \
  --tls-cert /secure/server.crt --tls-key /secure/server.key \
  --device-ca-cert /secure/device-ca.crt --device-ca-key /secure/device-ca.key \
  --enrollment-secret-file /secure/enrollment-secret
```

Server requires TLS 1.3. Reverse proxy deployment is not implemented: it must
not silently strip TLS or replace the trusted peer address. Single replica only.
Startup creates the initial schema; later schema evolution needs migrations.

`POST /api/v1/devices` requires matching Origin and application/json:
`{"name":"Minigpu","description":"Office Ubuntu host"}`.
Returns device metadata and plaintext code once. `GET /api/v1/devices` returns
`devices[]` without codes/digests/certificates. `POST /api/v1/enroll` accepts
`{"code":"XXXX-XXXX","csrPEM":"..."}` and returns deviceId, certificatePEM,
deviceCAPEM. Installer requests need not include Origin. See contract for limits.

## Tests

`go test -race ./...` runs unit checks. To also test PostgreSQL, set
MANAGER_TEST_DATABASE_URL to a disposable database. Integration creates and drops
only its uniquely named schema. It proves a 16-way claim race has exactly one
winner, rejects replay/expiry/revocation/invalid CSR, and checks signed identity.
No integration result is claimed when the PostgreSQL test is skipped.
