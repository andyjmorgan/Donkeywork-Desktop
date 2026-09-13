# Easternkingdoms physical KMS package readiness — 2026-09-06

Result: **package preflight passed, followed by an explicitly authorized live
KMS/H.264 proof with 1,801 additional decoded frames and no drops over one
minute**. Easternkingdoms remains an active fleet candidate, not an
omission: Intel physical KMS, view-only package profile, with JetKVM attached.

This is the development workstation. Initial inspection was read-only; root
subsequently authorized installing and starting only the package services.
No input injection, reboot, display-manager restart, logout, module change,
modeset or cable operation occurred. Never restart the workstation or its
display manager as a test.

## Current evidence

| Item | Observation |
|---|---|
| Host | easternkingdoms, `192.168.69.17`, x86_64 |
| OS/kernel | Ubuntu 24.04.4 LTS, `6.17.0-35-generic` |
| DRM | `/dev/dri/card1`, driver `i915` |
| Physical output | HDMI-A-2, connected and enabled, CRTC 88/pipe A, 1920×1080 at 60 Hz |
| Primary plane | 33, XR24, pitch 7680, Intel X-tiled modifier `0x0100000000000001` |
| Cursor | Separate AR24 cursor plane 83; not included in the earlier primary-plane proof |
| Existing session | LightDM active, seat0 session `c1`, X11 user `localuser` UID 1000 |
| DRM master | Xorg PID 2262 remains sole master |
| Viewer ports | No listeners on TCP 8090 or UDP 8091 at inspection |
| Package installation | Initially absent; `.5` now installed and package target started, not enabled for boot |
| Bundle | `0.1.0-lab.20260906.5-linux-amd64`, Ubuntu 24.04.4 build |
| Compatibility | Bundle-aware doctor returned zero failures; ELF/runtime library checks passed; `/usr/bin/ffmpeg` is root-owned 0755 with libx264 |
| Installer | Exact `.5` extracted payload checksum/target dry-run passed; no writes |

The earlier dedicated worktree `desktop-east-kms`, branch
`work/east-kms-validation`, commit `02c6026`, verified two primary-plane stills
against independent X11 root captures byte for byte, with the displayed clock
advancing. See that worktree's
`docs/validation/easternkingdoms-kms-2026-09-06.md` and original
`device/console/examples/east-kms-still.c`. That result is still-capture evidence,
not a sustained H.264 streaming or current JetKVM-image comparison.

## Intel conversion: distinguish the proven probe from the current daemon

The still probe uses `drmtap_grab_desc` plus `drmtap_convert_dmabuf`, verifying
linear output metadata. That explicit descriptor/conversion path has **not**
been integrated into `device/console/src/bin/daemon.rs`; the daemon still calls
`capture.grab_mapped()` and repacks its returned data/stride as BGR0.

However, `grab_mapped` is not simply a raw tiled mmap. The pinned
`libdrmtap-sys-0.5.4/csrc/drm_grab.c` `do_grab` path invokes
`gpu_auto_process` (around lines 1205–1250), and its EGL path (around
1438–1462) returns linear XRGB8888 with a width×4 stride. It retains the source
modifier metadata, which caused the earlier strict still probe to reject its
otherwise converted result. The main daemon does not test that modifier.
`readelf -Ws` confirms the `.5` binary contains a substantive
`drmtap_gpu_egl_convert` implementation, not merely the absent-EGL stub.

Therefore there was **no demonstrated mandatory missing Intel conversion fix**
blocking the bounded package trial. The implementation-path difference is now
covered by the follow-up: successful original stills used the descriptor API,
while the packaged mapped-capture stream also produced correct inspected
pictures. This does not establish every Intel format or modifier. Do not
introduce a modifier rejection or reinterpret raw tiled bytes as linear merely
to satisfy a test.

