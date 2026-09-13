# DonkeyWork Desktop console UI

This directory is the authoritative React/TypeScript source for the live
single-host console. Build it from this repository; no sibling worktree is
required. The repository's older `web/` directory remains a separate legacy
demo and must not be deployed over these assets.

## Build and test

Node.js 22+ and npm are required. From the repository root:

```sh
npm --prefix console-ui ci
npm --prefix console-ui test
npm --prefix console-ui run build
```

The production output is `console-ui/dist/`. Package that directory with the
Rust console components and the Go bridge in `console-web/`. Configure the
bridge's `--assets` argument to the installed static-assets directory. The
browser uses same-origin `/api/status` and `/api/offer`; serve the assets through
the bridge, not a separate unconfigured static server. Node/npm are build-time
dependencies, not requirements on the deployed host.

For layout development, `npm --prefix console-ui run dev` binds Vite to
`127.0.0.1:5173`. It does not proxy the console API; a standalone Vite page is
not a live-console deployment. The package build/install entrypoints are owned
by the repository's deployment tooling; this directory supplies their assets.

## Current runtime

- Native H.264 WebRTC video, automatic initial connection and reconnection on
  actual transport failure, fullscreen, aspect-preserving fit and light/dark
  themes. The footer reports decoded FPS and received RTP video payload B/s.
- Capture-only recovery preserves the PeerConnection and last decoded frame.
  The status says it is waiting; advancing decoded frames restores live state.
  Input is released during the interruption and never automatically replayed
  or reacquired. Displayed last frames are not evidence of live capture.
- Explicit keyboard/mouse acquisition on focused video. The first acquisition
  click is not injected. One ordered reliable data channel, one outstanding
  acknowledgement, bounded queues/timeouts, and release on focus/capture loss.
  Ctrl+Alt+Escape releases control. The daemon owns final held-state cleanup.
- Single-output absolute mouse mapping handles contain-fit bars and captured
  drags. Physical keyboard codes follow the remote layout; the host owns repeat.
  Hosts without input support remain view only.

Clipboard, touch/pen, relative pointer, Meta/AltGraph/IME input, and
arbitrary live host-resolution changes are not implemented in this alpha.
Browser/OS-reserved shortcuts cannot be guaranteed. This is an internal-network
prototype without fleet authentication or authorization UI; do not expose it
publicly. CSS scaling does not change the host display mode.

## Contracts and evidence

- [Physical-console stream and browser adapter](../contracts/console-web-prototype.md)
- [Input protocol 0.2.0](../contracts/console-input-web.md)
- [Same-peer capture recovery](../contracts/capture-recovery.md)

Vitest covers geometry, input ownership/ordering/deadlines, recovery decisions,
and the retained demo model. TypeScript and Vite validate the production build.
These checks do not replace real browser/daemon, greeter/login or host tests.
`src/DemoApp.tsx` and `src/model.ts` are retained reference/demo code; the live
entrypoint `src/main.tsx` imports `App.tsx`, not `DemoApp.tsx`.

## Source and visual provenance

Consolidated from the exact live `desktop-console-video/web` source, lockfile,
configuration, tests and assets on 2026-09-06, including its uncommitted runtime
integration changes. Future runtime edits belong here, not in that sibling.

The original UI follows the user's DonkeyWork design language and the Obsidian
DonkeyWork Design System light/dark references. DonkeyWork-Agents at
`6c5516867019ca9e96392aea75b09a3d095c3260` was inspected for theme/layout
reference. The user-owned `public/donkeywork.png` logo is reused unchanged.
No RustDesk or JetKVM UI assets/source, or Agents authentication/token code,
are imported. Fonts use local system fallbacks; no external font service is
contacted.
