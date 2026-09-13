# Fleet console autologin and UI separation

## Scope and outcome

Andrew requested separate terminal-services/console UI, localuser autologin and
automatic in-desktop agent startup. Attic hosts were explicitly excluded because
they have no desktop. No passwords were changed or written into configuration.

The manager now separates **Console sessions** from **Terminal services**.
Console pages have no managed-desktop creation/destruction controls. Terminal
services retain independent desktop create/reconnect/end. Navigation records
`view=console` or `view=terminal`, including the return link from a viewer.

| Host | Autologin | Agent startup | Acceptance |
|---|---|---|---|
| Minigpu .21 | GDM configured | XDG desktop autostart installed | Wayland video/input/reconnect verified after explicitly unlocking the existing desktop and starting the agent; lock recovery not solved |
| Easternkingdoms .17 | Existing LightDM config retained | Autostart executed after LightDM restart | Existing X11 desktop video, control and browser app interaction passed |
| Spark .28 | GDM configured and exercised | Autostart executed after GDM restart | Existing X11 desktop video, control and browser app interaction passed |
| Office1 .27 | GDM configured and exercised | Autostart executed, then agent stopped after failure | Portal restores and video/control work initially; opening Calculator reproduced GNOME Shell SIGSEGV |
| Office2 .30 | GDM configured and exercised | Autostart executed, then agent stopped pending Rocky fix | Not accepted: browser control test failed |
| Office3 .19 | GDM configured and exercised | Autostart executed, then agent stopped pending Rocky fix | Not accepted: browser video test timed out |

Do not describe this as a successful Rocky rollout. Autologin removes the greeter
transition but does not repair Rocky's compositor/rendering failure. The Rocky
device connections remain enrolled/Online; stopped console adapters no longer
advertise ready desktops. Autostart configuration remains installed, so a future
login will attempt to start them again; there is no restart loop or automatic
permission re-grant when an existing token is invalid.

## Rocky failure evidence

Office1 GNOME47.4/Mutter47.5/Mesa24.2.8: GNOME Shell core dumps at12:27:40 and
12:30:24 IST on September12. Both occurred during browser-driven application
testing, not host reboots. Core PIDs2583485 and2612705 are retained on Office1.
The stack includes llvmpipe fences, Mesa ReadPixels, Cogl framebuffer readback,
and `meta_screen_cast_monitor_stream_src_record_to_buffer`.

The old root console pilot was still running. On Office1 only, disabled
`donkeywork-desktop.target` and stopped its old capture/input/broker/supervisor/
web helpers. A fresh GDM autologin with those old services absent still reproduced
the crash. Thus removing the old tap did not resolve it; do not attribute the
failure solely to the legacy services. The display's existing VKMS setup was not
removed or retuned. No Mesa/kernel upgrade, rendering flags or driver replacement
was attempted. Those are the next diagnostic work, not claimed fixes.

## Installed components and security

- GDM: `AutomaticLoginEnable=true`, `AutomaticLogin=localuser`, preserving all
  other configuration. Backups beside each config end in
  `.pre-dwdesktop-autologin-20260912`. EK already used LightDM autologin into
  `ubuntu-xorg`; its configuration was unchanged.
- Autologin permits physical access as localuser. It does not unlock the user's
  encrypted login keyring; GDM logged that expected limitation on Spark. No
  password embedded in unit files, desktop entries, logs or the repository.
- User binary `~/.local/lib/dwdesktop/session-agent`; autostart entry
  `~/.config/autostart/dev.donkeywork.Desktop.desktop`; launcher
  `~/.local/lib/dwdesktop/desktop-agent-start`.
- Launcher refuses managed desktops' private runtime directories. It imports
  the real desktop's DISPLAY/XAUTHORITY/session environment then starts the
  stable app-identity user unit. The unit is PartOf graphical-session.target.
- Persistent private state moved to `~/.local/state/dwdesktop-console/`, mode0700,
  with token0600 and private console socket0600. Initial provisioning is an
  explicit installed `--provision-if-missing` opt-in; supported GNOME backend
  versions46.2 and47.2, exactly one active monitor. Existing invalid/revoked
  tokens are not silently regenerated. Rocky47.2 restore format was inspected
  before use and succeeded live without a consent click.
- On Ubuntu, the existing local managed broker merges console inventory through
  its operator-configured private socket. Installed/enabled a persistent
  `dwdesktop-managed-web.service` with per-host address config so a transient
  pilot unit is no longer the only boot path. Existing managed desktops were
  not explicitly ended or recreated; persistence across display-manager restart
  is not claimed by these console tests.