libdrmtap's `drmtap.c` around lines 336–350 drops any implicit DRM master when
CAP_SYS_ADMIN is held, allowing the existing display server to remain master.
The package KMS path requests capture, not modesetting or output adoption.
This supports a non-disruptive trial; it cannot guarantee zero GPU-driver risk
or acceptable resource use before the trial is observed.

## Initial preflight commands

Executed successfully from the main repository:

```sh
bash packaging/doctor.sh --profile kms --device /dev/dri/card1 --bin-dir /home/localuser/source/Donkeywork-Desktop/artifacts/releases/.build-0.1.0-lab.20260906.5.dtCoM0/donkeywork-desktop-0.1.0-lab.20260906.5-linux-amd64/bin

bash artifacts/releases/.build-0.1.0-lab.20260906.5.dtCoM0/donkeywork-desktop-0.1.0-lab.20260906.5-linux-amd64/packaging/install.sh --source /home/localuser/source/Donkeywork-Desktop/artifacts/releases/.build-0.1.0-lab.20260906.5.dtCoM0/donkeywork-desktop-0.1.0-lab.20260906.5-linux-amd64 --profile kms --listen 192.168.69.17 --device /dev/dri/card1 --ffmpeg /usr/bin/ffmpeg --dry-run
```

After subsequent explicit installation/start authorization, the same installer arguments
without `--dry-run` install **without activation**. Then `systemctl
daemon-reload` and `systemctl start donkeywork-desktop.target` start only the
package. Do not use `--configure-vkms` on this host. The `kms` profile does not
start the input helper and its session-supervisor entrypoint returns without
running GNOME output preparation. No GDM/LightDM/session/physical-mode command
belongs in this trial. Do not enable boot startup until acceptance is agreed.

With a deliberately started viewer, the existing read-only browser smoke is:

```sh
node tests/integration/live-console-smoke.mjs http://192.168.69.17:8090
```

It writes `artifacts/console-web/live-view.png` locally. Inspect the image,
observe frame/content freshness, CPU/GPU load and Xorg's unchanged DRM-master
ownership; have the user independently confirm JetKVM still renders. Stop
only `donkeywork-desktop.target` if the bounded trial fails. No restart of the
machine or display manager is a fallback.

## Authorized service-only follow-up — 20:36–20:40 UTC

Installed `.5` with the above KMS/card1/private-IP/FFmpeg arguments, omitting
`--dry-run` and without `--activate` or `--configure-vkms`. Then performed
`systemctl daemon-reload` and `systemctl start donkeywork-desktop.target`.
The target remains **disabled for boot**, verified with `systemctl is-enabled`.

The browser at `http://192.168.69.17:8090` rendered the actual existing GNOME
desktop and terminal correctly, not black or visibly tiled. Initial smoke:
1920×1080, 15→47 decoded frames, zero drops and no page errors. A second
view-only run observed 32→1833 frames over approximately 60 seconds, zero
drops. Private evidence is under
`artifacts/easternkingdoms-package/stream-sIsIzb/`; `after.png` was visually
inspected. The displayed desktop clock advanced from 20:37 in the first smoke
to 20:39 in the follow-up, establishing content freshness beyond advancing
decoder counters. Images contain existing private desktop contents and were
not uploaded.

Process identities remained capture 1126773, web 1126775, broker 1126772;
Xorg 2262 remained sole DRM master and LightDM 2217 remained active. HDMI-2
stayed 1920×1080 at 60 Hz. Kernel warning-or-higher journal entries since the
trial began were empty. The capture process appears as a non-master DRM client.
No independent JetKVM web image was read; user confirmation of its rendered
picture remains separate from unchanged HDMI mode/master evidence.

An indicative process sample showed capture around 12% of one CPU and software
FFmpeg around 69%, with web around 2%; these are process lifetime averages, not
a controlled benchmark or end-to-end latency result. Input remains view-only
pending the coordinated X11 target guard for this LightDM session; output must
remain KMS rather than replacing it with an X11 capture session.

