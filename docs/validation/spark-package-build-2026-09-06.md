# Spark ARM64 package build — 2026-09-06

Subsequent separately authorized service-only installation is recorded in
[Spark package migration](spark-package-migration-2026-09-06.md). The statements
below about no activation describe the build lane, not that later migration.

## Final artifact: 0.1.0-lab.20260906.5

Use `artifacts/releases/donkeywork-desktop-0.1.0-lab.20260906.5-linux-arm64.tar.gz`
and its adjacent `.sha256`, both retrieved and verified locally.

- Final archive SHA256: `1a0e6159a7d2455fbda690109a4d184b411ac8fcd591c295bb5d7136d806542a`.
- Final archive size: 11,699,323 bytes.
- Final exact-source SHA256: `0f3aaa2184dc1cdce8470e740299434ec04e85d7f75806ac642dcfbe3c38c574`.
- Final Spark staging: `/home/localuser/dwdesktop-spark-final.2irbpB`.
- Final local staging: `/tmp/dwdesktop-spark-final.wLSZbZ`.

Version .5 reuses the verified .3 binaries and browser assets: Rust sources and
Cargo manifest/lock, Go tree and browser source/lock were compared unchanged.
Every reused packaged binary was compared byte-for-byte on Spark. The payload
contains fresh current integration sources, package helpers/units, a new VERSION,
updated provenance, regenerated ELF/dependency records and SHA256 manifests.

The intervening packaging fixes were identified and verified by the root
integrator on minigpu, **not by activating this package on Spark**:

1. .3 → .4: remove the web unit's `RestrictAddressFamilies` setting; its combination
   with explicit `User=root` interfered with the launcher's required privilege drop.
2. .4 → .5: order the web unit after `donkeywork-desktop-input.service` to avoid
   connecting the old input socket during package-target restart.

The intermediate .4 archive remains only in isolated Spark staging and is not
the final artifact. .5 includes `share/build/REPACKAGE-0.5.txt` and its exact
repackaging script. Payload checksum verification and read-only X11 doctor were
repeated for .5: **0 prerequisite failures**. No Spark service was activated,
restarted or otherwise changed during either refresh.

## Original compiled artifact: 0.1.0-lab.20260906.3

Built and retrieved the initial internal ARM64 bundle
`artifacts/releases/donkeywork-desktop-0.1.0-lab.20260906.3-linux-arm64.tar.gz`.
The accompanying `.sha256` verifies locally.

- Archive SHA256: `eae1c0cbca6789bf7c1a7d19f0dae9dc6aca353831e3034ffe08b4b6b188a017`.
- Archive size: 11,698,562 bytes.
- Version: `0.1.0-lab.20260906.3`; ARCH: `arm64`.
- Host: Spark `192.168.69.28`, Ubuntu 24.04.4 LTS,
  `6.17.0-1032-nvidia`, `aarch64`.

## Build method and exact source

This is an explicitly approved **hybrid build**, not a claim that all components
were compiled natively on Spark. Spark had Rust/C tooling and cached dependencies
but no discovered Go/Node/npm toolchains. Nothing was installed to overcome that.

| Component | Build environment |
| --- | --- |
| Six Rust binaries | Native Spark ARM64, cargo/rustc 1.98.1, existing C/library dependencies; `--offline --locked --release` |
| Go browser bridge | Easternkingdoms Go 1.26.4 linux/amd64; `GOOS=linux GOARCH=arm64 CGO_ENABLED=0`, cached modules only |
| Browser assets | Easternkingdoms Node 22.22.3 / npm 10.9.8; copied existing node_modules, TypeScript check and Vite production build |
| Assembly, ELF/dependency validation, manifest | Spark ARM64 |

The source snapshot came from the dirty integrated main workspace, not just Git
HEAD. Selected source directories: `device/console`, `console-web`, `console-ui`,
`deploy/package`, `deploy/vkms`, `packaging`, plus `PROVENANCE.md`. Build caches,
node_modules, generated dist and Python caches were excluded.

- Baseline Git commit: `50b41a66ccbab35b85067edd89dec7d6751bf3c3`.
- Exact source snapshot SHA256:
  `885b83b3e4c36c7b97944ad9b6b962c216971d8a4ddedaaff23cebdc5b61d626`.
- Source is embedded as `share/build/project-source.tar.gz`.
- Assembly recipe is embedded as `share/build/assemble-hybrid.sh`.
- Toolchain/hybrid details, build OS, lockfiles, ELF/dependency reports, Cargo
  metadata and Go module inventory are in `share/build`.

An initial all-platform offline Cargo metadata request required uncached Windows
metadata (`anstyle-wincon`) and failed without downloading it. Metadata generation
was changed to `--filter-platform aarch64-unknown-linux-gnu --offline --locked`.
That target-specific inventory completed; it is not an all-platform inventory.

## Verification performed

- Native Rust release build: passed.
- Native Rust tests: **24 passed** (9 library, 11 input daemon, 4 socket broker).
  No live input devices were created by these tests.
- Package runtime Python tests on Spark: **8 passed**.
- Platform-neutral UI TypeScript/Vite build: passed.
- Every packaged binary's ELF architecture: ARM64; dependency records generated;
  no `ldd` missing-library result.
- Cross-built Go bridge's `--help` executed successfully on Spark without
  binding an HTTP listener or touching the capture stream.
- Payload `SHA256SUMS`: every included file verified before archiving.
- Retrieved archive `.sha256`: verified on Easternkingdoms.

Read-only doctor invocation against the staged payload:

```sh
bash packaging/doctor.sh --profile x11 --device /dev/dri/card0 \
  --bin-dir /home/localuser/dwdesktop-spark-package.wxNVTc/donkeywork-desktop-0.1.0-lab.20260906.3-linux-arm64/bin \
  --ffmpeg /usr/bin/ffmpeg
```

Result: **0 prerequisite failures**. Report confirmed systemd, libdrm, matching
artifact architecture, Python D-Bus/GLib, host FFmpeg/libx264, X11 capture support
and installed Xorg. It correctly warned that this X11 package profile is view-only
and real physical greeter/session capture still needs package-level acceptance.

## Scope preserved and remaining acceptance

Only new build staging directories and the retrieved release/document were
created. No dependency installation, running-service alteration, display change,
package activation, user logout or reboot occurred. No credentials were retrieved.
The existing Spark console pilot was not replaced or exercised for this build.

Retained build locations:

- Spark: `/home/localuser/dwdesktop-spark-package.wxNVTc`.
- Easternkingdoms: `/tmp/dwdesktop-spark-package.w5Nju7`.

The artifact is **not installed or activated**. Package X11 capture, recovery and
reboot acceptance remain separate work. It remains an internal lab alpha without
authentication, production confinement or a finalized outbound licence. FFmpeg
is an external host dependency, not bundled. Build/dependency provenance is not
a completed third-party licence/source compliance package.
