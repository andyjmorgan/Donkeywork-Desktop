# Contracts — draft 0.1.0

Reviewable baseline, not proven interoperability. contracts/ is integrator-owned. Changes need schemas, fixtures, tests and affected component review. Run npm test.

## Rules

All messages have protocol dwdesktop, exact version 0.1.0, UUID messageId and tagged payload. Media/terminal also bind sessionId and sessionEpoch. Unknown fields/types/versions are rejected. JSON counters are bounded to 2^53-1. Principal identity is issuer+subject, never email/hostname.

Local IPC: 4-byte unsigned big-endian JSON byte length followed by UTF-8 JSON, maximum 1 MiB. Validate size before allocation, UTF-8, schema, peer UID and socket permissions. Use a service-owned Unix socket, not unauthenticated loopback TCP.

messageId identifies the message. session.create requestId provides idempotency across retries for the same principal and identical parameters; conflicting reuse is invalid_argument. Control typed replies, terminal.opened, terminal.inputAck, error and control.ack carry payload.requestMessageId equal to the originating request's messageId; reply messageId is a new UUID. Every accepted request returns its typed result or ack; unparseable framing closes the connection. Control ack/error envelopes may be returned on a terminal channel for terminal requests without a dedicated reply. Media request-specific requestId fields retain their documented correlation semantics.

Schemas describe syntax; model.mjs exercises selected semantic invariants, not production authorization code. Fixtures are independent examples, not an ordered transcript.

## Authorization and lifecycle

Broker owns identity/policy. Worker owns OS resources and enforces granted operations. Watching, attaching and controlling are distinct. Browser-supplied principal/OS account fields are not trusted: internal session.create is constructed by the authenticated broker.

Create transitions internally starting -> ready -> closing -> closed (or failed). M1 initialization is bounded to 15 seconds: session.created is emitted only in ready state, meaning the session resource can accept an attachment, not that a decoder or PTY has already started. Initialization failure/timeout cleans up allocated resources and returns a correlated error (worker_unavailable for initialization failure/timeout); no externally visible starting response is permitted. Concurrent identical create retries join the same initialization/result. Close is idempotent. Preserve tombstones for at least the 120-second reconnect window. Restart that loses state invalidates sessionEpoch; never attach a new desktop under an old epoch.

Opaque 256-bit attachment grants are one-use, <=30 seconds, principal/session/epoch/permissions bound; persist hash only. Redeem through the authenticated broker, not a URL. Worker IPC receives trusted broker assertions. A direct media route requires separately reviewed equivalent authorization; portal login alone is insufficient.

M1 uses a separate long-lived broker control IPC connection and one dedicated Unix stream per attachment. The broker prepares the grant over control IPC, then opens a new stream whose first message must be attachment.redeem. Successful attachment.authorized permanently binds that stream to its attachmentId, principal, permissions, sessionId and sessionEpoch. Failed redemption closes the stream. Never multiplex attachments or accept another redeem on a bound stream. The broker binds its corresponding browser connection to this same attachment and forwards only its allowed session traffic. Validate session/epoch and required permission for every terminal/media operation; identifiers alone confer no authority. Broker-only prepare/renew/revoke/session lifecycle commands are forbidden on attachment streams. Control acquisition/release is forwarded by the broker over control IPC after checking the caller's attachment ownership. Media records use the framed adapter below; terminal records use length-prefixed JSON. Neither stream accepts messages for the other session kind.

Renew/revoke use the separate control IPC connection. An expired/revoked attachment closes its dedicated stream; reconnect opens a new stream and redeems a fresh grant. Terminal retention/resume below applies to the existing PTY, never to reuse of the old authorization. terminal.open requires terminal.open; input/resize/close require terminal.input; resume/output require terminal.open. A requested terminal profile must be allowed by the session's broker-selected OS account profile.

Renew attachment authorization every 15 seconds with worker-monotonic deadline <=45 seconds. Control lease cannot outlive attachment. Revocation closes and releases input; missing renewal fails closed at deadline. Broker renewal may also renew its bound control lease up to the same deadline; standalone renew/control identities cannot extend authorization. Competing acquire is atomic; explicit authorized takeover revokes prior control first.

No grants, credentials, clipboard contents, keystrokes or PTY bodies in diagnostic/audit logs. Closed sessions cannot be renewed.

## Framed media adapter

