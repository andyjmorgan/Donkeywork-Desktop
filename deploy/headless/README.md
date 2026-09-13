# Dedicated-user virtual desktop POC

These files start a **brokered virtual desktop**, not Spark's physical Wayland console. Native capture/input/RandR resize is tested against Xorg's dummy driver. Physical-console capture, GPU readback and hardware display-mode switching remain unproven.

The root integrator owns host installation. Component preparation has not started any service or changed a display.

## Installation inputs

- Dedicated locked-password account `dwdesktop`, primary group `dwdesktop`, home `/var/lib/dwdesktop`, no sudo membership. A service does not need an interactive login; a nologin shell is appropriate. Root can run a bounded CLI command with `runuser -u dwdesktop -- ...` without enabling user login.
- Ubuntu packages: `xserver-xorg-core`, `xserver-xorg-video-dummy`, `xauth`, `x11-utils`, `x11-xserver-utils`, `openbox`, `dbus-daemon` (provides dbus-run-session on Ubuntu), `python3`, `python3-tk`, `openssl`, `fonts-dejavu-core`, `coreutils`.
- Copy this directory to `/opt/donkeywork-desktop/headless`, root-owned and not user-writable, including the otherwise empty `xorg.conf.d` directory.
- Place native binaries at `/opt/donkeywork-desktop/bin/dwdesktop-core` and `/opt/donkeywork-desktop/bin/dwdesktop`.
- Supply `/etc/dwdesktop/headless.json`, owned by `dwdesktop` with mode `0600`, selecting X11 display `:99`, socket `/run/dwdesktop/cli.sock` and an explicit policy for the actual dedicated UID. The root integrator owns this host-specific configuration.
- Install the four unit files into `/etc/systemd/system`; reload systemd. Start `dwdesktop-headless.target` only after reviewing the display number and configuration. Enabling at boot is a separate operational choice.
- `/tmp/.X11-unix` should already be the standard root-owned sticky X socket directory. Inspect any existing `:99` server/socket/lock; these scripts never delete a competing display's files.

Direct `/usr/lib/xorg/Xorg` bypasses Xorg.wrap's console-user gate. It runs as the dedicated user, with a named non-seat0 seat, auto device/GPU discovery disabled, virtual devices and no privilege escalation. `PrivateDevices=yes` also removes access to physical GPUs/input nodes. Whether this Xorg build/dummy driver supports the rootless non-console configuration is a runtime gate. Failure must be investigated; **do not silently switch to root Xorg or alter GDM, seat0, the active console, or Xwrapper permissions**.

Xorg has TCP disabled and uses a new private MIT-MAGIC-COOKIE-1 authority each start. The cookie is passed to xauth on stdin, never command arguments or logs. `/run/dwdesktop` and `/var/lib/dwdesktop` are service-owned `0700`; the daemon socket is service-owned `0600`. Host configuration should permit only this UID to view/control/resize. No terminal permission is advertised until that backend is integrated.

The system services create the desktop without a greeter, autologin or console login. A private session bus comes from `dbus-run-session`; systemd supervises Xorg, Openbox/test app and the daemon. Restart the **target** to restart all three coherently. A core-only restart preserves Xorg and desktop applications. A whole-target restart creates a fresh desktop and expires daemon sessions. Unexpected stale daemon sockets fail rather than being deleted automatically.

## Local proof sequence

Root's runner invokes the CLI as `dwdesktop` with `--socket /run/dwdesktop/cli.sock --server-uid <actual dwdesktop UID>`. Artifact files belong under `/var/lib/dwdesktop`; use fresh filenames per capture. Retrieve the real output ID/mode inventory from `describe`, not a guessed `DUMMY0`/numeric ID. Omit `--include-cursor` until cursor composition is implemented.

The Tk application is a fullscreen native X11 window. Its input-proof button occupies x=64..383, y=112..175; click near (200,144), then observe `Button activations` advance in a new screenshot. Text entry occupies x=64..663, y=274..321 and keeps content only in memory. Coloured corners/grid and a live window-size label aid native-raster validation. Window-manager readiness/geometry must still be checked in the first actual screenshot.

Observe → click → observe and the 3840×2160 →1920×1080 →3840×2160 sequence must be performed against the same daemon session. Capture independent `xrandr --query` evidence under this same display/user/authority; these are actual virtual-output mode changes. Capture application state before/after to show session continuity. PTY continuity remains an additional gate until the terminal component is integrated.

## Login/PAM boundary

This proof uses systemd's fixed `User=dwdesktop` service identity and a private D-Bus session. It does **not** implement `pam_authenticate`, `pam_open_session`, a logind user login session, Keycloak-to-OS identity mapping, password entry, or a reusable credentials broker. Those are later session-bootstrap work with distinct lifecycle and privilege decisions. Reusing a display backend does not make service-account startup equivalent to PAM login or physical-console attachment.

## Local validation

`bash -n` checks the three shell scripts. Python AST parsing checks the test app without starting Tk. `systemd-analyze verify` checks unit syntax, but on a build machine missing Xorg or installed binaries it will report those absent executables. Configuration syntax/modes and rootless startup still require the authorized host run. No benchmark or live-X11 claim follows from these static checks.
