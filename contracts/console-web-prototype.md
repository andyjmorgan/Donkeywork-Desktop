# Physical console browser prototype

Experimental, local single-host display adapter agreed with the video/UI agent
on 2026-09-06. This does not amend the managed-session or fleet protocols.

The unprivileged Go bridge inherits an already-connected `dwconsole.stream`
0.1.0 Unix stream on standard input (`--stream-fd 0`). The launcher connects as
root, verifies the socket peer, clears supplementary groups, drops UID/GID and
sets no-new-privileges before executing the bridge. HTTP and WebRTC never run
as root. Capture and software encoding still share the privileged POC process;
the full capture/export privilege split remains outstanding.

The stream starts with a uint32 big-endian JSON length and the exact
`StreamHeader` in `device/console/src/lib.rs`, bounded to 64 KiB. Subsequent
records are uint32 big-endian byte length plus H.264 Annex B bytes, bounded to
1 MiB. Record boundaries are arbitrary, not NAL or access-unit boundaries.
Current source has no B frames. EOF, malformed framing or unsupported source
metadata must fail visibly. Source resolution changes currently require source
restart; browser scaling does not change the physical mode.

Same-origin `POST /api/offer` exchanges `{type: "offer", sdp}` for
`{type: "answer", sdp}` with complete (non-trickle) ICE. `GET /api/status`
reports source state and metadata. Media uses H.264 over WebRTC to a native
video element. The prototype has no input, clipboard, authentication or device
management endpoints. It binds explicitly to Spark's private address; no public
tunnel or router changes are part of this experiment.

Keyboard/mouse transport and clipboard are under separate source-backed design
review. The display prototype does not depend on those decisions.

Current minigpu extensions: `console-input-web.md` documents input; optional
`capture-recovery.md` documents same-peer source recovery. With that capability,
capture EOF is recovering rather than HTTP/WebRTC teardown. The original
display-only mode remains supported for other existing deployments.
