# Internal fleet manager — agreed design, 2026-09-10

This decision supersedes WP04's .NET/Keycloak direction for this milestone.
Go backend, PostgreSQL persistence, existing DonkeyWork React theme. Manager
origin: https://desktops.donkeywork.dev, internal DNS and internal access only.
No Keycloak, public ingress, TURN deployment or Internet connectivity tests.
The existing Python single-host broker remains a pilot, not the fleet manager.

## Identity and trust

The operator UI has no login. Anyone able to access it is an administrator and
can enroll/control devices. Restrict access at the network layer; TLS does not
provide operator authorization. Do not publish this service or its enrollment
endpoint through Cloudflare. Enforce exact Host/Origin and bounded JSON requests
to reduce browser cross-origin abuse; this does not replace authentication.

Device identity is separate: device generates a private key, manager signs its
CSR using a dedicated device CA. Server HTTPS trust must exist before enrollment
(system trust or an explicitly supplied CA file; never insecure skip-verify).
Device CA keys belong in protected persistent storage, not PostgreSQL or source.
Device keys stay on-device in root/service-private files. CA rotation and expiry
must not silently enroll a replacement identity.

## Operator enrollment

Create device with name and description. Return opaque device ID and one
cryptographically random eight-character code, displayed XXXX-XXXX, using a
32-character unambiguous alphabet (40 bits). Code expires after 24 hours.
Show plaintext only at issuance; store only a keyed HMAC digest with a separate
persistent server secret. Do not log request bodies, codes or credentials.

Installer prompts for HTTPS manager URL and code (hidden entry). It generates
the key locally and sends a bounded signed CSR and code in a POST body, never
query strings/argv. Validate CSR signature and key strength; ignore requested
identity/extensions and assign certificate identity from the database record.

Claim atomically consumes an unexpired, unclaimed, unrevoked code, binds the
device to the public key, and persists certificate metadata in one transaction.
Exactly one concurrent claimant wins. Once claimed, the code is DEAD, including
same-key retries. If the response is lost after commit, operator must revoke
that enrollment and issue a fresh code; do not weaken single-use semantics to
hide the failure. Installer should retain its key for diagnosis/recovery.

Invalid, expired, claimed and revoked codes get the same public error. Bound
attempts globally and per direct peer address before database lookup; do not
trust forwarded IP headers. Regenerate invalidates the old code transactionally.
Revocation blocks new device requests and terminates existing control connections.

## Device connection and sessions

Separate device listener requires verified client certificates. Bind certificate
fingerprint/serial to the current enrolled database record, not merely a trusted
CA or a client-supplied ID. Outbound persistent connection carries versioned
heartbeats, capabilities, desktops[] and correlated commands. No video in this
channel. Reconnect uses bounded exponential backoff with jitter.

Heartbeats every 15 seconds; offline after 45 seconds. Persist last-seen, but
online status requires a live connection owned by this manager process; after
restart all devices are offline until reconnect. A connection epoch prevents
an old disconnect from marking a replacement connection offline. Start with
one manager replica; shared persistence does not imply multi-replica routing.

On-device service owns its certificate and policy. In-session agents register
over a permission-controlled Unix socket; validate peer UID/PID and session
membership. They never receive the device private key. Session identifiers bind
to a lifetime/epoch, not a reusable PID or display number. Keep capabilities
explicit: managed X11 desktops and existing-session Wayland capture are separate
adapters. No promise that Rocky currently supports managed desktop creation.

## Implemented gateway adjustment — 2026-09-10

The first integrated manager uses forced relay, rather than requiring direct
browser-to-device connectivity. Browser H.264/WebRTC and ordered input terminate
at the Go manager; existing compressed RTP and input records are forwarded over
the device's outbound mTLS WebSocket, multiplexed alongside bounded commands.
No video transcoding. This supersedes the earlier "No video in this channel"
and direct-first proposal. See [the concrete contract](../../contracts/manager-session-bridge.md).

The local adapter connects through a restricted Unix socket to the established
configured-user managed broker. This is not an arbitrary-user PAM login broker
or the future Wayland in-session app. Existing desktop workers stay intact.
Browser access needs manager HTTPS and UDP30445; no public network/TURN work.

## Desktop connection and future gateway — original proposal

Manager brokers signaling and short-lived connection authorization. Browser and
device exchange video and ordered input using existing WebRTC directly on LAN.
Bind signaling/DTLS identity and one-use attachment grants to device, session,
epoch, connection and permitted operations. Operator principal is the explicitly
unauthenticated internal pilot, not a fabricated Keycloak user.

Gateway is a future transport alternative: direct / relay / forced-relay.
TURN will carry encrypted WebRTC media/input, manager issues limited relay
credentials. Do not implement public networking now. No assumption that an HTTP
tunnel provides TURN or that manager availability proves media reachability.

## Delivery slices and acceptance

1. PostgreSQL schema, Go create/list/enroll API, local CA integration; tests for
   24-hour expiry, concurrent claim, invalid CSR, replay, safe failure responses.
2. Themed manager UI, regenerate/revoke, interactive installer and certificate
   renewal; real verified TLS/mTLS end-to-end tests, untrusted/revoked rejection.
3. Persistent outbound device connection and live online/offline inventory;
   restart/reconnect/revocation and backpressure tests.
4. Attach existing managed-session adapter, then implement Wayland session agent.
   Prove visible capture/input, reconnect, permissions and capability failures.

Local development is isolated from existing running pilots. No lab deployment,
DNS changes or CA trust installation is implied by a successful unit test.
