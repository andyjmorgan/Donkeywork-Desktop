# Parallel agent and Fable workflow

## One baseline, bounded assignments

Use GitHub issues and docs/work-packages as the common task ledger. Each issue body is a self-contained handoff. Fable is another developer consuming the same repo/contracts; no special integration is assumed or configured.

The [issue ledger](work-packages/README.md#github-ledger) maps all eight packages. Suggested split: device core and capture/encode as one coordinated engineering lane; browser session and its 4K fixture tests as a second; broker/auth as a third. Fable can take WP06's original UI shell against labelled mocks while those contracts settle, or WP07's independent test harness. These are suggested assignments, not dispatched jobs. WP05 is independent PTY work once its worker/package interfaces are agreed.

Record the exact `git rev-parse HEAD` at handoff; never hand an agent a moving branch name as its only baseline reference.

1. Read the brief, architecture, contracts, acceptance and assigned package.
2. Start from the recorded baseline SHA and create a dedicated worktree/branch.
3. Own only the package's paths. Shared root manifests and contracts are integrator-owned.
4. Implement against fixtures; request amendments before depending on undocumented behavior.
5. Run contract checks plus component checks. Provide a reviewable PR with evidence.
6. Integrator merges in dependency order and runs real end-to-end checks.

Example, after the bootstrap is pushed:

```sh
git fetch origin
git worktree add ../desktop-wp03 -b work/wp03-browser-session origin/main
```

Do not use the same worktree for two agents. Do not share live secrets, broad Vault credentials or host-root access in task handoffs.

## Contract change control

0.1.0 is an initial **draft baseline**, not a proven production protocol. Implementations may start against it after reading known limitations. Any field/semantic change requires a contract-only PR with updated schemas, examples, negative cases and affected work-package acknowledgments.

Before cross-component integration, review the baseline with worker, browser and broker owners and mark an agreed revision. Freeze the agreed revision for that integration run. No component may privately fork the protocol.

Do not mistake JSON validation for session safety. [The contract reference](../contracts/README.md) describes semantic checks that generated types cannot enforce. The executable model is a fixture oracle, not a production auth implementation.

## Handoff template

- Work package and issue:
- Baseline SHA / contract revision:
- Branch and final commit:
- Owned paths changed:
- Behavior delivered:
- Tests run and results:
- Evidence (logs without secrets, recordings/benchmarks if applicable):
- Limitations and unresolved contracts:
- Exact next integration step:

## Integration order

Contract baseline -> parallel component shells/fixtures and codec spike -> real capture/browser pairing -> real worker/broker auth and PTY -> UI wiring -> 4K/security/recovery validation.

WP08 is design/backlog only. Fleet rollout, MCP desktop tools and computer-use pods do not become authorized implementation just because they are documented.
