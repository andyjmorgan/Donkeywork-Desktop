# Minigpu browser input and session recovery

## Implemented and observed

Original Rust uinput helper/private IPC, inherited-fd unprivileged Go bridge,
one reliable ordered WebRTC input lane, React keyboard/mouse capture. Browser
remains at http://192.168.69.21:8090. First click acquires; subsequent input is
remote. Blur/release/transport failure clears ownership; no implicit takeover.

At19:02–19:04 UTC, browser click selected the real GDM user tile; browser Escape
returned to the user list. Both H.264 screenshots visually inspected:
`artifacts/console-web/web-input-click.png`, `web-input-escape.png`.
No credentials or login action performed by the test. Acquire/release/reacquire
and blur-release passed with zero page errors. Independent observer of ONLY
our virtual keyboard confirmed the injected modifier and its release during
the browser blur test. This is stronger than a status-label assertion.

Guard-freeze live probe: both held Shift and mouse button released430ms after
producer SIGSTOP despite ongoing Renew traffic; producer SIGCONT restored.
Normal disconnect0ms rounded; missing heartbeat about1000ms. Polling evidence,
not a hard real-time guarantee.

CLI/browser coexistence initially failed by design of the serial socket loop;
fixed to allow8 idle private peers with one globally arbitrated owner. Live
non-injecting test passed: CLI denied while browser owns, CLI reset succeeds
while web idle, then browser reacquires. No keyboard/mouse events in this test.
Acquisition denial IDs distinguish an actual refusal from a stale expiry reply.

## User login exposed missing lifecycle integration

At19:05 Andrew logged in, changing GDM c7/UID124 to Wayland user session5838/
UID1000. Guard correctly revoked old input. New compositor defaulted1024x768;
capture briefly had no active plane, then Go restarted and served1024x768.
The pinned1080p greeter guard could not re-enable input. This was a missing
handoff implementation, not accepted session-switch behaviour.

Root restored the new user's output to1920x1080 with temporary DisplayConfig
mode, restarted only task services and verified the real Ubuntu desktop through
browser video at19:10. No GDM restart, host reboot, logout or user-app changes.

At19:13 installed and started `dwconsole-seat-supervisor.service`. Source:
`deploy/vkms/session-supervisor.py`. It follows public logind seat/session and
the active account's GNOME Shell/private bus. On change or stale/invalid guard:

1. Stop task web/input/guard/output services in that order, releasing old input.
2. Hold an idle inhibitor and configure the existing VKMS console to1080p.
3. Wait for both Mutter and DRM1080p—not just requested-mode metadata.
4. Start fresh guard, wait valid/fresh state, start input, then new web feed.

Guard additionally fingerprints lock state. No GDM/user-process restart. No
queued input crosses via reconnect; browser requires deliberate acquisition
after fresh video. Unknown/multiple GNOME Shell instances fail closed. A
display-only fallback is attempted if preparation fails.

## Current deployment

Supervisor owns transient `dwconsole-vkms-{web,output}`, `dwconsole-input-probe`
and `dwconsole-input-guard`. Existing capture is independent. Root-owned files
under `/opt/donkeywork-desktop/bin`: input-daemon,input-cli,input-target-guard.py,
session-supervisor.py,web-launch-input,console-web-input,keep-console-output.py.
Web assets updated; prior display-only binaries retained as
`web-launch-viewonly` and `console-web-viewonly`. No public endpoint/auth changes.

Stop supervisor before manual operations on its owned task units; otherwise it
will restore them. Stopping it also stops those four units, not capture/GDM/apps.
No boot persistence installed here. Existing VKMS/udev configuration unchanged.

## Verification and limits

- Rust20 tests passed; Go race tests and vet passed; browser69 tests/build passed.
- Live browser click/Escape, release/reacquire/blur and own-device release passed.
- Live CLI/browser arbitration passed without real input.
- Automatic preparation adopted the already logged-in session; full actual
  logout/login through the new supervisor still awaits the user's next transition.
- At19:14 deliberately stopped only the guard unit. Supervisor automatically
  rebuilt a fresh guard/controller/viewer for unchanged user session5838. After
  its readiness log, browser1080p video passed (16→47 decoded frames,0 drops),
  and CLI/browser ownership/reacquisition passed again. An immediate new-page
  request during the restart briefly got connection refused; existing clients
  retry automatically. User session remained active throughout.
- A second-viewer test begun before Andrew's login failed before acquisition;
  do not count it as a passed second-viewer live test. Go ownership tests pass.
- Single GNOME Wayland VKMS output,1:1 scale, fixed1080p pilot. Wheel, clipboard,
  touch/IME and Meta/AltGraph not enabled; browser explains unsupported paths.
- No user-session disruption to manufacture a login/logout test. Spark/East/Rocky
  input rollout remains separate. Physical/encoded scaling and real resolution
  changes need further acceptance beyond this fixed-mode pilot.
