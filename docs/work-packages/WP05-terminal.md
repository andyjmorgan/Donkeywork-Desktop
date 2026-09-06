# WP05 — M1a daemon PTY; M1b browser terminal

## Outcome and ownership

Own `device/terminal/` and `browser-terminal/` on `work/wp05-terminal` in a separate worktree. M1a delivers daemon PTYs consumed by WP09's CLI through WP01. M1b adds xterm.js. Do not edit CLI, core, broker, web, contracts or root manifests.

## M1a scope

- Implement genuine Linux PTY creation, bytes, resize, exit, retention/resume and process-group cleanup according to the terminal contract and `contracts/local-cli.md` draft v0.2.0.
- Use the fixed server-configured account/profile selected by verified UID policy. No arbitrary root shell, client-selected identity or unrestricted profile/environment.
- Enforce terminal permissions and session/epoch binding at the worker boundary; no browser grants or .NET dependency.
- Preserve the running PTY across actual display resolution changes. Terminal size and desktop mode are independent.
- Implement ordered byte delivery, bounded output/history, explicit gaps and the approved disconnect/revoke policy. Never auto-replay ambiguous terminal input or log PTY content.

## M1b deferred scope

Build an importable xterm.js component for the same terminal semantics, then integrate broker authorization and React composition through their owners. Browser disposal and safe rendering are M1b acceptance. No browser work is required to complete the M1a daemon portion.

## Prerequisites and acceptance

Freeze local terminal authorization, framing, account profile, limits and retention semantics before merge. Request contract amendments for missing interfaces; do not claim an existing Rust API. WP01 and WP09 supply live transport/client integration; fixtures permit independent work first.

Test split UTF-8 and binary/control bytes, ordering, resize, high output/slow consumer limits, disconnect/resume gaps, repeated close and process-group cleanup. Denied UID/permissions and wrong session cannot access a PTY. Prove a PTY started before `3840×2160 → 1920×1080 → 3840×2160` remains the same running process and accepts input afterward.

## Boundaries and handoff

Original code only; no RustDesk copying/adaptation. No host installation, credential retrieval, production changes or MCP implementation. Provide branch, base/commit SHA, contract SHA/version, commands/results, environment, dependency provenance, limitations and APIs proposed/implemented for WP01/WP09. Mark local/fake versus integrated real PTY results. Record deferred browser work explicitly; do not mark it implemented.
