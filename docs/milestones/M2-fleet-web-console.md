# M2 — Authenticated fleet web console

Status: next milestone (planned 2026-09-06)

## Outcome

Provide one DonkeyWork web console for the current lab fleet. An operator signs
in with Keycloak OAuth/OIDC, sees enrolled devices and their live capability /
online state, and can create, view, control, and destroy a console session.
The browser uses the existing `.8` daemon/media/input path; M2 does not replace
the proven device implementation.

## First release slice

1. OAuth authorization-code + PKCE login, server-side session, logout and
   expiry handling.
2. Explicit device enrollment with a persistent identity, name, address,
   profile, capabilities and revocation state.
3. Heartbeat/health reporting and honest online, stale, unavailable and
   unsupported states.
4. Fleet list/detail view; create one session; display H.264; acquire/release
   keyboard and mouse control; destroy the session.
5. Authorization for `desktop.view`, `desktop.control`, `desktop.resize` and
   terminal access, enforced by the broker and again by the daemon.
6. Active-session resolution selection using the real source mode, preserving
   session and PTY identity while resetting topology/stream generation.

## Boundary and deployment

The .NET broker is the only browser-facing control plane. It validates issuer,
audience, signature, expiry, CSRF and Origin; issues short-lived one-use
attachment grants; and talks to each daemon over an authenticated channel.
Daemons never trust browser-supplied identity and never expose privileged IPC.
The first deployment may be co-located for development; the fleet broker is
intended for the attic cluster after local acceptance. Keep access internal or
through UniFi VPN while this milestone is developed. Cloudflare Tunnel and
static public IP publication are deployment options, not M2 acceptance.

### Security invariants

- Use a dedicated Keycloak client and audience. Browser login is
  authorization-code + PKCE with state/nonce validation, server-side session
  state and Secure/HttpOnly/SameSite cookies. Validate issuer, signature,
  audience and expiry; `azp` is not an audience check.
- Protect state changes with CSRF defence and enforce `Origin` on WebSocket
  upgrades. Keep bearer tokens, attachment grants and enrollment secrets out
  of URLs and logs.
- Use short-lived opaque one-use attachment grants bound to principal, device,
  session epoch and permissions; store only their hashes. Revoke promptly and
  fail closed when renewal is missed.
- Keep permissions separate: `device.view`, `desktop.view`,
  `desktop.control`, `desktop.resize` and `terminal.attach`. Viewing never
  implies control. The daemon rechecks the broker's authorization at its own
  boundary.
- Enrollment is explicit and revocable: a short-lived bootstrap action creates
  a persistent per-device identity/key. A device cannot claim another device's
  identity. Audit principal, device, operation and outcome, but never pixels,
  clipboard, terminal bytes, credentials, grants or raw input payloads.

### Device and session model

The registry is the source of fleet presence, not a UI guess. Each device
reports `lastSeen`, daemon version, profile, display topology, supported modes,
codec and control state. The UI distinguishes online, stale, offline, disabled
and incompatible. Heartbeat loss releases input and causes attachments to fail
closed.

The browser-facing API should define, before implementation, explicit
resources for device enrollment/list/get/revoke, heartbeat/capabilities,
session create/get/destroy, attachment prepare/redeem/renew/revoke, and desktop
stream/input/release/resize. WebSocket authorization happens before worker
IPC. Media remains opaque H.264 access units at the broker; a direct media
route needs its own authenticated binding and contract review.

### DonkeyWork UI direction

Use Obsidian Me's `Personal/Notes/DonkeyWork Design System - Dark Mode.md` and
`Light Mode.md`, plus DonkeyWork Agents' frontend CSS and Sidebar/Header/Logo
components, as design references. Carry the visual principles rather than
copying implementation or assets: charcoal/slate surfaces, cyan-to-blue
actions, subtle borders, rounded controls/cards, Inter body text and JetBrains
Mono for technical values. The main screen is a device list with capability
summaries and a desktop workspace. Connection, stale-device, stream recovery,
control lease and resize states must be visible. Source resolution is always
separate from viewer fit/zoom.

## Work breakdown

- WP04: broker, Keycloak integration, enrollment, heartbeat, policy and grants.
- WP06: original DonkeyWork fleet UI and session lifecycle.
- WP03: browser media/session adapter and stream reconfiguration.
- WP05: browser terminal integration and resize semantics.
- WP07: security, performance, failure and end-to-end acceptance.

## Acceptance gate

The milestone is complete only when a real Keycloak user can perform the full
flow against at least two enrolled devices, including display, input, release,
logout/revocation and active resolution change. Test wrong issuer/audience,
expired and replayed grants, unauthorized devices/actions, stale heartbeats,
daemon loss, reconnect and browser focus loss. No credentials, grants,
clipboard contents or terminal payloads may enter logs or URLs. Record native
source dimensions, codec, frame pacing, bandwidth and measured input latency.

## Explicitly out of scope

Automatic physical-display/VKMS hotplug (see [WP10](../work-packages/WP10-display-hotplug.md)),
public internet exposure, fleet-wide reboot orchestration, MCP/computer-use
pods, clipboard automation and authenticated greeter handoff. These follow
only after M2 security and reliability acceptance.

## Relationship to surrounding work

The fixed-profile `.8` package is the current device-plane alpha: six hosts
have verified 1080p H.264 display and real input. The existing local web
prototype is an unauthenticated single-device development adapter, not a fleet
endpoint. WP10 hotplug is queued and unimplemented. MCP/computer-use pods,
Vault-backed credential paste and agent-specific desktop orchestration are
future work (M3/M4); M2 must not grow those privileges by implication.
