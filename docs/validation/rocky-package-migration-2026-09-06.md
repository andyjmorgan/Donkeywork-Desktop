# Rocky office1 package migration — 2026-09-06

**Installed `0.1.0-lab.20260906.5`; actual Rocky Wayland greeter output and
browser keyboard/mouse are working at http://192.168.69.27:8090.** The migration
changed only DonkeyWork task services. GDM and k3s retained their original PIDs
and activation timestamps. No host reboot occurred.

## Authorization and installed state

After the earlier pause, the user resumed work and the integrator explicitly
authorized service-only office1 migration. The active graphical seat was
verified as GDM Wayland greeter `c2`, UID42, before proceeding. No logged-in
desktop was interrupted. Although a later steering message permitted a scoped
GDM restart if needed, none was needed or performed.

- Host: office1,192.168.69.27, Rocky10.0, kernel6.12, AMD plus existing VKMS.
- Artifact: `donkeywork-desktop-0.1.0-lab.20260906.5-linux-amd64.tar.gz`.
- Archive SHA256: `a8d2ff4e4063c91d13a8d69e0c15f5adfca09b5d9fe5713c625fbeb3998d4618`.
- Archive and payload manifest checks passed; exact packaged doctor returned
  zero activation-prerequisite failures before migration.
- `current` points to `releases/0.1.0-lab.20260906.5` beneath
  `/opt/donkeywork-desktop`.
- Configuration: VKMS, listen192.168.69.27, automatic VKMS discovery,1920×1080,
  `FFMPEG=/opt/donkeywork-desktop/bin/ffmpeg`.
- Existing private encoder remains untouched, SHA256
  `850bec40438959b185c880b4c7096bf49fe9cb587ed816f52f2e3b5a9b80ead3`; its sources
  and notices remain under `/opt/donkeywork-desktop/share/codecs`.

The exact three prior transient units were backed up before stopping them:
`dwconsole-rocky-output`, `dwconsole-rocky-capture`, `dwconsole-rocky-web`.
Their unit files contain the existing runtime commands and private session-bus
environment. No package console.env/current symlink existed before migration;
the old binaries/assets and udev configuration were preserved.

Installed package target is enabled; capture, broker, input, web, supervisor,
output helper and guard are all active. Root-private sockets and a fresh guard
bind input to current GDMc2. Guard reports valid1920×1080. The main package
capture, input and web units showed zero restarts during this verification.

## Actual browser and input evidence

1. Display-only browser check decoded native1920×1080,21→172 frames over
   5.0068seconds:30.16 total-frame-rate sample, **five reported dropped frames**
   and zero page errors. The UI reported30.0FPS and44.4KB/s in the captured idle
   sample. These are short observations, not sustained performance/latency claims.
2. A960×640 browser screenshot was visually inspected: actual Rocky logo,
   localuser selection and21:31 clock. The browser auto-connected.
3. Explicit greeter-only browser input proof checked seatc2/Classgreeter before
   injection. First focus click acquired control without transmitting that
   click. The next reviewed user-selection click visibly opened the empty
   localuser password prompt. Escape visibly restored the user list. The UI
   Release input action returned to idle. Seatc2 remained the greeter afterward.
4. Actual click and Escape screenshots were both visually inspected. No password,
   text credential, authentication submission or user login occurred. Reported
   interaction bandwidth was58.8KB/s in those snapshots.

Private local evidence (not uploaded): `/tmp/office1-vkms-browser.png`,
`/tmp/office1-package-click.png`, `/tmp/office1-package-escape.png`.
The first path was reused for the new package screenshot, superseding its old
pilot browser image. Proof scripts live in the agent worktree at
`/home/localuser/source/desktop-rocky-vkms-readiness/deploy/rocky-pilot/`:
`browser-proof.mjs` and `package-input-proof.mjs`; the latter requires explicit
`--allow-reviewed-greeter-input` and pins office1 plus reviewed sessionc2.

## Unchanged services and security state

| Component | Before and after |
| --- | --- |
| k3s | PID1939266; active since2026-08-18 03:39:15 host-local |
| GDM | PID772693; active since2026-09-06 19:33:43 host-local |
| Active graphical seat | GDM Wayland greeterc2, UID42 |
| SELinux | Enforcing; recent AVC query returned no matches |
| Private encoder | Same path, ownership/mode and SHA256 |

Package web runs as nobody; root device helpers remain root. SELinux processes
use the distro's existing `unconfined_service_t`, so successful operation is not
a dedicated confinement policy claim. No repository/package-manager, firewall,
SELinux-policy, modules-load, udev, physical-driver, GDM or k3s changes occurred.

## Rollback and remaining acceptance

Root-private backup: `/var/lib/donkeywork-desktop-migration/office1.DRn4ar`.
It contains exact old unit files, active-seat identity, k3s/GDM baseline, encoder
hash and a copy of the scoped migration/rollback helper. If rollback is needed
while the same seat remains valid:

```sh
sudo bash /var/lib/donkeywork-desktop-migration/office1.DRn4ar/migrate.sh \
  --rollback /var/lib/donkeywork-desktop-migration/office1.DRn4ar
```

The helper stops/disables only package services, restores the three old units
under `/run/systemd/system`, and starts them in output/capture/web order. New
package files remain inert for inspection; no broad directory deletion occurs.
It refuses automatic restoration if the active seat changed, to avoid reviving
an old GDM bus binding. Rollback was prepared, not exercised after this success.

**Boot readiness remains incomplete on office1:** VKMS was previously loaded
live and no modules-load persistence was added in this scoped migration.
The package target is enabled, but the virtual driver may be absent after a
future boot. A separately coordinated persistence change is needed before
claiming reboot recovery. No reboot, login/logout transition, long-duration
soak or office2/3 rollout is inferred from the successful greeter/input proof.
