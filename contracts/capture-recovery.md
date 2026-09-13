# Persistent viewer / recoverable capture — pilot

The HTTP listener, PeerConnection, RTP track and input service survive session
changes. Supervisor restarts only the per-session output helper and target
guard. Guard revocation still clears held input and old generations.

## Narrow privileged reconnect operation

Root `socket-broker` listens on root-private0600 Unix socket in root0700 runtime
directory. It accepts root peers only. Launcher connects before dropping UID,
then inherits that connection into Go as fd4 (`--capture-broker-fd 4`). This is
local descriptor passing, not the future fleet/auth broker.

- Request exactly byte `C`.
- Success exactly byte `F` with exactly one SCM_RIGHTS connected capture socket.
- Temporarily unavailable: byte `E`, no descriptors.
- Unknown commands close the connection. No paths, shell commands or arbitrary
  opens are accepted. Capture path is fixed in trusted service arguments; root
  ownership, socket permissions and peer credentials are checked before passing.

Transferred capture socket retains `dwconsole.stream`0.1.0 framing. Go closes
old feed, requests a replacement, validates its header and creates fresh AnnexB
and access-unit parsers. Root closes its copy after transferring the descriptor.

## Recovery behaviour

Capture EOF/timeouts put `/api/status` into `recovering`, not process shutdown.
Drop queued old access units, suspend input, reset per-viewer keyframe readiness.
Resume only with a complete new keyframe and matching validated header. Keep
the SAME track/PeerConnection; do not reset RTP sequence by replacing the track.

This first pilot keeps the established1080p configuration. Transient1024x768
from a newly starting compositor is rejected until supervisor restores1080p.
Arbitrary live resolution negotiation is separate outstanding work—not hidden
behind CSS scaling. Malformed/unavailable feeds stay visibly waiting.

Browser retains its last frame during source recovery, releases input, and does
not close a healthy PeerConnection merely because decoded frames pause. Real
transport failures retain normal automatic reconnection. New frames restore
live state without replaying prior input or silently taking control.

## Acceptance

Reproduce a capture interruption while an existing browser remains open; assert
unchanged bridgePID, same PeerConnection, no additional offer, and increasing
decoded frames after recovery. Exercise a gap longer than the old five-second
UI stall threshold. Supervisor guard reset must not restart web/input processes.
No GDM restart/logout or user-app manipulation is required for these fault tests.
Actual subsequent user login/logout remains a distinct integration test.
