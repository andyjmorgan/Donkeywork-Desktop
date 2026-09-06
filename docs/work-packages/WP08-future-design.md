# WP08 — Future fleet, MCP and computer-use pod design

## Outcome and ownership

Write a bounded design/backlog for capabilities after M1a/M1b. Own only `docs/future/`; branch `work/wp08-future-design`, separate worktree. This package produces no implementation or deployment and does not block either milestone. M1a is local Rust daemon plus CLI; M1b adds browser streaming, .NET and Keycloak. Remote CLI OAuth/HTTP access remains a separately scoped future design, not an M1a gate.

## Scope

- Describe moving the M1b broker from the device to attic while retaining device/session boundaries. Use the current reviewed contract SHA, including `contracts/local-cli.md` draft v0.2.0; do not treat the local CLI profile as a ready remote API or expose raw broker IPC to clients.
- Specify proposed enrollment, device identity rotation/revocation, heartbeat freshness, authorization and fleet presence semantics for attic/office nodes, minigpu and Spark. Unknown inventory/capabilities must remain explicit.
- Describe MCP observe/input/resize tooling operating on the same desktop as the human viewer, with serialized actions and human takeover.
- Describe a desktop pod runtime for DonkeyWork-Sandbox: shared display/browser context, lifecycle, isolation, display resize and explicit persistence.
- Describe reference-based Vault credential delivery without secrets in tool arguments, output, audit bodies or screenshots by default. State remaining trust and retention limits rather than claiming clipboard secrecy.
- Record transport alternatives, including Cloudflare portal hosting and separately reachable media using static IPs. Do not pre-approve public exposure.

Original designs only; no RustDesk copying, UI/assets or translations. Document influences and proposed dependency licenses. No fleet installation, MCP coding, existing-project edits or production changes.

## Prerequisites and blockers

Use the identified draft contract baseline and list assumptions. Any proposed M1 contract change goes through a contract PR reviewed by affected owners; do not silently expand M1. Actual Sandbox/Vault integration requires fresh source inspection and applicable repository instructions during its later implementation. Missing inventory or unresolved transport decisions are named future blockers.

## Acceptance

- Deliver architecture notes and separately scoped backlog items with dependencies, owned future components, authorization requirements and testable outcomes.
- Explain which M1 contracts survive separation and which need versioned extensions.
- Include a threat/trust-boundary sketch and a human/agent control-ownership state model, clearly marked proposals.
- Explicitly defer fleet rollout, MCP execution and computer-use pods beyond M1.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, documents reviewed, dependency/provenance notes, open questions and prioritized follow-on items. Tests are document/link and consistency checks only; do not claim implementation validation. No production changes are authorized.
