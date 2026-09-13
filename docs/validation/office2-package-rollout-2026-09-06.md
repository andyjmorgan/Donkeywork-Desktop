# Office2 packaged VKMS rollout — 2026-09-06

Result: `http://192.168.69.30:8090` serves the real Rocky GDM Wayland greeter over1080p H.264. Browser display and reviewed greeter click/Escape were visually verified. No credentials entered; no login, logout, host reboot or reboot acceptance performed.

## Scope and baseline

- office2: Rocky10.0, kernel6.12.0-55.41.1.el10_0.x86_64, GDM47.0/Mutter47.5/GNOME Shell47.4; SELinux remains Enforcing.
- Physical AMD card0 had all DP/HDMI connectors disconnected. VKMS newly loaded as card1, CRTC38. No physical driver reload or display cable needed.
- Existing GDM greeter c1 was the only graphical session. Restarted GDM only after rechecking that fact; new active session c2, UID42. No authenticated user session interrupted.
- k3s PID1783885/start2026-08-18 03:39:13 IST unchanged.

## Installation

- Release `0.1.0-lab.20260906.5-linux-amd64`; archive SHA256 `a8d2ff4e4063c91d13a8d69e0c15f5adfca09b5d9fe5713c625fbeb3998d4618`. Outer and payload manifests verified by checksum and installer.
- Private upstream-built FFmpeg8.0.1/libx264 encoder installed at `/opt/donkeywork-desktop-codecs/ffmpeg-8.0.1/bin/ffmpeg`, SHA256 `850bec40438959b185c880b4c7096bf49fe9cb587ed816f52f2e3b5a9b80ead3`. Source archives/recipe retained in sibling `share/`. No RPM, repository, firewall or SELinux-policy changes.
- Installer: `--profile vkms --listen 192.168.69.30 --ffmpeg /opt/donkeywork-desktop-codecs/ffmpeg-8.0.1/bin/ffmpeg --configure-vkms`, first without activation. Creates persistent VKMS module configuration and scoped Mutter udev policy.
- Added root-owned0644 `/etc/modules-load.d/donkeywork-desktop-uinput.conf`; `vkms` and `uinput` loaded. GDM restarted only after headless/greeter check.
- First greeter selected1024x768. Started packaged `keep-console-output.py` as UID42 using actual GDM shell private D-Bus address, under package-owned transient unit `donkeywork-desktop-vkms-output.service`. It selected1920x1080 and held idle inhibition.
- Repeated installer with `--activate`, omitting `--configure-vkms`; doctor succeeded, supervisor adopted/replaced temporary helper. Package target enabled for boot.
- Root-private pre-change backup `/var/lib/donkeywork-office2-before.cNyj84` contains original modules-load/udev directories, vendor61-mutter rule, service baseline and session inventory. No previous Desktop service existed. Stop/disable package and restore only changed Desktop policy files for rollback; retain release/codec artifacts inert. Rollback not exercised.

## Actual evidence

- Browser960x640 viewport decoded1920x1080 video and displayed Rocky greeter. `/api/status`: `state=streaming`, H.264 Annex-B/libx264, `/dev/dri/card1`,1920x1080, nominal30FPS.
- Reviewed source-coordinate960,490 click selected visible localuser, empty password prompt displayed. Escape returned user list. Explicit Release input returned idle. c2 remained active Wayland greeter throughout, no JS errors.
- Visually inspected `/tmp/office2-display.png`, `/tmp/office2-click.png`, `/tmp/office2-escape.png`. Input screenshot UI sample30.0FPS/55.6KB/s; short sample, not a performance benchmark.
- Guard `/run/donkeywork-desktop-guard/state.json`: valid1920x1080, session c2 generation. Capture/input/web active, NRestarts=0. Parent received Ready checkpoint before office3 rollout.
- All four office nodes Ready before/after. API `/readyz` returned `ok`. Only same five pre-existing failed pods: three old node-debuggers and two kokoro-tts ErrImageNeverPull. No k3s/workload changes performed.

## Office1 persistence follow-up

Added root-owned0644 `donkeywork-desktop-vkms.conf` and `donkeywork-desktop-uinput.conf` under `/etc/modules-load.d` without touching its existing udev policy. Backup `/var/lib/donkeywork-office1-modules-before.mqF78w`. Existing package target active/c2; k3s PID1939266 and GDM PID772693 unchanged. No service restart or reboot. These files establish intended boot module loading, not verified reboot recovery.

## Remaining acceptance

Reboot recovery and authenticated greeter-to-user handoff remain untested on office2. No assertion of arbitrary compositor/desktop support or dedicated SELinux service confinement. GNOME-specific output management remains a real dependency of this VKMS profile.
