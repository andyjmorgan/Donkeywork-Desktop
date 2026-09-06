# DonkeyWork Desktop

Linux console and terminal access, starting with an authenticated agent CLI and followed by an original Keycloak-authenticated web console.

**Status: contract and work-package bootstrap. No runnable desktop implementation yet.**

M1a proves a Rust daemon and local Unix-socket CLI on one X11 pilot: native-4K PNG screenshots, clicks/keyboard input, actual live resolution changes and a real PTY. Explicit peer-UID policy authorizes access; no broker, browser or public endpoint is needed for that first slice. M1b adds the .NET broker, Keycloak and browser streaming, including mandatory live resolution changes from the UI. Move the broker to attic and enroll the fleet only after this engine is proven.

## Start here

- [Project brief](docs/project-brief.md)
- [Architecture and decisions](docs/architecture.md)
- [Protocol contracts](contracts/README.md)
- [Parallel work packages](docs/work-packages/README.md)
- [Agent/Fable handoff](docs/parallel-work.md)
- [M1 acceptance](docs/acceptance.md)
- [Provenance and licensing](PROVENANCE.md)

## Contract checks

Requires Node.js 22+ and npm. No Rust/.NET toolchain is required to validate the baseline.

```sh
npm ci
npm test
```

Tests check strict JSON schemas, examples, negative cases and an executable semantic contract model. They do **not** prove implementation interoperability, desktop performance, encryption or OS isolation.

## Proposed source boundaries

| Path | Owner |
|---|---|
| device/core/ | WP01: worker lifecycle and IPC |
| device/capture/ | WP02: capture/encode |
| browser-session/ | WP03: decoder/render/input |
| broker/ | WP04: .NET broker and auth |
| device/terminal/, browser-terminal/ | WP05: PTY transport and terminal component |
| web/ | WP06: React console |
| cli/ | WP09: authenticated agent CLI (M1a) |
| tests/integration/, tests/performance/, tests/security/, docs/validation/ | WP07: integration and acceptance |
| contracts/ | Contract integrator; changes require cross-component review |

Shared entrypoints/manifests are integration-owned; work packages should add component modules without racing to edit the same root files.

The working name is DonkeyWork Desktop. RustDesk is a documented engineering reference, not a source dependency or branding template. See PROVENANCE.md before introducing third-party code.
