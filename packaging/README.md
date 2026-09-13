# Internal Linux console package

One versioned tarball contains original Rust capture/input/socket helpers, the
Go bridge, built browser assets, Python runtime helpers and systemd units.
Profiles: **vkms** for monitorless GNOME Wayland; **kms** and
**x11** default to view-only unless guarded control is explicitly selected.1080p is
the console target. A tarball is built for one declared Linux CPU architecture
and build environment; it is not a universally portable Linux binary.

From `.6`, `--control x11` opts an X11 or physical-KMS profile into guarded
input for an existing X11 seat (including LightDM). The root guard verifies
capture process, active session, Xauthority metadata and one unscaled XRandR
output; KMS additionally checks the captured DRM device and mode. It never
creates a desktop or changes its layout. Without this option those profiles
remain view-only. VKMS retains its existing guard under `CONTROL=auto`.
This is not generic Wayland input support. Live validation is tracked per host.

Use `.8` or later for X11 input: earlier descriptors advertised both relative
and absolute position axes, causing Xorg/libinput to discard absolute events.
`.8` uses an absolute pointer with wheel axes only. Actual browser pointer and
keyboard effects were independently verified on EK/Spark and the Wayland
greeter path retested on minigpu. See `docs/validation/fleet-alpha-rollout.md`.

No live rollout follows from building or staging the package. The installer
requires an explicit profile and RFC1918 listen address. It never installs
packages, enables repositories, disables SELinux, loads a driver, restarts GDM,
logs out a user, or reboots a host. The exact source tree, build metadata and
SHA256 manifest accompany the artifact; checksums detect corruption, not an
untrusted publisher. Use an artifact produced from the reviewed local source.

## Inspect and stage

After extracting the archive, from its root:

```sh
bash packaging/doctor.sh --profile vkms --bin-dir "$PWD/bin"
bash packaging/install.sh --source "$PWD" --profile vkms \
  --listen 192.168.69.21 --dry-run

# Non-root filesystem staging test; cannot activate services.
bash packaging/install.sh --source "$PWD" --profile vkms \
  --listen 192.168.69.21 --root /tmp/dwdesktop-install-review
```

`doctor` is read-only. It checks OS/architecture, artifact ELF/dependencies,
systemd, Python D-Bus/GLib, FFmpeg/libx264, and profile prerequisites. It cannot
prove actual greeter pixels, login handoff or performance. Activation adds
checks that VKMS/uinput and a1080p DRM output are already available.

FFmpeg is an external dependency, not bundled. Default is `/usr/bin/ffmpeg`.
Use `--ffmpeg /absolute/approved/ffmpeg` consistently with doctor and installer
to select a separately reviewed encoder, such as office1's private codec build.
X11 requires an encoder build with `x11grab`; the minimal Rocky rawvideo encoder
does not provide that input. Python bindings, libdrm and ELF ABI must also match
the target. Missing dependencies fail preflight; this package does not fetch them.

## Install, configure, activate

```sh
sudo bash packaging/install.sh --source "$PWD" --profile vkms \
  --listen 192.168.69.21

# Only after display prerequisites and live-pilot migration are reviewed:
sudo bash packaging/install.sh --source "$PWD" --profile vkms \
  --listen 192.168.69.21 --activate
```

The second example reuses the first installation only if its manifest and
installed files still exactly verify; releases are immutable and never
overwritten. Changed contents require a new VERSION. No migration of existing
pilot services is implied by these commands.

Installation creates `/opt/donkeywork-desktop/releases/VERSION`, then atomically
switches `current`. Configuration is strict key/value data at
`/etc/donkeywork-desktop/console.env`; it is **never sourced as shell code**.
An existing differing config requires `--replace-config`. New unit names are
`donkeywork-desktop*`, avoiding overwrite of live `dwconsole*` pilot files.
`--activate` invokes doctor and refuses active old pilot units before installing;
it does not stop or migrate them automatically. HTTP8090 and WebRTC UDP8091 bind
only the selected lab IP; no firewall changes occur.

VKMS provisioning is separately explicit: add `--configure-vkms` to stage a
module-load file and preferred-primary udev rule. If the current vendor Mutter
rule explicitly ignores VKMS, the installer creates a same-name local copy
removing only that exact line. Existing local rules are never overwritten;
reconcile them manually. This vendor-copy override must be reviewed after
Mutter package updates. No udev reload, module load or graphical-service restart
is performed. Any required GDM restart needs its own coordinated action before
activation; `--configure-vkms --activate` will fail if prerequisites are not
already live. The flag is for first provisioning, not repeated upgrades.

## Rollback and limits

Previous release directories remain intact. Planned rollback stops only package
units, restores the prior `current` symlink/configuration, reloads systemd and
starts the prior package version. Do not overwrite a version directory. Failed
installation retains its staging directory for inspection; partial config/unit
installation may require review. Explicit VKMS files can be moved aside and the
original vendor policy restored during a separately coordinated display change.
Do not unload a physical GPU or restart a host as an installer cleanup action.

This remains an internal lab pilot: no user authentication/public hardening,
dedicated SELinux confinement, guaranteed cross-distro ABI, or completed fleet
acceptance. The root helpers and existing process-privilege boundaries retain
their documented limitations. Project outbound licence is pending; third-party
notices/provenance must travel with builds. Do not publish it as a production or
generally licensed distribution until that decision is recorded.
