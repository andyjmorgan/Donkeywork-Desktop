# Lab retirement — September 13, 2026

Andrew ended the experiment and requested removal of the deployed software and
commit of the project work. This record supersedes earlier fleet availability
claims. Passing individual tests did not deliver reliable unattended console access.

## Host cleanup completed

Targets: Easternkingdoms 192.168.69.17, Minigpu .21, Spark .28, Office1 .27,
Office2 .30 and Office3 .19. Attic machines were not desktop targets.

- Stopped managed desktop sessions, the user console adapters and local brokers.
- Revoked each host's app-scoped desktop-sharing restore grant successfully.
- Stopped and removed enrollment services and the old root console pilot,
  including transient capture/input helpers and Spark's headless pilot units.
- Removed deployed binaries, service definitions, autostart entries and project
  configuration from their active paths. Files were moved into root-only recovery
  directories rather than irreversibly deleted.
- Removed project-specific udev, modules-load and tmpfiles configuration.
- Restored the saved GDM configuration on Minigpu, Spark and all three Rocky
  nodes. The only differences were the two autologin lines and a blank line.
- Left EK's pre-existing LightDM autologin unchanged.
- Reloaded systemd definitions and udev rules, without triggering devices.

The cleanup script is `deploy/remove-lab-pilot.sh`. It selects only project unit
prefixes and paths; the unrelated `donkeywork-device-client.service` was inspected
and explicitly left alone. Two transient units disappeared while stopping their
parent services; cleanup was corrected to tolerate an already-stopped unit.

Verification across all six hosts found no matching installed system units,
loaded project units or running project workers. All four office k3s nodes were
Ready after cleanup. No host, cluster service or display manager was restarted.

## Preserved deliberately

- User desktop files, managed-session state and existing host desktops.
- Shared distro packages such as GNOME, Xorg, FFmpeg and GStreamer. These were
  not purged without establishing that no other application depends on them.
- Already-loaded graphics modules. Their project autoload rules are removed,
  but no module was forcibly unloaded from a running compositor. Restored GDM
  policy takes effect at the next normal display-manager start.
- Inert service accounts, prior migration backups and historical test state.
- Source, documentation and tests in this repository; generated artifacts,
  dependencies and local credentials are excluded from the commit.

Recoverable removed installations are under root-owned mode-0700 directories
matching `/var/lib/dwdesktop-removal-20260913.*` on each host. These directories
include private device state and must not be published or committed. Restoring
them would require an explicit decision to reinstall; they do not auto-start.

The attic manager and PostgreSQL workloads were not removed by the host cleanup.
Their shutdown was raised separately with Andrew; database/PVC deletion is not
authorized by this cleanup. No stateful cluster resource was deleted.

## Why the rollout was rejected

Console capture did not converge on a reliable unattended lifecycle across the
fleet. On Rocky, Office1 reproduced a GNOME Shell crash while launching an app,
with a Mutter screencast/framebuffer readback/Mesa llvmpipe stack. The same root
cause was not proven on Office2 and Office3. Autologin did not fix this failure.

On Minigpu, locking ended or prevented the Wayland sharing session. Mutter refused
creation while locked. Explicit unlock and agent restart restored capture, but
automatic lock/unlock recovery was not implemented. Autologin and idle locking
were wrongly treated as separate delivery concerns rather than one unattended
access requirement.

On X11, localhost UDP keyframe loss, worker cancellation and stale-socket bugs
were fixed. Final Spark and EK tests showed browser-driven Calculator input and
reconnect. These narrow successes did not establish fleet stability.

Several early acceptance claims relied on input acknowledgements or decoded
frames rather than visible application behavior and lifecycle coverage. The
latest per-host evidence corrected those claims. This archive is experimental
work, not a supported or production-ready remote desktop product.

## Archive checks

Before committing: 106 contract tests passed, all manager Go package tests passed,
73 console UI tests passed, and the console UI TypeScript/Vite build passed.
These are local archive checks, not renewed fleet acceptance. The cleanup script
passes `bash -n`. Known credential/private-key patterns were checked without
printing values; `authorized_tokens.json` is explicitly excluded. Generated
builds and dependencies remain ignored. The old blanket `bin` ignore was narrowed
to include Rust's actual `src/bin` source files in the archive.
