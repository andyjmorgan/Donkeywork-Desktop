# WP06 — M1b original DonkeyWork web console (deferred)

This package is deferred to M1b and is not an M1a dependency or initial dispatch lane. M1a's user/agent interface is WP09's local CLI. M1b adds the browser view around the same proven capture/input/PTY backend; it does not replace M1a's capture source.

## Outcome and ownership

Build the React single-host console. Own only `web/`; branch `work/wp06-web-console`, separate worktree. WP03 and WP05 own desktop/terminal packages; WP04 owns authentication and API behaviour.

## Scope

- Create an original DonkeyWork interface with Keycloak sign-in/out, local device capability/status display, desktop and terminal views.
- Integrate public browser-session/browser-terminal APIs and broker endpoints only after their M1b contract amendments are approved. The v0.2.0 local CLI profile is not a browser API, and the earlier v0.1.0 design does not implement these package APIs or HTTP endpoints. Once assigned for M1b, UI layout may proceed against clearly labelled local mocks.
- Present honest connection, authorization, capability, reconnect and error states. Provide fullscreen, monitor selection, scaling and supported quality controls without implying unavailable functionality.
- Keep desktop source resolution separate from CSS size and browser viewport. Make keyboard focus and release of remote input understandable and accessible.
- Add a remote desktop resolution selector populated from `availableResolutions`, separate from viewer zoom/scaling. Send `display.resize` only with `desktop.resize` permission and active control lease; show pending state, apply `display.resize.result` and updated topology/`streamGeneration`, and prevent stale input during the transition. Show failures and the retained/restored previous mode. Do not reconnect the desktop session or interrupt the terminal to change resolution.
- Use fixtures for early UI development, visibly marked as simulated.

M1b has one co-located device: no functioning enrollment workflow, fleet rollout or MCP UI. No RustDesk layout, assets, wording or copied source. Use original design and document dependency/asset provenance. No production changes.

## Prerequisites and blockers

Freeze API/auth model and package interfaces before merge. Missing shared decisions require a contract PR. WP03/WP04/WP05 implementations are required for real sessions. Mocked views may be reviewed visually but do not establish authentication, media or terminal functionality.

## Acceptance

- Component/browser tests cover signed-out, denied, offline, unsupported codec, reconnect and worker-error states using approved fixtures.
- Validate keyboard navigation, focus handling and responsive layout including a 4K source in smaller browser windows.
- Verify desktop/terminal disposal on navigation and logout; do not retain credentials or session grants in URLs/logs.
- Demonstrate original visual design and document which controls are actually connected.
- Prove the UI changes actual remote resolution `3840×2160 → 1920×1080 → 3840×2160` in one active session while the terminal survives. Test denied, unsupported and failed changes with visible feedback. Unsupported live mode changes on the pilot block M1b completion; CSS resize or video scaling cannot substitute. M1a establishes this device operation via CLI first.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, commands/results, browser/environment, safe screenshots, dependency/provenance notes, fixture-versus-live status and remaining blockers. Coordinate real login/logout and session validation with WP07. Do not claim real 4K quality from CSS layout or screenshot previews.
