# Local physical-console viewer

This is the authoritative Go bridge source in the main repository, consolidated
from the live `desktop-console-video/console-web` tree on 2026-09-06, including
its uncommitted input/recovery integration. Runtime source and dependency
lockfiles are unchanged by consolidation. No sibling worktree is needed.
The live frontend now builds from [`../console-ui`](../console-ui/README.md);
the root repository's older `web/` remains a legacy demo.

Optional seamless capture recovery: `--capture-broker-fd 4` uses a separately
inherited Unix socket to request replacement capture connections via SCM_RIGHTS.
The broker receives byte`C`, replies byte`F` plus exactly one connected FD, or
byte`E` while unavailable. No filesystem socket access or new HTTP API is added.
Capture loss pauses input and releases its owner, flushes old media queues and
parser state, and reports `recovering` while retaining HTTP, PeerConnections,
tracks and the browser's last image. Only an identical source/codec/geometry
header is accepted; the first complete new keyframe resumes media/input.
Requests retry every250ms. Without this optional broker, original EOF shutdown
behavior remains unchanged. Root capture supervision and browser stall handling
are separate integration components; these local tests are not live handoff proof.

Optional input pilot: `--input-fd 3` accepts a separately inherited connected
`dwconsole.input`0.2.0 socket; input is disabled by default. The existing
root-dropping launcher supplies this connection. One reliable ordered WebRTC
data channel named `dwconsole.input` carries all keyboard/pointer events.
One viewer can acquire control at a time; no automatic takeover. Browser
release waits for the helper release ACK before the bridge returns `released`.
Only explicit browser renewals extend the one-second helper lease.

The input executor bounds JSON to4096bytes, queued requests to32 and request
age/helper response time to500ms. Invalid generation/sequence/coordinates,
channel closure, queue overflow, capture failure and helper errors revoke
control. Helper expiry is forwarded asynchronously; reacquire creates a fresh
generation. Input content is never logged. Source dimensions must match helper
dimensions before acquisition succeeds. This does not add authentication,
clipboard, relative mouse or arbitrary live-resolution support. Wheel input is
supported as bounded vertical/horizontal ticks. See the
integrator's `contracts/console-input-web.md` for the pilot contract.

Experimental `dwconsole.stream` 0.1.0 adapter approved by the integrator for the Spark display-only prototype. Go/Pion receives an already-connected, inherited Unix socket, converts framed Annex B H.264 into WebRTC samples, and serves the React viewer on the same origin. The network process refuses root. It does not connect to privileged device sockets itself.

Build locally (Go 1.24+, Node 22+):

```sh
cd console-web
go test -race ./...
go vet ./...
CGO_ENABLED=0 GOOS=linux GOARCH=arm64 go build -o console-web-arm64 .
cd ../console-ui
npm ci
npm run build
```

The integrator's privileged launcher connects to `/run/dwconsole/stream.sock`, installs the connected socket as stdin, clears supplementary groups, drops UID/GID and execs:

```sh
/opt/donkeywork-desktop/bin/console-web \
  --stream-fd 0 \
  --listen 192.168.69.28:8090 \
  --ice-listen 192.168.69.28:8091 \
  --origin http://192.168.69.28:8090 \
  --assets /opt/donkeywork-desktop/web
```

Install all files from `console-ui/dist/` at the assets path. Open `http://192.168.69.28:8090`; video connects automatically. LAN clients need TCP8090 and UDP8091 to Spark. Origin and Host must match the configured browser URL; no cross-origin requests or wildcard CORS. No STUN/TURN, public service, authentication or enrollment. Input is available only when its optional inherited descriptor is configured. Use only the expressly authorized internal lab interface. Root capture/encoding remains a separate prototype limitation. Build Node/npm dependencies are not needed on the deployed host.

Adapter API: GET `/api/status` reports `streaming`, `recovering` or `failed`, with error and original source metadata where available. The UI declares live only when browser decoded-frame counters advance; retained last frames are not evidence of live capture. POST `/api/offer` accepts a WebRTC `{type:"offer",sdp}` and returns a gathered `{type:"answer",sdp}`. The browser gathers candidates first. With the recovery descriptor, source EOF preserves existing peers and requests a fresh capture connection as described above. Without it, EOF shuts down the bridge and requires a fresh inherited source socket. The source socket's inactivity timeout is 10 seconds, startup included.

Media records are arbitrary bytes, not frames. The bounded parser reassembles split start codes, groups progressive AVC slices using `first_mb_in_slice`, and prepends cached SPS/PPS at IDR. New viewers wait for the encoder's next periodic IDR. No on-demand keyframe control exists yet. This adapter is for the current no-B-frame libx264 output, not a universal H.264 parser; partitioned/SVC/MVC slices are rejected. Profile/level negotiation and high-profile/hardware encoders require further work; the initial SDP advertises constrained baseline with level asymmetry, so actual browser decode must be verified against the source SPS. There is no claim of 4K or 4K60 acceptance.

Each of at most two viewers has three queued access units, each at most 16MiB. Overflow closes the slow viewer rather than dropping reference pictures while reporting success. HTTP/signaling bodies and timeouts are bounded. Pion supplies RTP packetization, SRTP/DTLS and default RTCP/NACK handling. PLI is consumed but can only recover at the next periodic IDR. Stop/disconnect releases the peer and queue. No raw video or screenshots are retained by the bridge.

The browser uses native `<video>` with `object-fit: contain`, preserving aspect ratio at smaller viewports. CSS fitting and fullscreen do not change physical display mode. Absolute pointer and physical keyboard handling use the separate input contract; arbitrary live host resolution changes remain outstanding. Go tests cover stream parsing, input ownership/validation and capture recovery. Frontend tests cover geometry, input control, recovery decisions and the retained demo model. These tests are not live-host evidence.

Dependencies: original integration code, Pion WebRTC and its Go dependencies (see go.mod/go.sum; upstream licenses retained through normal distribution obligations). No RustDesk or JetKVM application source or UI is copied. The frontend retains the existing DonkeyWork theme and asset provenance described in [console-ui/README.md](../console-ui/README.md).
