# WP06 — Original DonkeyWork web console

## Outcome and ownership

Build the React single-host console. Own only `web/`; branch `work/wp06-web-console`, separate worktree. WP03 and WP05 own desktop/terminal packages; WP04 owns authentication and API behaviour.

## Scope

- Create an original DonkeyWork interface with Keycloak sign-in/out, local device capability/status display, desktop and terminal views.
- Integrate public browser-session/browser-terminal APIs and broker endpoints only after their contract amendments are approved. The current v0.1.0 draft defines wire messages, not yet those package APIs or HTTP endpoints; UI layout may proceed against clearly labelled local mocks.
- Present honest connection, authorization, capability, reconnect and error states. Provide fullscreen, monitor selection, scaling and supported quality controls without implying unavailable functionality.
- Keep desktop source resolution separate from CSS size and browser viewport. Make keyboard focus and release of remote input understandable and accessible.
- Add a remote desktop resolution selector populated from `availableResolutions`, separate from viewer zoom/scaling. Send `display.resize` only with `desktop.resize` permission and active control lease; show pending state, apply `display.resize.result` and updated topology/`streamGeneration`, and prevent stale input during the transition. Show failures and the retained/restored previous mode. Do not reconnect the desktop session or interrupt the terminal to change resolution.
- Use fixtures for early UI development, visibly marked as simulated.

M1 has one co-located device: no functioning enrollment workflow, fleet rollout or MCP UI. No RustDesk layout, assets, wording or copied source. Use original design and document dependency/asset provenance. No production changes.

## Prerequisites and blockers

Freeze API/auth model and package interfaces before merge. Missing shared decisions require a contract PR. WP03/WP04/WP05 implementations are required for real sessions. Mocked views may be reviewed visually but do not establish authentication, media or terminal functionality.

## Acceptance

- Component/browser tests cover signed-out, denied, offline, unsupported codec, reconnect and worker-error states using approved fixtures.
- Validate keyboard navigation, focus handling and responsive layout including a 4K source in smaller browser windows.
- Verify desktop/terminal disposal on navigation and logout; do not retain credentials or session grants in URLs/logs.
- Demonstrate original visual design and document which controls are actually connected.
- Prove the UI changes actual remote resolution `3840×2160 → 1920×1080 → 3840×2160` in one active session while the terminal survives. Test denied, unsupported and failed changes with visible feedback. Unsupported live mode changes on the pilot block M1 completion; CSS resize or video scaling cannot substitute.

## Handoff

Provide branch, commit SHA, contract-baseline SHA, commands/results, browser/environment, safe screenshots, dependency/provenance notes, fixture-versus-live status and remaining blockers. Coordinate real login/logout and session validation with WP07. Do not claim real 4K quality from CSS layout or screenshot previews.
