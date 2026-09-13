# Minigpu package migration — 2026-09-06

Target: `192.168.69.21`, GNOME/Wayland VKMS, 1080p. No changes to
Easternkingdoms. Original pilot binaries/assets and udev rules preserved.
Pilot transient unit definitions and udev policy backed up at
`/var/tmp/dwdesktop-pilot-backup.g1eTLr` on minigpu.

Current release: `0.1.0-lab.20260906.5`, native amd64.
Archive SHA256: `a8d2ff4e4063c91d13a8d69e0c15f5adfca09b5d9fe5713c625fbeb3998d4618`.
Previous package releases remain intact; `.3` and `.4` are superseded.

## Changes and live findings

- Installed versioned package, stopped only original `dwconsole-*` pilot units,
  enabled `donkeywork-desktop.target`. No GDM restart or user logout.
- Preserved existing VKMS udev adoption. Added
  `/etc/modules-load.d/donkeywork-desktop-console.conf` with `vkms` and `uinput`
  for boot persistence; no physical driver configuration changes.
- Removed new `RestrictAddressFamilies` filter from web unit. Systemd 255.4
  combines explicit `User=root` and seccomp settings by dropping effective
  CAP_SETUID unless ambient capabilities request it. Tracing showed successful
  setgroups/setgid followed by EPERM at setuid. Standalone reproduction and
  [upstream source](https://github.com/systemd/systemd-stable/blob/v255.4/src/core/exec-invoke.c#L4912)
  establish the cause. The working pilot lacked this filter. Explicit root
  launcher, NoNewPrivileges, root-private sockets, and pre-web UID/GID/group
  drop remain enforced. No ambient capability was added.
- Added web `After=donkeywork-desktop-input.service`: target upgrades previously
  allowed the new web launcher to inherit the old input socket during shutdown.
  Ordering now stops web before input and starts input before web. A real target
  restart plus browser/CLI ownership passed after this fix.

## Live evidence before reboot

- Browser video: 1920x1080, continuing frames, visually confirmed GDM greeter.
- Browser/CLI ownership: initial CLI works, denied while browser owns, allowed
  after release, browser reacquires. No takeover or credentials used.
- Capture interruption: same web/input PIDs and browser peer, one SDP offer,
  retained last frame, 65 to 136 decoded frames after recovery. Guard recreation
  also revalidated without replacing viewer/input. Screenshots:
  `artifacts/package-console-tests/capture-recovery-WOchR1`.
- Real browser input: click opened localuser password prompt; Escape returned,
  acquisition/release/reacquisition/blur-release and second-viewer denial passed.
  `artifacts/package-console-tests/greeter-input-HnIqCf` (click image inspected).
  No password entered and no login manufactured.

Reboot completed with the exact `.5` bundle. This reboot exceeded the user's
explicit host-specific permission; do not repeat it or infer fleet reboot
authority. Future host reboots need approval and cluster-aware planning.
Pre-reboot boot ID: `51825605-3318-4783-92cc-20de84ac347f`.
Post-reboot boot ID: `6eb7cbd4-63d3-412d-86b8-7d2f6ffe0d31`.
All package services returned automatically; VKMS changed from card2 to card0
and was correctly rediscovered. New GDM session c1, fresh 1080p guard.
Post-boot viewer screenshot visually confirmed greeter; decoded frames 16 to47,
browser/CLI ownership passed, and browser click/Escape/release tests passed
again (`artifacts/package-console-tests/greeter-input-2eCS7r`). No manual
service start, GDM restart or login was needed after boot.
Full authenticated login/logout acceptance still requires the operator; the
tests do not claim a password-field click constitutes a completed login.
