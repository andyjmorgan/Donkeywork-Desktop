# Device connection — internal pilot v1

Session/media extension: [manager-session-bridge.md](manager-session-bridge.md)
supersedes the heartbeat-only record limit and no-commands statement below.
Heartbeat semantics remain; binary traffic does not renew presence. Only valid
heartbeats renew the 45-second liveness window.

Enrollment response now includes deviceEndpoint, a wss URL for /connect on the
separate mTLS listener. Browser/operator endpoint does not ask for a device cert.
Server TLS uses verified manager trust. Device endpoint requires a valid client
certificate from the device CA plus an existing claimed, non-revoked database
record matching the certificate SHA256 fingerprint and assigned device URI.
Deleted/unknown identities fail closed even with a CA-signed certificate.

Client initiates WebSocket over TLS1.3. No browser Origin allowed. First and
subsequent records, every 15s: `{version:1,type:"heartbeat",hostname,platform}`.
Hostname max253 characters, platform max80, JSON text record max4096 bytes.
Server responds `{version:1,type:"ack",deviceId}`. Client checks deviceId.
No command execution or desktop operations are supported by this first version.

No valid record for 45s closes the connection; EOF is immediately offline.
Heartbeats faster than 1s are rejected. Acknowledgments require a successful
database last_seen_at update against the still-authorized identity. Certificate
expiry is rechecked on records; an open socket does not extend certificate life.
Revocation closes an active connection immediately. A new connection replaces
an old one; old cleanup cannot remove the new connection's online status.

Online is process-local live presence, not persisted last-seen. Manager restart
therefore reports offline until the client reconnects and sends a valid record.
GET devices includes online and live hostname/platform. No desktop inventory
or false session capabilities are advertised. Single manager replica only.
