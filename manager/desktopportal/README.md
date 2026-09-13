# Existing-desktop portal agent

Fleet update: [autologin, UI separation and the Rocky blocker](../../docs/validation/fleet-autologin-2026-09-12.md).
Autostart is now installed for the six scoped desktop hosts; attic excluded.
State lives in `~/.local/state/dwdesktop-console`. Minigpu uses Wayland portals;
Spark/EK use their authenticated X11 session (FFmpeg x11grab + XTest). Rocky's
initial portal connection is not acceptance: Office1 reproduced a Mesa/Mutter
crash on app launch. Rocky agents are stopped pending that fix. The historical
Minigpu subsection below predates this rollout.

## Current Minigpu integration — 2026-09-12

The earlier preflight below is historical. Capture, H.264 browser relay, portal
keyboard/pointer input and browser reconnect now work on Minigpu's existing
GNOME46 Wayland desktop. The manager lists **Existing Wayland console** alongside
the independently managed desktops. Open it at
https://192.168.10.11:30443/?device=fd6b6a59-40fc-41ac-84ef-b498ca8d6880&managed .

`serve --socket PATH --restore-file PATH` holds the restored portal session and
provides a private Unix HTTP adapter. Run under the user unit
`app-dev.donkeywork.Desktop.service`. `deploy/managed/broker.py` optionally reads
its `console-socket` config and merges live console inventory. The device agent
and manager media relay require no different transport or new enrollment.

The scoped permission bootstrap and its limits are documented in
[the provisioning validation](../../docs/validation/minigpu-portal-provisioning-2026-09-12.md).
Tokens are0600 in a0700 user-owned directory. The installed pilot unit is not
enabled for autostart; current-session access is tested, fresh-login startup is
not. No automatic permission re-grant occurs on failure.

One viewer, one monitor. Input uses the public RemoteDesktop Notify APIs, not
uinput. Exact generation/sequence and a one-second lease guard input; disconnect,
reset and expiry release held keys/buttons. Browser scaling maps to the portal's
logical size; the encoder scales its raster to that same size. This pilot does
not implement clipboard, physical display resize, greeter control or logout/login
handoff. End/resize operations are rejected and hidden for console entries.

Dependencies: GStreamer PipeWire, video conversion/scaling/rate, `x264enc`, RTP
and UDP plugins. Minigpu needed `gstreamer1.0-plugins-ugly` plus its four libraries;
installed without upgrading packages or restarting system services. Damage-driven
PipeWire needs a100ms keepalive before videorate to produce the idle stream. Video
is encoded as H.264 on-device, then relayed unchanged through the existing gateway.

Validation: `tests/integration/manager-wayland-console.mjs`; evidence under
`artifacts/manager-wayland-console/`. See its report and visually reviewed images.

## Historical first slice (superseded)

Source: `desktopportal/` and `cmd/session-agent/` in the manager Go module.
Independent of the current managed-Xorg broker. No manager capability is
advertised yet. Uses godbus to call standard desktop portals; no GNOME-private
API, root access, driver capture or injected greeter input.

Build: `go build -o dwdesktop-session-agent ./cmd/session-agent`.
Run as the actual logged-in desktop user:

```
dwdesktop-session-agent probe
dwdesktop-session-agent share --snapshot /private/new-console.png --timeout 2m
```

`probe` does not request sharing. `share` requests one monitor plus keyboard and
pointer, waits for desktop-user consent, obtains the restricted PipeWire FD and
passes it into GStreamer's pipewiresrc to capture one PNG. Control permission is
reported, but input injection is not wired in this slice. Output is exclusive
0600; no overwriting. Session/request cleanup occurs on success, denial, timeout
or interruption. No persistence tokens are requested or saved.

Requires the current desktop session bus and installed portal backend, PipeWire,
gst-launch-1.0, pipewiresrc, videoconvert and pngenc. The snapshot is a diagnostic,
not a proposed PNG-loop video transport. H.264 streaming and local registration
with the device service are next; clipboard/resize/unattended restore are not
claimed. The current portal's coordinate size may differ from captured pixels;
input integration must explicitly account for that.

## Minigpu preflight, 2026-09-10

Physical session 214 is active Wayland under localuser. RemoteDesktop version2,
ScreenCast version5, input/source/cursor masks all7. All snapshot GStreamer
plugins exist; x264enc does not. Use the existing FFmpeg libx264 integration or
explicitly package the missing encoder for the streaming step, not a silent
PNG-loop fallback.

Binary deployed as `/tmp/dwdesktop-session-agent-preview`. Scoped transient
user unit `dwdesktop-portal-check` waits up to9minutes for approval, with a
10minute systemd maximum. Output path for this attempt is
`/tmp/dwdesktop-portal-check.pe7fcy/console.png`. Progress reached
`awaiting_desktop_consent`; capture has NOT yet been validated. No autostart,
desktop packages, manager deployment or physical-session settings changed.
Managed desktop and managed broker remain active.

Tests: real isolated D-Bus daemon checks an early response emitted before the
method returns, timeout cancellation and Request.Close. Result parsing rejects
denial/cancellation/malformed responses. `go test -race ./desktopportal` passes.
Live consented screenshot/control/streaming remain separate acceptance checks.
