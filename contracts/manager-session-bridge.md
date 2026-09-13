# Internal manager session bridge — pilot v1

The enrolled device connection carries correlated, bounded JSON RPC alongside
heartbeats. `request` records contain version=1, id (random UUID), method, path,
and optional JSON body. `response` records echo id and contain status and JSON
body. Both endpoints allow only the managed-broker inventory/create/status/
reconnect/close/resize/offer routes. No arbitrary URL, shell, file or header proxy.
Maximum record 512 KiB; eight concurrent requests per device; 12-second local
HTTP timeout and 14-second manager timeout. Ambiguous mutations are NOT replayed.
Create retains the existing broker requestId idempotency contract.

Manager routes are `/api/v1/devices/<device-id>/broker/<route>` with route
`desktops`, `environments`, or `desktops/<id>/<action>`. POST requires exact
manager Origin. Devices must be live and non-revoked. Replacement, revocation
and disconnect terminate pending requests. Responses bind to the exact device
connection and request; unknown IDs cannot satisfy another device's request.

The optional device adapter connects only to an operator-configured local Unix
socket, never a browser-provided address. Socket directory is operator-owned;
socket access is limited to the broker user and device-agent group. The broker
continues running as the existing pilot user, not root. This stage does not add
arbitrary account selection or privilege escalation.

## Media relay

Browser offer is terminated by the Go manager (WebRTC UDP 30445). A correlated
`media.open` request instructs the device to connect a local WebRTC receiver to
the selected broker session. It does not receive the browser's SDP or arbitrary
network address. The local session bridge still owns capture/encoding and input.
H.264 RTP passes unchanged through the agent's existing outbound mTLS WebSocket
to a manager RTP track. No decode or re-encode in the gateway.

Binary records contain 32 ASCII hex stream-ID bytes, one kind byte, and payload:
1=RTP video (device→manager), 2=UTF-8 input protocol (both directions, 4096-byte
maximum), 3=keyframe feedback (manager→device), 4=close (both directions).
Streams are bound to the exact authenticated device connection; at most four
per device. No media-only second device connection. A bounded 256-packet video
queue closes an overloaded viewer instead of dropping compressed dependencies
or retaining unbounded stale video. Input data-channel queues are capped at
16KiB. Existing input lease, sequence, focus-release and scaling rules remain.

Viewer disconnect closes its local media peer, releasing input but retaining
the desktop. Device disconnect/replacement/revocation closes every related
viewer. No automatic replay of input or create operations. Resize and logout
continue to use the existing stream-recovery/session-state paths.

The browser needs HTTPS 30443 and UDP 30445 to the manager. The device needs
outbound WSS 30444 and local session media connectivity only; browser-to-device
reachability is not used by this relay. No Internet/TURN/firewall-fallback claim.

## Existing Wayland console adapter (Minigpu pilot)

An optional operator-configured, same-user private Unix socket adds a live
`kind: "console"` inventory record. Its random ID is scoped to the running portal
agent, not a managed desktop. Only a successfully opened portal session advertises
ready. Reconnect/status/offer use the existing routes and gateway; close/resize
return409 and never log out, terminate, or reconfigure the user's desktop.
Status includes `canResize:false`. The UI labels console entries and hides
managed-only actions. No automatic consent provisioning on offer or failure.

The pilot accepts one viewer. H.264 encoding runs on-device via the restricted
PipeWire connection; RTP is relayed unchanged. Input uses existing
`dwconsole.input`0.2.0 framing, exact sequence/generation, one-second renewable
lease and held-key/button release on reset, disconnect, expiry or service stop.
Coordinates are bounded to the negotiated video raster and mapped to the
portal stream's logical coordinate size. Input is standard RemoteDesktop Notify
methods, not uinput or a compositor-private API. Clipboard and display resize are
unsupported; logout/portal closure removes availability, not creates a desktop.

On logged-in GNOME X11 hosts, the agent uses session-authorized FFmpeg x11grab/XTest
instead of trusting nonfunctional portal frame/input acknowledgements. Same
lease, coordinate, event validation and gateway rules apply. It never selects a
greeter/root X authority. Optional console-only gateway sockets use an
operator-provisioned localuser:dwdesktop-agent runtime directory and0660 socket;
they return an empty managed-environment inventory and cannot create desktops.