The service-only rollback is `sudo -n systemctl stop donkeywork-desktop.target`.
Installed release/configuration remain recoverable on disk. No uninstall or
display reset is needed to stop our capture.

## Physical HDMI removal/replug: current behaviour, not a tested promise

For Easternkingdoms, physical KMS depends on an active scanout. If HDMI removal
causes the compositor to tear down the primary plane/framebuffer, libdrmtap
returns an error (`do_grab`, no active plane/framebuffer); the daemon ends that
feed. The FD broker lets Go acquire fresh connections while the existing
viewer stays in `recovering`. If the driver/compositor retains a scanout,
connector removal does not itself guarantee a capture error: repeated decoded
frames are not proof a physical monitor is still connected.

Replug recovery currently requires the replacement header to match **Device,
DisplayID/CRTC, geometry, FPS, pixel format, encoder and codec/protocol fields**
(`console-web/recovery.go`, `compatibleCapture`). A changed framebuffer ID is
not a header change. A reassigned CRTC, different mode or different DRM device
is: it can leave the viewer waiting even after the physical desktop returns.
There is no automatic physical→VKMS fallback or general topology renegotiation.
No removal/replug was performed here; retain this as a separate acceptance gap.

## Moving JetKVM to minigpu: active output-policy hazard

The current minigpu VKMS helper is deliberately prescriptive:
`deploy/vkms/keep-console-output.py` constructs one logical monitor containing
only `Virtual-1` at 1920×1080 and invokes `ApplyMonitorsConfig` method 1. Any
other outputs are omitted from the requested layout. It also powers the
display and holds an idle inhibitor.

`deploy/package/session-supervisor.py` accepts only that single-output 1:1
Virtual-1 topology. The guard invalidates on topology changes; the supervisor
then re-prepares the target and reruns the output helper. Consequently plugging
JetKVM into minigpu may result in physical HDMI being disabled again as the
helper restores its Virtual-1-only layout. "Input pauses on topology change"
is true but incomplete: output topology can also be actively rewritten.

Desired hotplug behaviour needs an explicit physical-versus-virtual ownership
policy, validated output selection, preservation or intentional replacement of
other outputs, input generation invalidation and source/stream renegotiation.
It must not silently force physical HDMI off or claim automatic fallback that
does not exist. This belongs to
[WP10 display hotplug](../work-packages/WP10-display-hotplug.md), not a cable
experiment hidden inside this readiness check.

## Remaining acceptance

### Guarded-input preparation and real-hardware discoveries

Before upgrading the working `.5` output package, existing configuration and
package units were copied to `/var/tmp/dw-ek-pre-input.iQTwrc` (root-owned).
No LightDM/Xorg restart, mode change, login action or reboot was performed.

Read-only validation uncovered three concrete differences from Spark/VKMS:

- XRandR's output timestamp is negative (`-481873`) after long uptime. This is
  an identity value, not an invalid layout; signed values must be retained.
- i915's active DRM mode has an empty name. Its numeric mode timings describe
  1920×1080 correctly; checking the string `mode: "1920x1080"` rejects valid
  Intel scanout. Validation must use the actual hdisplay/vdisplay fields.
- The same `:0` server accepts both LightDM's root authority and the active
  user's authority. Discovery must distinguish alternate credentials for the
  same server from genuinely ambiguous display targets.

After the first two fixes, the explicit X11 guard `--check` passed on the real
KMS capture PID and HDMI-2 output. Automatic discovery still required the
third fix at this preparation checkpoint.

Prepared tests (not claimed passed until results below) are
`tests/integration/easternkingdoms-input-proof.py` and its `--browser` companion
`easternkingdoms-browser-input.mjs`. The browser path performs a real video
focus/acquire, three absolute moves, standalone Shift, and release. Both paths
observe only DonkeyWork-owned evdev devices plus independent XQueryPointer
coordinates and unchanged desktop focus. They restore the original pointer
through the same guarded CLI. They do not click the remote desktop, type text,
launch processes in the session, or use X11 as an alternate input injector.

