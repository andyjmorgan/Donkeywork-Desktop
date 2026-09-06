# DonkeyWork Desktop

Linux console and terminal access from an original, Keycloak-authenticated web console, followed by agent/MCP access to the same sessions.

**Status: contract and work-package bootstrap. No runnable desktop implementation yet.**

M1 co-locates the .NET broker/web server and Rust device worker on one pilot host. A browser on a different machine must demonstrate high-quality 4K desktop interaction and a real PTY. Split the broker onto attic and enroll the fleet only after this session engine is proven.

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
| tests/integration/, tests/performance/, tests/security/, docs/validation/ | WP07: integration and acceptance |
| contracts/ | Contract integrator; changes require cross-component review |

Shared entrypoints/manifests are integration-owned; work packages should add component modules without racing to edit the same root files.

The working name is DonkeyWork Desktop. RustDesk is a documented engineering reference, not a source dependency or branding template. See PROVENANCE.md before introducing third-party code.
