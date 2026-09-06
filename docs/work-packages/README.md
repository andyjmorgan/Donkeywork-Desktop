# Parallel work packages

M1 proves one Linux host providing its console and terminal to an authenticated web browser. The broker and device worker run on that host. Fleet rollout, enrollment, MCP execution and computer-use pods are later milestones.

M1 also requires changing the **actual remote desktop resolution** from the web UI during an active session: `3840×2160 → 1920×1080 → 3840×2160`, without reconnecting the desktop session or interrupting its terminal. Browser/video scaling does not satisfy this requirement. The pilot must expose supported modes; lack of live mode-changing capability blocks M1 completion. Contract messages `display.resize` / `display.resize.result`, `availableResolutions`, the `desktop.resize` permission, control lease, topology revisions and decoder `streamGeneration` changes are shared dependencies across WP01–04, WP06 and WP07.

These documents are executable assignment briefs for agents, Fable or human contributors. GitHub issues track assignment and discussion; the versioned documents define the work. An assignment is not permission to install software on lab hosts, change production, publish endpoints or modify Keycloak.

## Start gate and coordination

The protocol in `contracts/` is **v0.1.0 draft**. Fixture-driven prototypes can begin against an identified commit. Before implementations merge, the integration owner must approve and freeze a consistent baseline, including capture/encoder interfaces, session lifecycle, grants, media framing, input and terminal messages. Missing decisions block the affected implementation; record the exact missing decision rather than silently inventing another protocol.

Any shared-interface change requires a contract PR first, with affected owners reviewing compatibility and fixtures. Work package authors own no files in `contracts/`. Until a baseline is frozen, no package may claim interoperability or production readiness.

Use a separate Git worktree and branch per assignment (`work/wpXX-name`). One owner writes each path at a time. The integration owner owns root manifests, shared build configuration, contract fixtures and release decisions; request changes there through a separate PR. No agent changes another package's directory to make its own work pass.

| Package | Owned implementation paths | Dependencies |
| --- | --- | --- |
| [WP01](WP01-device-core.md) | `device/core/` | Contract lifecycle, local IPC and capture/terminal interfaces |
| [WP02](WP02-capture-encode.md) | `device/capture/` | Capture/encoder contract; WP01 for live integration |
| [WP03](WP03-browser-session.md) | `browser-session/` | Media/input contract; WP02 for live video, WP04 for grants |
| [WP04](WP04-broker-auth.md) | `broker/` | Auth/IPC contract; WP01 for live sessions |
| [WP05](WP05-terminal.md) | `device/terminal/`, `browser-terminal/` | Terminal contract; WP01 and WP04 for live authorization |
| [WP06](WP06-web-console.md) | `web/` | Public APIs from WP03–05; fixtures permit earlier UI work |
| [WP07](WP07-integration-acceptance.md) | `tests/integration/`, `tests/performance/`, `tests/security/`, `docs/validation/` | Frozen contracts and WP01–06 for full acceptance |
| [WP08](WP08-future-design.md) | `docs/future/` | M1 contracts and emerging limitations; design only |

Suggested initial lanes are WP01, WP02, WP03 and WP04, subject to available owners. WP05 and WP06 can develop against approved fixtures. WP07 defines measurements early and performs system validation once components exist. WP08 does not block M1. This is a dependency graph, not a schedule or provider allocation.

## GitHub ledger

[M1 milestone](https://github.com/andyjmorgan/Donkeywork-Desktop/milestone/1) tracks WP01–07; WP08 is future design only. No implementation owners have been assigned automatically.

| Work package | Issue |
| --- | --- |
| WP01 — Device core | [#1](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/1) |
| WP02 — Capture/encode | [#2](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/2) |
| WP03 — Browser session | [#3](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/3) |
| WP04 — Broker/auth | [#4](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/4) |
| WP05 — Terminal | [#5](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/5) |
| WP06 — Web console | [#6](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/6) |
| WP07 — Acceptance | [#7](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/7) |
| WP08 — Future design | [#8](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/8) |

## Shared acceptance and handoff

All implementation must be original: no copying or translating RustDesk source, UI, assets or wording. Document independent dependencies and their licenses. RustDesk is background inspiration only.

Every handoff includes branch and commit SHA, owned paths changed, contract-baseline SHA, commands run with results, recorded environment, limitations, blocked prerequisites and the next integration step. Distinguish fixture/mock tests from real hardware results. Never report 4K, hardware acceleration, security or end-to-end compatibility based on stubs. Include provenance and dependency-license notes. Open a draft PR when useful; do not merge across ownership boundaries without review.
