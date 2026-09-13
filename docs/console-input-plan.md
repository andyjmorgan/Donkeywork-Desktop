# Console input implementation plan

Status: implementation gates, not a claim of working input. 2026-09-06.
Scope: existing graphical console; no managed desktop, fleet broker, auth UI,
BIOS access or clipboard. Preserve current working video paths.

## Decisions and evidence

Reference review: `../desktop-console-input-design/docs/reviews/2026-09-06-physical-console-input-design.md`
in the sibling worktree (commit 633175ca08a24e83c4943046a5a18940fe9736f9).
Fresh source checks completed by RustDesk and JetKVM review agents.
Upstream source informs original design; no implementation or assets copied.

| Decision | Upstream evidence | Our remaining proof |
|---|---|---|
| Privileged, narrow input helper; try uinput keyboard and absolute pointer | RustDesk `src/server/uinput.rs` at 692113c87e476c2e8a82a7cdb9aa98354abc11fc | Device classification, active-seat routing and absolute mapping on each compositor |
| One reliable ordered input data channel on existing WebRTC connection | Existing review's RustDesk shared input queue; JetKVM separates hover but not all input | Bounded latency and release under congestion; no separate mouse network connection |
| Physical key identity, host layout | JetKVM `WebRTCVideo.tsx` and keyboard hook at d58e6e44d795b34969d4b5e35511ff2adc577afb | AltGr, left/right modifiers, keypad and repeat on our browsers/hosts |
| Content-rectangle mapping, not video element bounds or DPR multiplication | JetKVM `ui/src/hooks/useMouse.ts` | Letterbox, zoom, fit, fullscreen and host scaling tests |
| Host watchdog releases keys AND modifiers AND buttons | Both projects track held input; JetKVM ordinary-key timeout excludes modifiers | Kill/disconnect/tab-hide tests and stale generation rejection |

Capture choice does not choose the input backend. KMS/VKMS are output paths,
not keyboard/mouse protocols. Do not declare uinput universal before testing.

### Verified review deltas

All references below use the pinned commits above; these are source findings,
not reproduced upstream bugs or our live acceptance results.

- RustDesk `libs/enigo/src/linux/nix_impl.rs:155,278` selects XDO/TFC on X11;
  `src/server/input_service.rs:2379` selects uinput for non-X11 server mode.
  Shared uinput on Spark is our experiment. If it fails, use an explicitly
  selected X11 injector; do not force an allegedly universal backend.
- RustDesk `src/server/uinput.rs:1151` configures absolute and relative axes,
  buttons and BUS_USB without explicit seat/output association in that module.
  Its 300 ms sleep at line 1240 and refresh ACK at line 873 are not compositor
  adoption proof. Check every device operation and verify host-visible action.
- RustDesk `src/server/input_service.rs:921` has 360-second stale-key cleanup;
  `reset_input_ondisconn` at line 1435 is macOS-only. Our one-second all-held-
  state watchdog is an original requirement, not an inherited guarantee.
- JetKVM `internal/hidrpc/hidrpc.go:31` and `webrtc.go:267,538` dispatch keyboard
  and pointer to separate workers. Keep ONE ordered executor after validation,
  not merely one transport. Test modifier-down/click/modifier-up under load.
- JetKVM `ui/src/hooks/useHidRpc.ts:250` splits absolute motion from button
  transitions, but both carry button state; `internal/hidrpc/message.go:144`
  has no cross-channel generation/barrier. Do not adopt that split initially.
- JetKVM `WebRTCVideo.tsx:591` installs mouse blur reset only in absolute mode;
  inspected `webrtc.go:624,698` close paths clear keyboard, not mouse. Its
  ordinary-key timeout excludes modifiers (`hid_keyboard.go:668`). Cover all
  held state in our watchdog and fault tests.
- JetKVM uses USB HID hardware, not Linux uinput. Its browser behaviour does
  not establish Wayland seat routing or compositor coordinate mapping.

## Work lanes and order

1. Source reviewers: check existing review for missing guarantees and report
   exact references. No host changes. Root integrates evidence and decisions.
2. Root: freeze a small experimental console-input contract with fixtures and
   negative tests; independent of managed-session contracts. Implement helper
   and CLI proof first. No browser-to-root arbitrary event forwarding.
3. After host proof: browser/bridge lane implements geometry, focus and the
   negotiated input contract. Kernel helper owns release guarantees.
4. Integration lane: reproduce host-visible actions and failure tests; record
   pass/fail/untested by host. No success inferred from IPC acknowledgment.

## Contract requirements before code integration

