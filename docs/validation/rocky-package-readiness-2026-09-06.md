# Rocky office1 package readiness — 2026-09-06

**The exact `.3` amd64 package passes office1's ABI and read-only activation
prerequisites using the existing private FFmpeg. Migration is not performed:
the installer correctly refuses activation while the three old Rocky pilot
services remain active.** This is package readiness, not a new package runtime
or fleet acceptance result.

## Artifact and scope

- Host: `office1`, `192.168.69.27`, Rocky Linux10.0, kernel
  `6.12.0-55.41.1.el10_0.x86_64`, glibc2.39.
- Archive: `donkeywork-desktop-0.1.0-lab.20260906.3-linux-amd64.tar.gz`.
- SHA256: `6350b107e75640d2b9ec48e550b846437253825160088a2e9ec11057ddbd1336`.
- Staged at `/tmp/dwdesktop-package-review.zo1dsW/`; extracted release directory
  is `donkeywork-desktop-0.1.0-lab.20260906.3-linux-amd64` beneath it.
- Archive checksum and the complete extracted `SHA256SUMS` verified.

Only temporary artifact staging and read-only diagnostics occurred. No package
installation, live migration, service restart/activation, reboot, input event,
repository/package-manager operation, firewall change or SELinux policy change
was performed. The source baseline is the root-integrated dirty tree based on
`50b41a66ccbab35b85067edd89dec7d6751bf3c3`; this report does not imply that the
baseline commit alone contains the packaged implementation.

## Checks and results

The **packaged** doctor was run as root with its read-only `--activation` checks:

```sh
sudo bash packaging/doctor.sh --profile vkms --device auto \
  --bin-dir /tmp/dwdesktop-package-review.zo1dsW/donkeywork-desktop-0.1.0-lab.20260906.3-linux-amd64/bin \
  --ffmpeg /opt/donkeywork-desktop/bin/ffmpeg --activation
```

Result: **0 prerequisite failures**, exit0. Artifact amd64 matches x86_64;
ELF/dependency checks found no missing library/ABI; libdrm, Python D-Bus/GLib,
systemd and libx264 are available. All seven packaged binaries also completed
bounded `--help` startup successfully: dwconsole-daemon, dwconsole, input-daemon,
input-cli, web-launch, socket-broker and console-web. Those commands did not
start the capture/input/network services.

VKMS is loaded and adopted. Driver-selected `/dev/dri/card0` reports an active
1920×1080 output, CRTC38; `/dev/uinput` exists. Active seat session remains GDM's
Wayland greeter `c2`, UID42. These are the existing working pilot state, not a
new boot-persistence or input-injection proof.

Encoder must be configured explicitly:

```text
FFMPEG=/opt/donkeywork-desktop/bin/ffmpeg
```

This is the prior separately built minimal FFmpeg8.0.1/libx264 executable,
root-owned0755, SHA256
`850bec40438959b185c880b4c7096bf49fe9cb587ed816f52f2e3b5a9b80ead3`.
Sources, notices and build recipe remain at
`/opt/donkeywork-desktop/share/codecs/`. It supports the VKMS rawvideo encoder
path; it is not a general system FFmpeg or an X11 capture build. The new package
does not bundle or install it. Its current path remains outside versioned
releases and must be retained during migration; do not remove the whole old
`/opt/donkeywork-desktop/bin` directory as cleanup.

SELinux remains **Enforcing**. FFmpeg has `bin_t`, DRM card `dri_device_t`, and
uinput `event_device_t`. The read-only recent AVC query returned no matches.
This verifies existing policy compatibility for these checks only; packaged
service domains, uinput operations and startup still require the actual planned
migration test. No new confinement claim is made.

The installer without activation passed `--dry-run` using the exact extracted
package, profilevkms, listen192.168.69.27 and explicit encoder path. A separate
`--activate --dry-run` returned exit1 before writes with:

```text
active dwconsole pilot units exist; explicit migration is required before activation
```

Confirmed afterward: no `/etc/donkeywork-desktop/console.env` and no
`/opt/donkeywork-desktop/current` were installed.

## Existing live services and migration gate

All three remain active and unchanged:

| Service | Observed state |
| --- | --- |
| `dwconsole-rocky-output.service` | Existing UID42 GDM1080p output/idle-inhibitor helper |
| `dwconsole-rocky-capture.service` | PID775055, active since19:38:19 host-local; private encoder, card0 |
| `dwconsole-rocky-web.service` | PID775058, active since19:38:19 host-local; view-only HTTP8090/UDP8091 |

The existing viewer status endpoint still reports H.2641920×1080,30 configured
FPS, `state=streaming`. No new screenshot/decode measurement was necessary for
this readiness lane. k3s stayed active with its original2026-08-18 activation
timestamp; GDM stayed active since the earlier19:33:43 VKMS experiment.

The concrete next operation, when assigned, is an explicit migration of only
those three pilot services to the package target, preserving their previous
commands/configuration for rollback and retaining the private encoder. Use
profilevkms, listen192.168.69.27, discovered card0 and the encoder above. Then
prove actual browser output/input and session/reboot behavior under the packaged
units. No additional distro package or driver installation is indicated by this
preflight. Office2/3 are not covered by this single-host check.
