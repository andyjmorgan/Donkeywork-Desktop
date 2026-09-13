# DonkeyWork Desktop

> Retired lab experiment — September 13, 2026. The user ended this rollout after
> repeated console/session stability failures. Desktop agents and legacy pilots
> have been removed from all six desktop hosts. Earlier success and deployment
> statements below are historical, not current service availability or acceptance.
> See [retirement and cleanup](docs/validation/retirement-2026-09-13.md).

> Manager milestone (2026-09-10): [internal Go/PostgreSQL design](docs/milestones/M3-internal-manager.md)
> and [first enrollment implementation](manager/README.md). No Keycloak or public
> access in this milestone. This supersedes the older .NET/Keycloak manager plan.

> Current direction (2026-09-09): managed terminal-services desktops, initially
> Ubuntu Spark and Minigpu. See [the runnable pilot](deploy/managed/README.md)
> and [its explicit limits](contracts/managed-session-poc.md). The historical
> physical-console/fleet completion claims below are not current acceptance.

Linux console and terminal access, starting with an authenticated agent CLI and followed by an original Keycloak-authenticated web console.

**Current console alpha:** Spark X11, Easternkingdoms Intel KMS and
minigpu/office Rocky VKMS stream H.264 to the browser with guarded keyboard/mouse
and shared CLI input. See the [fleet rollout evidence](docs/validation/fleet-alpha-rollout.md)
for per-host versions and actual verification. Full login/logout, hotplug and
fleet-wide reboot acceptance are separate; no fleet auth/broker is integrated.

The working console is consolidated here: Rust `device/console/`, Go
`console-web/`, and React `console-ui/`. The older `web/` remains a demo;
do not deploy it over the live assets. Build a native, versioned lab bundle
with `bash packaging/build.sh`; see [installation and profiles](packaging/README.md).
No sibling worktree is required.

Native bundles now exist for amd64 and ARM64. See the
[minigpu package migration](docs/validation/minigpu-package-migration-2026-09-06.md),
[Rocky prerequisites](docs/validation/rocky-package-readiness-2026-09-06.md), and
[Spark ARM64 build](docs/validation/spark-package-build-2026-09-06.md).
The `.8` fleet remains the validated baseline; `.9` adds browser wheel input
end-to-end and is ready for service-only rollout. Earlier artifacts are
superseded. Target-host boot and login
acceptance are tracked individually, not inferred across the fleet.

M1a and the fixed-profile `.8` alpha are complete. The local daemon, H.264
browser view and real input are proven on the current six-host fleet. The next
milestone is the authenticated fleet web console: Keycloak OAuth/OIDC, device
enrollment and online status, plus session create/view/control/destroy. The
broker will move to attic after local acceptance; keep the portal internal or
behind UniFi VPN while auth and fleet policy are validated.

## Start here

- [Web console start and existing DonkeyWork theme references](docs/web-console-start.md)
- [Run the local web-console preview](web/README.md) — `npm --prefix web ci`, then `npm --prefix web run dev`; no live broker connection yet.

- [Accepted headless POC and future login model](docs/decisions-headless-poc.md)

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
