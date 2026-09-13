# Accepted POC session model — 2026-09-06

Andrew approved a dedicated local user for the POC after discussing console access, headless sessions and future username/password selection. This adopts Model B in [Fable's display/login note](reviews/2026-09-06-display-model-and-login-note.md).

## Now: one service-managed desktop

- Dedicated locked-password, non-sudo local account `dwdesktop` on Spark. Desktop, test applications, daemon and CLI run as that UID; do not reuse localuser's browser/profile or root identity.
- One Xorg dummy-driver display with genuine 3840×2160 and 1920×1080 RandR modes. Lightweight window manager; no physical seat, GDM, autologin or existing Wayland session changes.
- Start under system service supervision with a private Xauthority cookie and Unix sockets, X11 TCP disabled. No human needs to log into the physical console. An administrator invokes the CLI as the dedicated account for this same-UID alpha; the product does not grant arbitrary identity switching.
- The POC proves a managed virtual desktop, capture/input/resize protocol and eventually PTY continuity. It does not prove Spark's physical-console readback/mode changes, native Wayland access or GPU/browser streaming performance.
- Fable calls this proof M0; the existing GitHub M1a ledger remains the implementation ledger. Neither label should imply the deferred physical-console path passed.

## Later: elect a user

Discussed accepting username/password to create a managed session as a selected OS user. Deferred until protocol and web UI proof. A future minimal privileged launcher needs PAM authentication/account checks/open_session, logind/session lifecycle, fixed identity policy, credential-safe input, MFA/password-expiry handling and cleanup. Keycloak sign-in alone does not authorize becoming an arbitrary OS user.

Create managed desktop, reconnect managed desktop and attach physical console are separate operations. Credentials alone do not solve Wayland capture/input consent. Concurrent desktop sessions under one UID need an explicit reuse/reject/isolation policy because D-Bus, keyrings and application profiles can conflict. No credentials stored in argv, logs or plaintext config.

The POC's fixed service identity is not an implementation of PAM password authentication. Keep display/session-facing code behind the backend seam; future authenticated session creation requires design and tests, not merely swapping a username argument. Do not add speculative login/recording fields to the current protocol.