Each record: uint32 big-endian header JSON length, header envelope, then exactly payload.payloadBytes binary bytes when declared. Header <=64 KiB, frame <=16 MiB, cursor <=256 KiB. Nonbinary messages have no trailing data. Enforce limits before allocation. Do not base64 video on the hot path. Datagram/WebRTC media adapters need explicit fragmentation/reassembly/identity binding before implementation.

stream.configure declares exact browser codec/profile, bitstream format, decoder bytes, colour metadata and dimensions. Full-display capture in v0: sourceWidth/sourceHeight equal selected display's post-rotation raster; encoded width/height may differ. Browser scaling and encoder downsampling do not change host resolution.

Wait for stream.ready. Each decoder configuration change increments streamGeneration and starts with a keyframe. frameId increases within that generation. Frames must match current sessionEpoch, topologyRevision, streamId and streamGeneration. M1 forbids B-frame/reordering: decode and presentation order match.

captureTimeUs/presentationTimeUs are session-relative monotonic timestamps. Cross-host latency needs clock-offset measurement/external evidence. Never infer complete input-to-photon latency from these alone.

Bound queues by bytes/age. Obsolete decoded candidates may be replaced. On missing compressed reference frames or dependency-breaking discard, request a keyframe and suppress dependent frames until recovery. Feedback reports measured decode/present rates and queue state.

## Display, input and cursor

Origins are global host pixels. Input is display-local physical pixels in the post-rotation raster. Scale is informational. Validate against current dimensions in addition to schema bounds. Require desktop.control/current lease for input; order/deduplicate inputSequence per lease. Do not automatically replay uncertain clicks/text.

Key events use USB HID usage page 7; Unicode insertion is separate. Wheel units are explicit. Disconnect/timeout/revoke releases held input. Reset releases keys/buttons and remains possible during resize. Cursor hotspots must fit dimensions; RGBA payload length equals width*height*4. Respect cursorEmbedded to avoid double rendering.

Text clipboard is explicit and permission-controlled. read needs clipboard.read; write needs clipboard.write and current control. Browser permission failures are visible. Result text is null for a failed operation or write acknowledgment. No automatic background clipboard synchronization.

## Active resolution change — HARD M1 requirement

The UI must change the actual host display mode with an active desktop session: 3840x2160 ->1920x1080 ->3840x2160, without reopening desktop or disrupting terminal.

display.topology includes canResize and availableResolutions. display.resize binds requestId, leaseId, displayId, current topologyRevision and requested dimensions. Require desktop.resize plus current control. Validate supported mode/staleness before OS action; serialize transitions, release held input, suspend coordinate-bearing input.

Success: newer topology -> stream.configure with newer generation -> stream.ready -> keyframe. display.resize.result applied reports confirmed dimensions/revision. Session ID/epoch and PTYs stay unchanged.

Failure: preserve/restore prior mode and report rejected with actual resulting state and safe reason. If rollback fails, publish actual topology and reconfigure; never misreport old dimensions. Retry same requestId/parameters returns recorded result; conflicting parameters are rejected.

Browser fit, zoom and stream downsampling do not satisfy this requirement. Lack of pilot mode-switch support blocks M1 completion.

## Terminal

Use a real PTY under a policy-selected account/profile, with process-group cleanup. No arbitrary root-shell request. Output/input are raw bytes in canonical base64, <=64 KiB decoded per chunk. UTF-8 may span chunks.

Input is ordered/deduplicated for terminal lifetime. Never automatically resend unacknowledged input after ambiguous delivery. Resize sequence is independently increasing. Output sequence starts at 1; each chunk increments it.

Bound replay to 4 MiB and 120 seconds. Disconnect retains process at most 120 seconds; explicit revoke terminates immediately. Resume requires new authorization, replays strictly after lastOutputSequence and atomically switches to live output. Missing history emits terminal.gap first. Partial replay does not reconstruct full emulator state; display a discontinuity.

Exit follows final output with finalOutputSequence; exactly one of exitCode/signal is non-null. terminal.close is idempotent. Slow viewers cannot create unbounded buffers.

## Validation still required in implementations

Socket peers, TLS, cryptographic entropy, replay/deadlines, quotas, codec compatibility, decoder limits, same-session resize, rollback and actual 4K quality require runtime tests. Schema/model checks do not establish any of these by themselves.