- Rocky is console-only: a second socket `/run/dwdesktop-session/broker.sock`
  is0660 inside a2750 localuser:dwdesktop-agent directory, provisioned by
  tmpfiles. It exposes no terminal-services creation backend.
- Enrolled Office1, Office2 and Office3 in the existing mTLS manager. Enrollment
  used private code stdin; no codes retained in logs. Device IDs:
  Office1 `383f3fed-5e23-4270-bfa1-8c602362b31d`, Office2
  `9e001ed7-53b6-4eda-8b9f-1ae13f90111f`, Office3
  `f9361977-7f4b-4a29-9db0-86af87936ab8`.

## Media compatibility

The PipeWire path remains in use for Wayland. GNOME X11 portals restored
permission but did not produce frames on EK/Spark; capture now uses
FFmpeg x11grab in the logged-in user's own DISPLAY/Xauthority. The intermediate
ximagesrc trial did not give acceptable composited output. Visual review showed
that portal input acknowledgements on X11 did not actually operate the desktop;
X11 input therefore uses session-authenticated XTest via jezek/xgb1.1.1. The X11
path no longer requests or depends on portal grants at all; Xauthority and a
verified XTest extension authorize that user's own display. A missing portal
stream field also exposed an unchecked variant decode; absent fields now return
an explicit error instead of panicking in the Wayland path.
Wayland input remains the portal path. No greeter capture or privileged fallback
was added. Do not treat an acquired lease alone as proof of visible input.

Ubuntu hosts without GStreamer x264 use their installed FFmpeg/libx264. Rocky's
existing minimal FFmpeg lacks an RTP muxer, so H.264 access units are packetized
using Pion and passed into the unchanged relay. Office2/3 were missing the codec
binary; installed the Office1 build (libc/libm dependencies only) and its existing
source/provenance bundle under `/opt/donkeywork-desktop`. No external package
repository was added. Codec replacement alone is not a Rocky stability fix.

## Validation and outstanding work

### Follow-up transport and input validation

EK initially received encoded bytes with zero decoded frames. During a repeat
test, the host UDP receive-buffer error counter rose from 13,303 to 13,824.
The FFmpeg packetizer had burst entire keyframes through a localhost UDP socket.
It now writes RTP packets directly to the WebRTC track, with synchronous
backpressure rather than that lossy intermediate hop. The GStreamer path is
unchanged. A regression test delivers a 512 KiB access unit and verifies packet
sequence continuity, MTU and frame timestamps. Go race tests and vet passed.
Only the EK and Spark agent binaries were replaced in this follow-up; recoverable
`session-agent.pre-direct-rtp` copies remain beside each installed binary.

Actual browser screenshots now show Calculator with `123+456 = 579` on both
EK and Spark. Both passed viewer reload/reconnect, and EK's agent and managed
broker remained active after browser disconnect. Earlier input acknowledgements
and empty Calculator screenshots were not sufficient evidence.

Minigpu's follow-up initially found the agent stopped. Restart failed because
Mutter reported `Session creation inhibited`; loginctl confirmed the existing
Wayland desktop was locked. This is not evidence of a revoked restore token.
An explicit one-time loginctl unlock was used for the follow-up test; idle-lock
policy was not disabled, and the existing grant was not overwritten. Automatic
recovery after locking remains separate from initial autologin/startup acceptance.
The generic fleet script failed its input-state check; the dedicated
`manager-wayland-console.mjs` then passed video, reconnect, control lease expiry
and out-of-order rejection. Its screenshot was inspected and shows the typed
calculation result 579. An Ubuntu crash-report dialog is also visible; files in
`/var/crash` predate this test (latest is light-locker, September 9). No new
compositor crash was established. The agent remained active after disconnect.

`tests/integration/fleet-console-smoke.mjs` navigates the separated UI, decodes
video, acquires input and launches Calculator using browser events. Artifacts
are under `artifacts/fleet-portal/`; individual runs overwrite report.json, so
the per-host images and this acceptance table are the record across runs.
Go race tests and focused vet pass. Browser startup waits for both live UI and
video, then focuses the tab before acquiring input.

Host boots were not exercised: only GDM/LightDM and our own services restarted.
No cluster service or host restarted. Attic desktop configuration was untouched.
Fresh-login permission persistence, one-viewer scope, clipboard/resize limitations
remain as documented in the portal README. Rocky application stability is a
real blocker, not a completed milestone or a problem hidden by autologin.

Rollback: restore each display-manager backup (EK unchanged), move aside the
desktop autostart entry, and stop our portal unit. Broker configuration and
runtime socket files can be restored independently. The Office1 legacy target
can be re-enabled if deliberately returning to the old pilot; it is not a fix
for the reproduced compositor crash. Preserve device enrollment and managed
desktop data during rollback.
