# Spark package X11 control validation — 2026-09-06

## .8 current result: display, absolute pointer and keyboard accepted

`0.1.0-lab.20260906.8` corrects the mixed pointer descriptor: absolute X/Y
with relative **wheel** axes only, no relative position axes. Rust was rebuilt
natively on Spark with offline/locked Cargo; all25 Rust tests passed.
Go and browser assets were reused unchanged. Service-only installer upgrade
kept the same boot, user desktop and display manager running.

Independent X server observations, not protocol ACKs:

- CLI moves `(960,540)` and `(300,200)` produced those exact `XQueryPointer`
  coordinates.
- Browser at1120×760 rendered the1920×1080 source letterboxed/scaled. Real
  browser pointer movement targeting source `(960,540)` and `(300,200)`
  produced exact X server coordinates.
- Browser Shift down/up produced `XQueryKeymap` down=true/mask1 then
  down=false/mask0. No remote clicks or text were injected into open apps.
- Explicit UI release succeeded; subsequent CLI acquired and restored the
  original pointer `(0,0)`, independently verified with no held modifier/button.
- Browser decoded90 frames with zero drops. The inspected
  `artifacts/spark-package/control-view.png` shows the existing GNOME desktop;
  visible telemetry30.0FPS and510.0KB/s at capture time.
- All six package services, including transient guard, remained active.
  Web PID339034 runs UID/GID65534 with NoNewPrivs1.

The browser harness first needed `bringToFront()` before video focus because
input correctly rejects an unfocused document. No product bypass was used.
Proof harness: `/tmp/dwdesktop-spark-control.OY9uYw/browser-proof.mjs`.

Current ARM64 archive SHA256:
`4a7487965bf96bd2fdbe51e440780aef38f9bea34b7b9ef0f27d673b48cfef84`.
Exact integrated source SHA256:
`6163d4d3742d0e6c1d2bb1b1501f8f6e8d8157218c350b6479b1892acfc4b0e7`.
Local archive: `artifacts/releases/donkeywork-desktop-0.1.0-lab.20260906.8-linux-arm64.tar.gz`.

Scope: current logged-in console display/input proven. This test did not
log out, switch to the greeter, reboot, or establish session handoff acceptance.

## .7 intermediate result (not full input acceptance)

Spark `192.168.69.28` was upgraded service-only from `.5` to
`0.1.0-lab.20260906.7`, with `PROFILE=x11 CONTROL=x11`, using the package
installer `--replace-config --activate`. No reboot, GDM restart, logout,
dependency installation, driver or firewall change occurred.

Boot ID remains `ea0c946a-1aca-4533-96a5-1929ea0fd15c`. The existing logged-in
session is `4`, UID1000, display `:1`, VT2, HDMI-0 1920×1080.

- Capture, web, broker, input, session supervisor and target guard started.
- Guard independently verified the current root capture PID296707 and X11
  display/authority/single unscaled output; guard remained valid and fresh.
- Browser playback showed the actual existing GNOME desktop, 1920×1080 H.264,
  46 decoded frames, one dropped frame, zero JavaScript errors. Inspected
  `artifacts/spark-package/live-view.png` at 1120×760.
- CLI Shift HID225 held1000ms: independent read-only libX11 `XQueryKeymap`
  observation saw down and then up across100 polls; final modifier mask0.
- **Pointer failed:** CLI moves `(960,540)` and `(300,200)` ACKed but independent
  `XQueryPointer` remained `(0,0)`. Xorg adopted both DonkeyWork devices, then
  logged `Discarding absolute event from relative device. Please file a bug`.
  Existing mixed ABS_X/Y and REL_X/Y capabilities cause Xorg/libinput to choose
  a relative pointer. ACK is not acceptance; pointer descriptor needs correction.

The read-only observer uses ctypes libX11 only; it does not inject events.
Live scripts/staging: `/home/localuser/dwdesktop-spark-control.RW2mts`.
Local scripts/staging: `/tmp/dwdesktop-spark-control.OY9uYw`.

## Provenance and rollback

ARM64 `.7` reuses `.3` binaries after Rust source/Cargo manifest+lock, Go tree,
and UI source+lock comparisons found no changes. Current Python/helpers,
units, installer, doctor and exact source snapshot replaced the packaging.
Native ELF/runtime dependency and package checksum verification passed.

- Archive SHA256: `eca713041aa4428c2f34460778069525591861f584248882d38ab97ddb962471`
- Source snapshot SHA256: `dab8b8d7f4c2531c601179bc01512fb796a7f933a949176e7fad0660e6e13851`
- Prior `.5` config/current/unit backup:
  `/var/backups/donkeywork-desktop-spark-control.1w83d1/pre-control-complete.tar.gz`
- Backup SHA256: `94db6eb1a48da9c947e208c4a24c0577ba02cf0cdc1c6aafa4593bb9fc8ce242`

`.5` release and old pilot files remain intact. Restoring the backed-up config,
current symlink and units followed by daemon-reload/package target restart
returns to the working view-only `.5`; no display-manager action is needed.

Guard fixes included signed/wrapped RandR timestamps, actual DRM timing
dimensions instead of mode labels, same-display authority alias selection
through root Xorg's matching VT, and KMS connector-to-active-CRTC correlation.
Twenty package tests passed; live read-only guard/discovery passed on both
Spark and Easternkingdoms. These checks do not establish login handoff or
greeter control acceptance.