- Separate root-private control socket from one-way video socket. Unprivileged
  bridge inherits a preconnected, verified descriptor; never owns /dev/uinput.
  The CLI reaches the same validator and injection engine.
- Bounded framed messages; strict version/type/size/allowlist validation.
  One controller; additional viewers are view-only. No automatic takeover.
- Server-issued control generation, monotonic sequence, source identity and
  topology revision. Reject stale/replayed actions; never replay uncertain clicks.
- Key down/up uses USB HID page 7 allowlist, not arbitrary Linux key codes.
  Exclude power/SysRq. Text/Unicode injection is not physical key injection.
- Pointer transitions include absolute target coordinates; ordered move,
  button, wheel, key and reset. Coalesce only unsent hover, never across a
  button/key boundary. Explicit wheel units; fractional accumulation.
  Preserve this order through one host executor, including mixed key/mouse
  batches; separate backend workers must not reorder modifier-plus-click.
- Initial watchdog target: renew every 250 ms, expire after 1 second. Measure
  before calling these values final. Expiry revokes the generation and releases
  all injected held state, independent of browser cleanup or incoming traffic.
- Queue bounds and timeout fail closed; delayed packets cannot renew an expired
  generation. ACK means injection accepted, not application consumption.
- Reset is always possible while topology changes suspend other input.
  Seat/session, capture-source or topology changes release and invalidate input.
  Resume only against the newly displayed target, not simply a live socket.
- No event payload, password or clipboard content in logs.

## Host proof: bounded and observable

Start with minigpu, then Spark. Easternkingdoms is the working workstation:
no reboot, display-manager restart, logout or lock test there. Use only a
bounded harmless target for live input; do not type into an unknown window.
Rocky joins after its output agent completes; no competing live operators.

For each host, record udev classification, device seat, compositor discovery,
actual output count/geometry/scale and observed action. Device creation is not
readiness. Capture evidence must show the host reaction, not a browser cursor.

| Test | minigpu VKMS/Wayland | Spark X11 | East Intel/X11 | Rocky VKMS/Wayland |
|---|---|---|---|---|
| Keyboard recognition + harmless down/up | PASS: Escape returns greeter user list | Untested | Untested | Untested |
| Absolute centre/corners and click without preceding move | Centre click PASS; corners untested | Untested | Untested | Untested |
| Held modifier/button release on client death | PASS: own evdev observed Shift/left release | Untested | Untested | Untested |
| Greeter/user transition routing | Untested | Untested | Deferred: active workstation | Untested |
| Host scale/mode change invalidates old coordinates | Untested | Untested | Non-disruptive checks only | Untested |

Use the real greeter to prove a harmless selection/back action without
credentials. Full login acceptance needs an explicitly selected test account;
do not retrieve credentials just to fill a matrix. Full-corner accuracy needs
a visible target surface and independent cursor/action observation. Primary
KMS capture may omit the hardware cursor: local crosshair motion is not proof.

## Browser acceptance after host proof

- Map CSS client coordinates through the actual contain-fit content rectangle
  to source pixels. Ignore new clicks in bars; drag capture clamps movement
  and always releases outside the viewer. Do not multiply by devicePixelRatio.
- Test centre/corners, 16:9 in wide/tall containers, scrolling, browser zoom,
  fullscreen and encoded-downsampled video. Initially one active output only;
  unsupported multi-monitor layouts fail explicitly.
- Capture only while viewer focused. Blur, hidden tab, pagehide,
  pointercancel/lost capture and connection loss trigger reset.
- One repeat authority: host repeat, not duplicated browser repeat downs.
  Preserve left/right modifiers; test AltGr and missing Meta key-up behaviour.
- Ordinary HTTP viewer must work without Keyboard Lock, Pointer Lock or
  Clipboard APIs. Reserved chords need explicit UI controls later; don't
  promise the browser receives every OS shortcut.
- Relative pointer, clipboard, IME and mobile soft-keyboard text are deferred
  capabilities. Their absence must be explicit, not silently approximated.

## Exit criterion

Ship browser keyboard/mouse only after host injection and failure cleanup pass
on the declared pilot. Publish supported combinations and remaining tests.
No claim of battle-hardened behaviour merely because a design resembles prior art.

First implementation evidence: `docs/validation/minigpu-console-input-2026-09-06.md`.
Minigpu browser input is now live with guard revocation and a session supervisor.
See `docs/validation/minigpu-web-input-2026-09-06.md` for actual browser/CLI
tests and the login-discovered handoff fix. Full logout/login acceptance under
the new supervisor and other-host input validation remain outstanding.
