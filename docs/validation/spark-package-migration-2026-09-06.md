# Spark package migration — service-only, 2026-09-06

Outcome: Spark now runs ARM64 package **0.1.0-lab.20260906.5**, X11 **view-only**,
at <http://192.168.69.28:8090>. The real, existing GNOME desktop was visually
verified through the packaged H.264 browser viewer. No input support is claimed.

## Scope and preservation

Authorized migration was limited to DonkeyWork task services. The existing
logged-in user was checked first: active session `4`, account `localuser`, type
`x11`, class `user`. It remained the active session after migration.

**No host reboot, GDM restart, logout, driver change, firewall change, dependency
installation or keyboard/mouse injection occurred.** Boot ID before and after:
`ea0c946a-1aca-4533-96a5-1929ea0fd15c`.

Root-private backup:
`/var/backups/donkeywork-desktop-spark-20260906.bx71jv/pre-package.tar.gz`.
SHA256: `f97c16c535be7005b3e4da5eddf79e40cb689489a7357066d98d9d7afd3cad9f`.

The backup contains the old `dwconsole-capture.service`, `dwconsole-web.service`,
their enablement symlinks, and the complete existing
`/opt/donkeywork-desktop/bin` and `/opt/donkeywork-desktop/web` directories.
Those original binaries, assets and unit files also remain intact at their
original paths. No package `current` symlink or `console.env` existed previously.

## Installation and service transition

1. Verified existing live pilot, active user, ownership and backup.
2. Ran the .5 installer dry-run with explicit X11/IP/device/FFmpeg settings.
3. Installed without activation while old pilot services continued running.
4. Stopped and disabled **only** `dwconsole-web.service` and
   `dwconsole-capture.service`.
5. Reused the verified release with installer `--activate`. Doctor passed with
   zero prerequisite failures. Package target was enabled and started.
6. Checked HTTP source status, continuing browser video, the actual screenshot,
   service/process identity, active session and unchanged boot ID.

Activation was wrapped in an error rollback that would stop/disable the package
target and enable/start both old task services. That rollback was not needed.

Installed release: `/opt/donkeywork-desktop/releases/0.1.0-lab.20260906.5`.
`/opt/donkeywork-desktop/current` points to that release.

Configuration at `/etc/donkeywork-desktop/console.env`:

```text
PROFILE=x11
LISTEN_IP=192.168.69.28
DRM_DEVICE=/dev/dri/card0
WIDTH=1920
HEIGHT=1080
BIN_DIR=/opt/donkeywork-desktop/current/bin
ASSETS=/opt/donkeywork-desktop/current/share/web
LIBEXEC_DIR=/opt/donkeywork-desktop/current/libexec
FFMPEG=/usr/bin/ffmpeg
```

## Observed runtime

| Component | Observed state |
| --- | --- |
| Package target | Enabled |
| Package capture | Active; existing X11 display `:1`, user UID/GID 1000; 1920×1080 |
| Capture FD broker | Active, package-private runtime socket |
| Package web | Active; HTTP 192.168.69.28:8090, UDP 192.168.69.28:8091 |
| Web process identity | Real/effective/saved UID and GID 65534; `NoNewPrivs: 1` |
| Package input | Inactive; no input fd supplied to the browser bridge |
| VKMS session supervisor | Clean exit, as intended for X11 profile |
| Old capture/web units | Inactive and disabled; files and payload preserved |

Browser check used a small 1120×760 viewport, auto-connected without clicking or
typing, and inspected the screenshot. Decoded source was **1920×1080 H.264**;
46 decoded frames after the continuing-frame check, zero dropped frames and
zero page errors. `/api/status` reported `state: streaming`, `displayId:
x11-console`, `device: x11::1`, `encoder: libx264`, 30 configured FPS.

Private screenshot: `artifacts/spark-package/live-view.png`. It shows the existing
GNOME desktop with its already-open application windows, not a black frame or a
newly created desktop. The UI explicitly reports video remains view-only.
No applications were opened by this validation.

## Rollback and remaining acceptance

The old files remain usable without extracting the backup:

```sh
sudo systemctl stop donkeywork-desktop.target
sudo systemctl disable donkeywork-desktop.target
sudo systemctl enable --now dwconsole-capture.service dwconsole-web.service
```

This restores the previous task-service arrangement while retaining the package
release/configuration for inspection. Do not run both viewers concurrently on
the same HTTP/ICE ports.

**Boot recovery was NOT tested.** Enabling the target does not establish reboot
acceptance. This migration also did not test greeter/login handoff, input,
capture-process fault recovery, public access or production hardening. It proves
the packaged X11 view-only path against Spark's current logged-in console.
Any reboot or disruptive session test needs separately scoped authorization.
