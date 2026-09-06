# M1a review disposition — 2026-09-06

Fable's review is retained unchanged in docs/reviews/2026-09-06-m1a-architecture-review.md. Accepted: CLI-first sequencing, screenshot contract, explicit CLI auth/transport, snapshot metadata, X11 pilot gate, WP09 ownership and one capture source for still/video. Browser and broker work moves to M1b.

Corrections and explicit decisions:

- A 16 MiB video payload cap does not guarantee native-4K PNG support. The local snapshot contract sets independent 64 MiB encoded and decoded limits with checked dimension arithmetic.
- M1a completes locally using explicit UID policy. Remote OAuth/API is M1b, not a hidden second M1a gate. Device flow is operator login, not an unattended credential solution.
- Observation does not require an input lease. Do not weaken input deadlines to accommodate short-lived CLI commands. Authorization, attachment and control are distinct.
- Topology rejection already exists; snapshots bind observations into that invariant. Unchanged geometry cannot ensure unchanged application content or click target.
- A mode inventory is read-only; switching modes is a separately authorized mutation. No lab access or display changes are delegated by this plan.
- X11 is the selected first backend, not a claim that Wayland mode changes are impossible. Generic Wayland support is deferred.
- Terminal wire semantics can be reused conceptually, but the local UID/session transport must be explicit rather than accepting broker-trusted commands from CLI callers.

## First implementation boundaries

Each lane uses a dedicated worktree and standalone Cargo package in its owned path, with its own manifest/lockfile. No root workspace is needed until integration. The integrator owns eventual workspace wiring and cross-component adapter changes.

- WP01 device/core/: Unix framing, UID policy, session/lease state, request dispatch. A backend unavailable result is valid; fake capture success is not.
- WP02 device/capture/: independently testable X11 capture/input/mode library. Expose documented typed frame/topology/input/mode methods. Core integration waits for owner-reviewed Rust API agreement; no private wire variants.
- WP09 cli/: local wire client, typed command UX, JSON metadata and private PNG artifact writing. Socket fixture tests before live daemon availability.
- WP05 device/terminal/: real PTY added in the next slot. Until integrated, daemon terminal commands fail explicitly as unsupported.

Initial independent commits may be tested components, not an interoperable runtime. The integration gate remains the shared local wire contract plus an owner-reviewed backend API. Live lab evidence is a later explicitly authorized task.
