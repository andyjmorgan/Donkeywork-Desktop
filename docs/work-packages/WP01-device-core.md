# WP01 — M1a Rust daemon core and local CLI IPC

## Outcome and ownership

Implement the original single-host daemon core in `device/core/`; branch `work/wp01-device-core`, separate worktree. WP02 owns capture/still/input/resize backends, WP05 owns PTYs and WP09 owns the CLI. Root manifests/contracts remain integrator-owned. M1a is local-only, without .NET, Keycloak or browser attachment grants.

## Scope

- Implement lifecycle, capabilities and the dedicated local CLI Unix-socket profile in `contracts/local-cli.md`, draft v0.2.0. Do not expose raw broker IPC to local clients.
- Verify peer UID from the OS and apply the explicit allowlist, socket ownership/permissions and fixed permission/account profile. Client-supplied identity, account or permissions confer no authority. Fail closed on configuration/auth errors.
- Serve topology and screenshot requests with `desktop.view`, without acquiring a control lease. Route screenshots to WP02's shared capture source and return correlated metadata plus bounded binary bytes using the approved framing, never base64 in control JSON.
- Require permission and current control lease for input/resize. Bind actions to session/epoch and current topology; reject stale/out-of-bounds input without silent coordinate rescaling. Never replay ambiguous clicks/text.
- Serialize actual mode changes, release held input, update topology, preserve session/PTY lifetime and report actual resulting mode on failure/rollback. No streaming-generation dependency in M1a.
- Enforce the local lease profile, deadlines/revocation, resource limits and cleanup. Keep input/still/terminal bodies out of logs.

## Prerequisites and blockers

Read AGENTS.md and the architecture review. Freeze v0.2.0 local auth/framing/lease semantics and in-process capture/PTY interfaces before merge. Those interfaces are proposals until approved, not APIs already present. Missing decisions require a contract PR. WP02/WP05/WP09 are needed for real proof; doubles permit isolated work first.

## Acceptance

Test denied UID, unsafe socket permissions, spoofed identity, forbidden broker commands, view-only observation/control denial, malformed/oversized records, session/epoch isolation, stale topology, revocation and held-input release at lease expiry. Validate screenshot metadata/payload bounds without allocating unbounded bytes.

Integrate CLI screenshot → authorized input → confirming screenshot; then actual `3840×2160 → 1920×1080 → 3840×2160` while session and PTY persist. Unsupported pilot resize blocks M1a. Stubs establish no real capture acceptance.

## Boundaries and handoff

Original code only; no RustDesk source/assets/adaptations. No live host installs, display changes, credential retrieval or production modifications without a separate exact-target integration task. Provide branch, base/commit SHA, contract SHA/version, paths, commands/results, environment, dependency provenance, limitations and WP02/WP05/WP09 integration steps. Do not commit/push main or modify another owner's paths.
