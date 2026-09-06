# Working agreement

Read README.md, contracts/README.md, docs/parallel-work.md and your assigned work package before implementing.

- Implement original code. Do not copy or adapt RustDesk source, UI assets or wording. It has been inspected as a reference; this is not clean-room development. Surface any proposed upstream source reuse as a separate decision.
- Own only assigned paths. Use a dedicated branch/worktree. Do not reset, overwrite or clean another worker's files.
- contracts/ is owned by the integrator. Propose a contract amendment before depending on new messages, fields or semantics; update fixtures and tests together.
- No placeholder success paths: unsupported capture, codec, authentication or transport must fail clearly.
- No live host installs, display changes, credential retrieval, firewall changes or cluster deployment merely because a work package exists. Work locally with mocks/fixtures until a specific integration task authorizes its exact targets.
- Never emit tokens, grants, clipboard text, PTY content or plaintext credentials into logs.
- Use appropriate maintained cryptography/codec libraries. Own the integration; do not invent crypto.
- Performance claims need reproducible measurements. M1 4K60 is a target on a declared capable baseline, not a claim about every device.
- Keep changes bounded; test meaningful invariants and failure paths. Do not report fixture conformance as end-to-end validation.
- Handoff: branch, base SHA, commit SHA, owned paths, tests/commands/results, contract version, limitations and next integration action.
- Do not push to main or merge another task's branch from a work package. Submit a reviewable branch/PR.
- Do not add licence boilerplate from another project. Our outbound licence is still awaiting an explicit choice.
