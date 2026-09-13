# Minigpu console input probe

2026-09-06, 18:43–18:47 UTC. Original Rust helper and CLI, source in
`device/console/src/{input.rs,input_device.rs,bin/input-daemon.rs,bin/input-cli.rs}`.
Probe framing documented in `contracts/console-input-probe.md`.

## Observed results

- uinput keyboard: `/sys/devices/virtual/input/input15`, ID_INPUT_KEYBOARD=1.
- uinput pointer: `/sys/devices/virtual/input/input16`, ID_INPUT_MOUSE=1,
  absolute X/Y plus relative capabilities. Both tagged seat (default seat0).
- Active console was GDM Wayland session c7, UID124; VKMS 1920x1080.
- CLI `click 960 490` selected localuser on the actual greeter. Browser H.264
  capture visibly changed from user list to password prompt. No credentials
  entered. Evidence `artifacts/console-web/minigpu-input-click.png`.
- CLI `key 41` (USB HID Escape) returned to user list, verified through browser
  video. Evidence `artifacts/console-web/minigpu-input-escape.png`.
- Independent event-node observer opened ONLY our two virtual devices, with
  no grabs. Injected Shift + left button at blank point100,800. Explicit
  key/button down and up events observed for both disconnect and lease expiry.
  Initial observed last release: disconnect0ms, expiry1030ms. These are local
  observer elapsed times after ACK, not end-to-end application latency.
- Rust suite:8 library tests and1 fragmented-frame deadline test passed.
- Browser geometry separately has22 new tests; integrated into video worktree
  as5944c37. Geometry fixtures are not compositor-mapping proof.

## Deployment and rollback

Built debug binaries copied to `/tmp/input-daemon` and `/tmp/input-cli` on
minigpu. Transient `dwconsole-input-probe.service`, root-only private socket
`/run/dwconsole-input/input.sock`, runtime directory0700. No network listener,
no browser input endpoint, no display-manager restart or machine reboot.
Stop that exact transient unit to remove virtual devices. `/tmp` probe binaries
and `/tmp/dw-input-release-probe.py` can subsequently be removed explicitly.
Existing capture/web services are independent and unaffected.

## Not established

This is a bounded10-second CLI-controller proof, not production control.
No login/logout, mixed-DPI, multiple displays, four-corner accuracy, actual
resolution transition, clipboard, mobile input or Spark/East input acceptance.
Width/height are operator-supplied. Capture/session/topology generation binding
must exist before browser input is enabled. Default SIGTERM kernel device
teardown is not an explicit graceful-release implementation/test.

Review caught lease renewal on every input request and ACK-backpressure drift;
correction is explicit Renew only, timestamped before ACK and writes bounded
by remaining lease. The initial release figures above precede that correction;
record the repeat separately rather than silently reusing them as new evidence.

## Corrected helper regression — 18:49 UTC

Rebuilt and redeployed corrected helper; new virtual nodes input17/input18.
Independent event observer verified both Shift and left-button down/up:

| Failure | Last observed release after request ACK |
|---|---|
| Connection close | 0 ms (rounded) |
| No Renew heartbeat | 1009 ms |
| Continuous ordinary motion, no Renew | 1008 ms |

No real credentials or authentication attempt. Full Rust suite now12 tests,
including blocked ACK deadline, ordinary-event nonrenewal, expired Renew and
inode-safe socket cleanup. Stopped the transient input probe after acceptance;
virtual devices removed, capture and web services confirmed still active.
Probe binaries/scripts remain in /tmp for reproduction. No browser input enabled.
