# Web console start — 2026-09-06

Andrew accepted the managed-desktop session startup and application-launch POC as sufficient to start the web UI. This changes sequencing, not the unverified engine acceptance results. Live CLI resize, Unicode insertion, integrated PTY and browser streaming remain tracked engineering work. The router certificate prompt was not a completed router-page test; Andrew elected to move on.

## First increment

Build the original React/TypeScript single-host console under `web/`, with a clearly labelled local demonstration adapter. Show one configured Spark device, not invented fleet enrollment or measured online health. Provide desktop workspace, fullscreen, fit/native viewing, and a separate simulated remote-mode selector. Unavailable terminal, live attachment and Keycloak controls must explain their state. No fake login or network-success fallback.

This increment validates layout and UI state handling, not remote access. Bind the development server to loopback. Do not publish it at rd.donkeywork.dev or expose the local UID socket over HTTP.

## DonkeyWork theme sources

Andrew explicitly requested existing DonkeyWork themes. Read on 2026-09-06:

- Obsidian Me: `Personal/Notes/DonkeyWork Design System - Dark Mode.md`.
- Obsidian Me: `Personal/Notes/DonkeyWork Design System - Light Mode.md`.
- `DonkeyWork-Agents/src/frontend/apps/web/src/index.css`.
- Agents `components/layout/Sidebar.tsx`, `Header.tsx`, and `components/branding/Logo.tsx`.

Use charcoal dark surfaces (#0a0d12, #0f1318, #151a21, #1a2028), cyan #22d3ee, cyan-to-blue primary actions, subtle borders, rounded 12px controls/16px cards, Inter body and JetBrains Mono code stacks. Light mode uses white/slate surfaces, slate text and darker cyan #0891b2. Carry the DonkeyWork logo and compact app chrome into a desktop-specific workspace. Record reused asset provenance. Do not import RustDesk visuals or code. Browser token storage and logout URL patterns in Agents are not this project's auth architecture.

## Next integration lanes

1. Freeze browser-facing HTTP/attachment and importable session-package contracts. Existing v0.1.0 messages are draft design, not implemented HTTP endpoints.
2. Implement the .NET 10 same-origin BFF with Keycloak code+PKCE, server-side sessions, secure cookies, CSRF and Origin enforcement. Development machine currently only has .NET 8; install a scoped .NET 10 SDK before broker implementation.
3. Implement trusted worker attachment authorization and a bounded media path over the existing canonical capture source. Do not simply wrap the CLI in an unauthenticated web server or mistake a service UID for the remote user's identity.
4. Connect the web viewer to real media/input and confirmed live resolution changes, then the real PTY. Keep demo and live adapters explicit; never silently substitute demo results after a worker error.
5. Validate Keycloak login/logout, failure/revocation cleanup, native 4K fidelity and active-session mode changes before fleet or public rollout.

Starting web work does not waive the live resize hard requirement. Viewer CSS zoom must never be reported as a host mode change.
