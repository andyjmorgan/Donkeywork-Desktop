# Console input probe — 0.1.0

Historical first proof. Current persistent CLI/web adapter is0.2.0; see
`console-input-web.md`. Do not use this old handshake with current binaries.

Private root-only CLI experiment, not yet a browser input endpoint. No changes
to managed-session contracts. All messages uint32 BE length + strict JSON,
maximum 4096 bytes, no empty records. Root-owned 0600 Unix socket in root-owned
0700 directory; both endpoints check SO_PEERCRED uid 0. Single controller.

Server hello: protocol `dwconsole.input`, version `0.1.0`, fresh random
`generation`, `width`, `height`, `leaseMs:1000`. Dimensions are operator-selected
for a single-output proof; automatic topology validation is not implemented.
Do not connect the web bridge until source/seat/topology binding is integrated.

Request: `generation`, contiguous `sequence` starting at 1, `event`:

- `{"type":"move","x":0,"y":0}` source-raster coordinates.
- `{"type":"button","button":1,"down":true,"x":0,"y":0}` (1/2/3 left/right/middle).
- `{"type":"key","hid":41,"down":true}` USB page7 allowlist, not Linux keycode.
- `{"type":"reset"}` releases all injected held state.
- `{"type":"renew"}` renews the control lease.

Reply: `sequence`, `accepted:true`; ACK means injection returned successfully,
not target application consumption. Invalid request, framing, generation,
sequence, timeout or disconnect closes connection and resets held state.
All requests execute serially. No replay or automatic retry. Read deadline
applies to an entire frame, not each fragment. Output errors also reset.
Lease initially expires 1 second after hello. ONLY explicit valid `renew`
extends it, to 1 second after that request's acceptance timestamp. Motion,
keys, buttons and reset never renew; delayed requests cannot revive an expired
lease. ACK transmission does not renew it. Each response has an absolute write
budget bounded by both the remaining lease and 100 ms, including partial writes.
Maximum control lifetime is 10 seconds for this bounded probe, regardless of
renewals. Kernel timeout rounding/scheduling can slightly delay observed release;
this is not a hard real-time guarantee.

Normal cleanup removes only the bound socket pathname if its device/inode still
match; replacement paths are preserved. Default SIGTERM/SIGKILL bypass Rust Drop:
kernel file-descriptor/device teardown is the fallback, not an explicit key-up
guarantee. Signal-triggered release and stale-socket recovery remain unverified;
do not infer them from client-disconnect tests. No signal handler is installed.

CLI offers move, click, key and bounded hold operations; all use this protocol.
No arbitrary text, clipboard, relative movement, wheel or network exposure.
Power/SysRq and unknown usages are rejected. No payload logs.

Acceptance: schema/sequence/bounds unit tests, release-on-drop backend tests
where possible, then real minigpu classification, harmless greeter action and
client-death release. Device creation alone is NOT readiness. Host transition
and topology invalidation remain blockers to browser integration.