### `.7` service-only upgrade: output passes, input descriptor defect found

Installed `.7` with `--profile kms --device /dev/dri/card1 --listen
192.168.69.17 --control x11 --replace-config --activate`. This enabled our
package target for boot and restarted only package services. LightDM stayed
PID 2217; capture became PID 1197136, web 1197137, supervisor 1197105, input
1197330. Automatic guard discovery and connector-correlated validation passed.
The browser still decoded 1920×1080 H.264 (15→47 frames, one dropped frame,
no page errors in this short smoke).

The independent input proof correctly **failed** before keyboard injection:
requested pointer `(200,200)` was emitted on the owned evdev node but the X11
pointer stayed at `(1042,27)`. Xorg adopted our device as relative-only despite
its mixed ABS_X/Y and REL_X/Y capabilities. Xorg logged:
`Discarding absolute event from relative device. Please file a bug`.
The proof preserved original focus and restored/verified original pointer;
no remote button or text was injected. Browser input acceptance was not run
against this known-broken descriptor. The fix belongs in our virtual device
capabilities, not a global Xorg transform or desktop configuration change.

Keyboard acceptance now also requires independent XQueryKeymap evidence of
Shift held and released, in addition to observing our evdev events. Browser
Shift is bounded to one second with key-up in `finally`; CLI Shift is 250 ms.

### `.8` actual CLI and browser input acceptance: PASS

Installed `0.1.0-lab.20260906.8-linux-amd64` from
`artifacts/releases/.build-0.1.0-lab.20260906.8.Vo6GYE/` with the same explicit
KMS/X11-control settings and service-only activation. The corrected virtual
pointer omits REL_X/Y while retaining ABS_X/Y and relative wheel axes. Xorg now
exposes `Abs X` and `Abs Y`; no global Xinput settings were changed.

Both commands exited zero:

```sh
sudo -n /usr/bin/python3 tests/integration/easternkingdoms-input-proof.py --allow-modifier-pointer-probe
sudo -n /usr/bin/python3 tests/integration/easternkingdoms-input-proof.py --allow-modifier-pointer-probe --browser
```

CLI requested/independently observed coordinates matched exactly:
`(200,200)`, `(960,540)`, `(1720,880)`. Its standalone Shift generated owned
evdev down/up and was independently held/released in the X server keymap.

The real Chromium viewer at a 1280×900 browser viewport acquired the existing
1920×1080 stream and control, sent the same three scaled mouse movements,
pressed/released standalone Shift, and explicitly released control. Each
movement was independently checked against our evdev node and XQueryPointer;
Shift was checked against evdev and XQueryKeymap. No remote mouse-button down
or non-Shift key down occurred, and there were no page errors. The first run
completed all assertions but exposed a test-harness stdin handle preventing
Node exit; closing that handle produced a clean second run. Both runs restored
the original pointer and retained desktop focus.

Final checks: LightDM PID 2217 and Xorg PID 2262 remained unchanged; Xorg was
still the sole DRM master. Physical HDMI-2 remained the sole 1920×1080 output.
Package capture PID 1210706, web 1210735, supervisor 1210724 and input 1210734
were active. Kernel warning-or-higher journal since 20:55:30 had no entries.
The package target is now enabled for boot; boot behaviour was **not tested**
because Easternkingdoms must not be rebooted. No logout, greeter handoff,
physical hotplug, remote desktop click/text entry, or remote process launch
was performed as part of this bounded existing-desktop acceptance.

Long-duration Intel performance/fidelity, cursor composition,
JetKVM coexistence under streaming, physical hotplug, arbitrary mode changes
and layout/seat transitions remain unproven. The root process still performs
capture/EGL conversion and software encoding; the network-facing Go process
drops privileges. Do not describe this as completed privilege separation or
an internet-hardened fleet release.
