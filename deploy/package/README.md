# Host-package runtime profiles

Original package integration around the validated pilots. No existing `dwconsole-*`
units or live configuration are modified by these sources. These units are not
an assertion of fleet-wide acceptance or a production SELinux policy.

The installer places binaries under `/opt/donkeywork-desktop/current/bin`, these
three scripts under `current/libexec`, web assets under `current/share/web`, and
units from `systemd/` under `/etc/systemd/system`. Also package the existing
`deploy/vkms/keep-console-output.py` and `input-target-guard.py` into `libexec`.
`current` is the installer-owned release symlink; the runner never updates it.

## Configuration

`/etc/donkeywork-desktop/console.env` is root-owned and not writable by other
users, within a root-owned non-writable directory. It is strictly parsed as data;
it is never sourced by a shell. No quoting, expansion, inline comments or extra keys.

```text
PROFILE=vkms
LISTEN_IP=192.168.69.21
DRM_DEVICE=auto
FFMPEG=/usr/bin/ffmpeg
WIDTH=1920
HEIGHT=1080
BIN_DIR=/opt/donkeywork-desktop/current/bin
ASSETS=/opt/donkeywork-desktop/current/share/web
LIBEXEC_DIR=/opt/donkeywork-desktop/current/libexec
```

Only PROFILE and LISTEN_IP are mandatory. Other lines above show defaults.
LISTEN_IP must be an explicit RFC1918 IPv4 address, not a wildcard/public address.
The three installation paths are fixed to this layout. Browser HTTP is port 8090;
WebRTC host ICE is UDP 8091. No authentication or public exposure is provided.

FFMPEG may select a private installation (for example Rocky's existing codec),
but must be an absolute root-owned, non-writable-by-others regular executable.
It is passed as a process argument, never evaluated by a shell. The runtime does
not install FFmpeg or promise that an arbitrary binary includes libx264.

| Profile | Output | Input and adoption requirements |
| --- | --- | --- |
| `vkms` | Existing monitorless GNOME Wayland console, fixed 1080p | Guarded input enabled only after session/output preparation. Exactly one VKMS card for `auto`; explicit card must also be VKMS. |
| `kms` | Existing DRM console on explicitly configured `/dev/dri/cardN` | View-only. No monitor creation, desktop installation or input claims. |
| `x11` | Spark's existing GNOME/GDM X11 seat | View-only. Uses the tested session-following X11 launcher copy, with package paths. Not a generic arbitrary-display X11 configuration. |

The runtime does **not** load VKMS, change Mutter rules, restart GDM, change boot
configuration, configure firewalls or install desktop environments. Those require
the installer's explicit preparation workflow and live acceptance. VKMS mode and
idle inhibition are applied by the session supervisor after an adopted output
exists; no new login session is created. Its session helpers are copied from our
pilot implementation, not imported from RustDesk.

For X11, omitted/auto DRM_DEVICE resolves to `/dev/dri/card0` for the Spark
baseline; an explicit configured card is passed through to the daemon. The
daemon still requires a valid DRM node even though capture uses X11.

## Units and runtime ownership

Enable/start `donkeywork-desktop.target` only after explicit activation approval.
Its capture, broker, web and session-supervisor services use this package prefix.
Every service is `PartOf` the target so target restart applies an upgrade.
Non-VKMS supervisor exits successfully without starting input.

VKMS supervisor starts the static input service only after a fresh guard, and
owns transient `donkeywork-desktop-vkms-output.service` and
`donkeywork-desktop-input-guard.service`. These also belong to the package target.
Input and browser persist across supported session changes; output/guard are
rebound through the pilot supervisor. Capture reconnect uses the FD broker.

Private root-only runtime directories are
`/run/donkeywork-desktop-{capture,broker,input,guard}`. They do not overlap pilots.
The root launcher opens the required sockets then drops to UID/GID 65534 before
executing the network-facing Go bridge. The broker only reconnects the configured
capture socket. Capture/input remain privileged prototypes; units are not a
complete privilege split or proof of internet hardening.

## Host prerequisites

- systemd/logind, Python 3.10+ for runner/pilot APIs, Bash,
  executable bundled binaries, FFmpeg with `libx264`, required shared libraries.
- VKMS: loaded/adopted VKMS, GNOME/Mutter Wayland, python3-dbus, PyGObject/GLib,
  readable `/sys/kernel/debug/dri/N/state`, `/dev/uinput`, existing graphical seat.
  The software-card number is discovered, not assumed to be card2.
- KMS: selected supported DRM driver, active captureable output, DRM/EGL libraries.
- X11: the Spark GNOME/GDM layout, loginctl, getent/id, pgrep, runuser, xrandr,
  and the current account's `/run/user/UID/gdm/Xauthority`.
- Root ownership/non-writable executable files and root-private sockets.
- SELinux remains enforcing where configured: no disablement or generic allow
  policy is installed. Rocky capture, codec packaging and policy compatibility
  require independent validation; these files do not claim it is already proven.

Local checks: `python3 -m unittest discover -s deploy/package/tests -v`,
`python3 -m py_compile deploy/package/*.py`, and
`bash -n deploy/package/start-console-x11.sh`.
