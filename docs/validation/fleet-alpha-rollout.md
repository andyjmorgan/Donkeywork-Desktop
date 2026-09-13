# Alpha fleet rollout ledger

Goal: every current office-cluster machine, Easternkingdoms, minigpu and Spark
has an installed alpha package with verified remote display **and input**.
View-only is incomplete. Current cluster inventory is authoritative, not the
historical readme listing retired office4.

| Machine | Address | Capture | Current evidence / remaining work |
| --- | --- | --- | --- |
| office1 | 192.168.69.27 | VKMS | .8 installed; real browser greeter click/Escape/release verified |
| office2 | 192.168.69.30 | VKMS | .8 installed; real browser greeter click/Escape/release verified |
| office3 | 192.168.69.19 | VKMS | .8 installed; real browser greeter click/Escape/release verified |
| office-spark-1 / Spark | 192.168.69.28 | X11 | .8 guarded control: browser pointer positions and Shift verified through X11 queries |
| Easternkingdoms / EK | 192.168.69.17 | Intel physical KMS | .8 display and CLI/browser pointer/Shift/release independently verified |
| minigpu / MG | 192.168.69.21 | VKMS | .8 real browser greeter click/Escape/release verified; earlier .5 boot recovery verified |

Office API inventory 2026-09-06: office1/2/3 are control-plane+etcd, Spark is
GPU worker; all four Ready. No retired office4 or attic node is silently added
to this goal. Cluster health must remain checked during node-local deployment.

No host reboot is authorised by this ledger. GDM restarts are permitted when
needed, but check for a logged-in user before ending their desktop; never
restart EK's display manager or reboot EK. Avoid touching k3s, workloads,
firewalls or SELinux policy. Back up changed host configuration and old services.

Acceptance per host: native package/config installed, services healthy, actual
nonblank live browser view inspected, real pointer and keyboard effect verified
in a safe target, control release/fail-closed behaviour and package restart
readiness checked. Record limits rather than claiming cross-host proof.
Hotplug physical/VKMS switching remains queued separately as WP10.

## Wayland handoff correction `.10` — 2026-09-07

After `.9`, office1 and office2 reproduced a GNOME Shell logout loop: Mutter's
llvmpipe renderer segfaulted during login, returning the user to GDM. The
supervisor was reapplying `ApplyMonitorsConfig` inside the newly logged-in user
session. `.10` changes this to apply the VKMS layout only at the greeter; the
user session inherits the established `Virtual-1` mode. Office1–3 and minigpu
were upgraded service-only and report healthy 1080p H.264 streaming. No new
GNOME Shell coredumps appeared after rollout in the immediate audit window.

## Unified fleet rollout `.10` — 2026-09-07

Release `0.1.0-lab.20260907.10` was deployed service-only to all six hosts as
one unified offering (KMS, VKMS and X11 are internal backend profiles).
The amd64 bundle was used on EK, office1–3 and minigpu; Spark received a
native ARM64 Rust build plus ARM64 Go bridge. The rollout preserved each
host's existing profile, encoder path and display/session state; no reboot,
GDM/LightDM restart, GPU change or cluster workload change was made.

The release includes bounded vertical/horizontal wheel events through the existing
ordered input channel and injects Linux `REL_WHEEL`/`REL_HWHEEL` events. All
four package services are active on every host and each `/api/status` endpoint
reports 1920×1080 H.264 streaming. Office nodes remained Ready and `/readyz`
returned `ok` after rollout. Archive hashes:

- amd64: `4bd0f0580b98112e47732766418da88d7fe64880f0289938982818b2d4046fb6`
- ARM64: `30568d237646dd9d7a40a83f0830fd44a58b0e39d9d0e777f758394f17842ad1`

## Final current-state audit — 2026-09-07

Fixed-profile fleet alpha acceptance complete. Root independently checked all
six current release links at `0.1.0-lab.20260907.10`, active capture/web/input/
session-supervisor services, valid 1920×1080 guards, and `/api/status` reporting
H.264 1920×1080 streaming without an error. EK was checked locally; the other
five hosts over SSH. Both release archive hashes matched the recorded builds.
Office inventory remained four Ready nodes and `/readyz` returned `ok`.

Actual input evidence (not merely service health or protocol acknowledgements):

- [Office1–3 final browser regression](office-package-eight-regression-2026-09-06.md).
- [MG package validation](minigpu-package-migration-2026-09-06.md).
- [EK console/input validation](easternkingdoms-package-readiness-2026-09-06.md).
- [Spark browser and independent X11 input proof](spark-x11-control-2026-09-06.md).

No reboot or display-manager restart was performed for the final `.10` upgrades
and audit. This does not claim all-host boot recovery, authenticated session
handoff, internet hardening, or automatic physical/VKMS switching. Those remain
separate acceptance scopes; viewers are internal-only unauthenticated alpha.
