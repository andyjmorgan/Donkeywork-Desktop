# Local console input adapter — 0.2.0

Experimental single-host, single-output control. No auth UI/fleet endpoints.
No upstream implementation copied. Complements, not replaces, the existing
H.264 protocol. Display-only deployments remain supported unchanged.

## Privilege and binding

Root launcher optionally connects the root-private input socket and passes it
as fd3 to the already unprivileged Go bridge (`--input-fd 3`). Neither browser
nor Go owns uinput. One private connected stream can acquire/release successive
control generations without re-opening the root socket. No automatic takeover.

Helper accepts unlimited controller duration only with a root-owned, non-group/
other-writable guard JSON file, fresh within500ms: `session`, `width`, `height`,
`valid`. Invalid/stale guard or changed identity/dimensions revokes held input.
The minigpu producer checks logind ActiveSession, Mutter single-output 1:1 mode
and DRM active mode every100ms, and latches invalid on change/error. Restarting
the guard creates a fresh identity. It does not automatically follow a login.
These are conservative polling bounds, not atomic kernel/session transaction
guarantees. Capture/source auto-handoff remains a later acceptance gate.

## Private framing and state

Every message uint32BE length then strict JSON,1..4096 bytes. Root peer checks
on both endpoints. IDLE has no held input and no active lease.

1. IDLE receives `{"type":"acquire","requestId":"optional-bounded-id"}`.
   Optional requestId (1..64 bytes when present) correlates denied acquisitions;
   bridge supplies a fresh ID. Denied unavailable echoes it; asynchronous
   revocation omits it. This separates stale expiry replies from a current denial.
2. Valid target returns `{"type":"ready","protocol":"dwconsole.input",
   "version":"0.2.0","generation":"random-token","width":1920,
   "height":1080,"leaseMs":1000}`. Fresh sequence starts1.
3. ACTIVE requests retain `{"generation":"...","sequence":1,"event":{...}}`.
   Events: move(x,y), button(button1left/2right/3middle,down,x,y),
   wheel(vertical,horizontal,x,y), key(hid,down),
   reset, renew, release. Unknown fields/usages and malformed values rejected.
4. ACK is `{"type":"ack","sequence":1,"accepted":true}`.
5. Release resets held state, ACKs then returns IDLE. Reset clears held state
   without ending control. Only explicit renew extends the1-second lease.
6. Expiry/guard failure resets and emits `{"type":"unavailable","reason":
   "control expired"}`, then IDLE. Old generation requests cannot inject.

No automatic event replay. Absolute read/write budgets, partial-frame handling,
bounded queues and one ordered injection executor apply. SIGTERM graceful
cleanup is implemented/tested separately; hard kill still relies on device
removal. Root CLI unguarded proof remains capped10seconds.
Up to8 local connections may be idle; a single global owner holds input. An
idle web connection cannot monopolize the helper or prevent CLI acquisition.
Closing/releasing a non-owner must never reset the actual owner's held state.

## WebRTC messages

Browser opens one **ordered, reliable** data channel `dwconsole.input` on the
same PeerConnection as video. No keyboard/mouse network split. The bridge
allows one controlling viewer and validates hello dimensions against video.

- Browser `{"type":"acquire"}`; bridge returns ready or unavailable.
- Browser `{"type":"event","generation":"...","sequence":1,"event":{...}}`;
  bridge forwards only the private Request, returns ACK/unavailable.
- Browser `{"type":"release"}`; bridge serializes release with outstanding
  input and releases ownership, then sends `{"type":"released"}`. Browser
  waits for this barrier before a new acquisition. No generated keepalives:
  only browser renew.
- A viewer close or capture failure releases control; queue overflow/staleness
  fails closed. Input-bearing payloads are never logged.

Browser first focus click acquires without injecting that click. Input only
while focused, visible and displaying video. Renew250ms, single in-flight ACK,
bounded queued events/age. Coalesce only unsent motion without crossing state
boundaries. Blur, hidden, pointercancel/capture loss and disconnect reset/release.
Delayed acquire response after abandonment is discarded; no automatic reacquire.

Coordinates use contain-fit CSS content rectangle to physical source pixels.
Ignore clicks in bars, clamp captured drags, preserve release outside viewport.
Physical key code maps to HID; host owns repeat. Wheel deltas are bounded to
[-32,32] ticks per axis and injected as Linux REL_WHEEL/REL_HWHEEL. Relative
pointer, text/IME/clipboard and OS-reserved shortcuts are not approximated.

## Required fixtures and live gates

Rust: state/lease/guard/stale generation/malformed framing/deadline tests.
Go: strict browser frames, ownership, queue/release/closure tests.
Web: geometry, ordering, focus/release, generation/ACK and backpressure tests.
Live: minigpu greeter click/Escape from browser, blur releases held state,
guard loss revokes without affecting output, and second viewer cannot take over.
No login/credential test or cross-host acceptance inferred from these checks.
