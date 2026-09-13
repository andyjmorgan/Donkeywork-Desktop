# DonkeyWork Desktop web console — local demo

Standalone React/TypeScript/Vite UI. This first slice is deliberately labelled
**local demo** throughout. It does not connect to Spark, query presence, perform
authentication, open a terminal or send desktop input. No broker endpoints or
browser-session APIs have been invented.

```sh
cd web
npm ci
npm run dev
```

Vite binds `127.0.0.1:5173` only by default. `npm run build` typechecks and builds
`dist/`; `npm run preview` serves that build on loopback. `npm test` validates the
pure preview state and coordinate mapping. Node.js 22+ is required.

## What works locally

- One labelled Spark fixture, with **Not connected** status rather than a fake
  online heartbeat; disabled registration/sign-in/terminal controls explain why.
- Open/close preview, browser fullscreen, native 1:1 scroll and aspect-preserving
  fit. The SVG test pattern is a local illustration, not a captured desktop.
- A separate supported **demo** resolution selector, pending state, simulated
  success/failure and preserved prior mode on failure. Changing browser scale
  never changes the source resolution. No actual host modes are changed.
- Local coordinate inspection, stale-transition suppression, cancel-on-close,
  keyboard focus styles, a mobile navigation drawer and light/dark theme toggle.
- Only the non-sensitive theme preference uses localStorage. No credentials,
  auth tokens, sessions or input payloads are stored or fetched.

## Visual provenance

Uses the user's DonkeyWork design language from DonkeyWork-Agents at
`6c5516867019ca9e96392aea75b09a3d095c3260`: its frontend `index.css`,
`components/layout/Sidebar.tsx`, `Header.tsx`, `ThemeToggle.tsx` and
`components/branding/Logo.tsx` were inspected for visual reference. The existing
user-owned `public/donkeywork.png` is reused as the logo. No Agents auth storage,
token/logout code or RustDesk UI/source/assets are reused.

The parent project records the Obsidian **DonkeyWork Design System — Dark Mode**
and **Light Mode** references. This implementation uses their background/text,
cyan accent and cyan-to-blue primary tokens, 256px sidebar, 56px header, 12px
button corners and 16px cards. Inter and JetBrains Mono are requested with local
system fallbacks; no external font service is contacted.

## Integration still required

Broker/BFF authentication, live device presence, enrollment, stream transport,
decoder rendering, authorized input, actual resolution changes and the browser
terminal all remain outstanding. The demo state in `src/model.ts` is UI-only,
not a transport contract. Future integrations must use reviewed API/package
contracts and replace the clearly labelled fixture; this build is not an M1
acceptance claim or a public deployment.
