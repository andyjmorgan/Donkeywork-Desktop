# Parallel work packages

M1a proves a local authenticated Rust daemon plus agent CLI on one Linux X11 host: native-4K screenshots, coordinate clicks, keyboard/text input, actual live resolution changes and a real PTY. It requires no .NET broker, Keycloak, browser, remote CLI authentication or video encoding. M1b adds browser video, the .NET broker, Keycloak and original web console around that same capture/input backend. No implementation APIs are claimed to exist yet.

Both stages require actual display mode changes `3840×2160 → 1920×1080 → 3840×2160` without reopening the desktop session or disrupting its PTY. M1a drives this through the CLI; M1b through the web UI. Scaling a screenshot, video or browser is insufficient. Unsupported pilot mode switching blocks acceptance.

Read the [architecture review](../reviews/2026-09-06-m1a-architecture-review.md) for findings. The selected response is local-only M1a; the review's remote OAuth/HTTP phase and suggested timing/payload values are proposals, not additional M1a requirements. The integrator's [local CLI contract](../../contracts/local-cli.md), draft v0.2.0, defines the selected profile and limits.

## Start gate and coordination

Freeze a reviewed v0.2.0 baseline, schemas, fixtures and the WP01/WP02 capture interface before implementation merges. Wire schemas do not themselves define Rust traits, browser package APIs or HTTP endpoints. Missing interfaces block their dependent integration and require a contract PR before implementation assumes them. The integrator owns `contracts/`, shared root manifests and build wiring.

Use separate worktrees and `work/wpXX-name` branches. One owner writes each path; no private protocol forks or edits to another owner's directory. Assign against a recorded baseline SHA. These briefs are planning/implementation assignments, not permission to install on hosts, change displays, retrieve credentials, change Keycloak or deploy.

## Dependency and ownership graph

| Package | Stage | Owned paths | Dependencies |
| --- | --- | --- | --- |
| [WP01](WP01-device-core.md) | M1a | `device/core/` | Frozen local CLI/session/auth and capture/PTY interfaces |
| [WP02](WP02-capture-encode.md) | M1a capture/stills/input; M1b video | `device/capture/` | Capture interface; WP01/WP09 for live proof |
| [WP09](WP09-agent-cli.md) | M1a | `cli/` | Local CLI contract; WP01/WP02/WP05 for live proof |
| [WP05](WP05-terminal.md) | M1a daemon PTY; M1b browser | `device/terminal/`, `browser-terminal/` | Local terminal auth/profile contract; WP01/WP09 first |
| [WP07](WP07-integration-acceptance.md) | M1a gate, then M1b gate | `tests/integration/`, `tests/performance/`, `tests/security/`, `docs/validation/` | WP01/WP02/WP05/WP09 for M1a; WP03/WP04/WP06 additionally for M1b |
| [WP03](WP03-browser-session.md) | M1b deferred | `browser-session/` | Proven capture source, media transport/API amendments and WP04 |
| [WP04](WP04-broker-auth.md) | M1b deferred | `broker/` | Explicit broker HTTP/auth/IPC boundary contracts |
| [WP06](WP06-web-console.md) | M1b deferred | `web/` | Approved browser-session/terminal package APIs and broker HTTP API |
| [WP08](WP08-future-design.md) | Future design only | `docs/future/` | Proven M1a/M1b boundaries; no implementation |

Three initial lanes: **WP01 daemon core**, **WP02 capture/still/input**, **WP09 CLI**. They can use approved fixtures until the interfaces connect. WP05 PTY comes next, while WP07 prepares independent acceptance. WP03/WP04/WP06 and the video/browser halves of WP02/WP05 are deferred to M1b. WP08 does not block either gate.

M1a authentication uses verified Unix peer UID, an explicit allowlist, service-owned socket permissions and a fixed permission/account profile. The CLI cannot submit a principal or access raw broker-only IPC. Topology/screenshot reads require `desktop.view` but no control lease; input and actual resize require permission plus the current lease and topology validation. Browser grants and stream generations do not belong to M1a.

## GitHub ledger

[M1a — Agent CLI and Linux daemon](https://github.com/andyjmorgan/Donkeywork-Desktop/milestone/1) and [M1b — Live browser desktop](https://github.com/andyjmorgan/Donkeywork-Desktop/milestone/2) track the split. No implementation owners are assigned automatically.

| Work package | Issue |
| --- | --- |
| WP01 — M1a device core | [#1](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/1) |
| WP02 — M1a capture/still/input; M1b video | [#2](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/2) |
| WP03 — M1b browser session | [#3](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/3) |
| WP04 — M1b broker/auth | [#4](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/4) |
| WP05 — M1a PTY; M1b browser terminal | [#5](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/5) |
| WP06 — M1b web console | [#6](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/6) |
| WP07 — M1a/M1b acceptance | [#7](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/7) |
| WP08 — Future design | [#8](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/8) |
| WP09 — M1a agent CLI | [#9](https://github.com/andyjmorgan/Donkeywork-Desktop/issues/9) |

## Shared acceptance and handoff

Write original code; do not copy/adapt/translate RustDesk source, assets or wording. Document inspiration and dependency licenses. This is not clean-room development, and the outbound project license remains undecided.

Every handoff records branch, base/commit SHA, contract-baseline SHA/version, owned paths, commands/results, environment, provenance, limitations, blocked prerequisites and next integration step. Distinguish fixture tests from physical-host evidence. Never claim 4K capture, hardware acceleration, auth or interoperability from stubs. No payloads, keystrokes, grants, credentials or PTY content in logs. Submit reviewable branches; no merges/pushes to main from a work package.
