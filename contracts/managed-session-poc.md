# Managed-session PoC boundary — 2026-09-09

This is a deployment/test adapter, not a replacement for the versioned broker
contract. No new messages are added to `dwdesktop.local` 0.2.0.

## Authentication and identity

`deploy/managed/manage.py` validates the SSH host key using known_hosts and
authenticates with the supplied local-account password, explicitly disabling
agent/key fallback. Lab automation retrieves `lab-localuser` from dwvault in
memory. Passwords are not passed in arguments, stored in runtime files, or logged.
Both pilot SSH servers have UsePAM=yes. An SSH/PAM login starts a user-systemd
unit. **The graphical desktop is not itself a new PAM/logind login session**;
keyring unlock, desktop polkit integration, password-change/expiry UI, and
account-revocation teardown are not implemented by this adapter.

## Lifecycle

Updated logout policy: **never auto-create after logout**. Desktop process exit
retires the old instance and its input/PTY state. The independent portal reports
`session_ended` and presents an explicit start choice. `POST /api/session/connect`
with `{}` reuses the fixed account's existing private desktop or starts its
preinstalled service only after the user clicks. No credentials, account name,
display selector, executable or environment are accepted from that request.
The same unauthenticated-LAN limitation as viewing applies to this pilot endpoint.
No password is retained for recreation; the originally authorized account's
service policy permits this action. Future fleet policy/auth remains required.

- `create`: one named unit per account/host, fixed private X display :109.
  Refuse an active existing unit and any occupied X109 socket/lock. Start Xorg
  dummy, a private D-Bus/Xfce desktop, private tmux PTY, and existing Rust core.
  Wait for X11 and a window-manager root property before starting the core;
  creation returns only after a real core describe succeeds.
- `status`: systemd unit state. It is not proof of rendered application health.
- `cli`: password-authenticated SSH invocation of the existing local CLI against
  the session-user-owned Unix socket. `open/close` attach/detach the protocol;
  neither creates nor destroys the operating-system desktop.
- `terminal`: SSH PTY attachment to the private tmux server. Detach with Ctrl+B,
  D. The shell persists until session destruction or shell exit. This is not
  the unfinished native `dwdesktop term` transport.
- `destroy`: stop this unit and its entire cgroup. Private Xorg, desktop apps,
  core and PTY terminate; sockets disappear. Files/profiles/evidence remain.
  No physical-session service, host, cluster, or GDM restart is involved.

The CLI's existing attachment expiry remains independent of OS desktop lifetime.
New attachment IDs reconnect to the same managed desktop. Resize uses RandR in
the existing core; physical console resolution is never changed.

## Isolation and limits

Private Xauthority, config, cache, D-Bus and runtime directory prevent accidental
desktop-instance collisions. This is **not a sandbox from the same Unix UID**:
the account still has its normal filesystem/system-bus privileges. One pilot
session per host only; no production concurrent-create API, multi-user policy,
browser auth, credential broker, session quota or watchdog recovery is claimed.

Ubuntu + Xorg dummy + Xfce is the tested runtime. It does not depend on the
physical console being X11, Wayland, present, logged in, or captured. It is not
evidence of Rocky support or arbitrary Wayland-compositor compatibility.

The physical-console uinput helper is **not** redirected at this desktop.
`deploy/managed/web.py` now supplies the existing Go WebRTC bridge with a framed
private-X11 H.264 feed and a connected input adapter. The adapter verifies the
core socket owner/peer UID and translates browser events to the existing local
core session, lease, snapshot and topology operations. One-second browser input
expiry closes the core connection and releases held state. No uinput, root
process, logind seat guard or physical display FD participates.

`view` starts an independent persistent-in-memory portal service; `view-stop`
stops only that viewer. It binds the explicit pilot IP on HTTP 8095 and WebRTC
UDP 8096. **No web authentication is present. Any client able to reach this
pilot endpoint can request desktop control. It must remain on the trusted lab
network, never internet-exposed.** Exact Origin/Host checks are not authentication.

The optional `/api/resize` endpoint accepts only 1080p or 4K, invokes the private
core under the service UID, and never accepts a username, path or executable.
Input is released before resize. The adapter restarts only encoder/bridge on
mode changes and the UI reconnects automatically; the desktop and PTY persist.
This is not seamless decoder reconfiguration: a short media reconnect remains.
The CLI supports authenticated create/destroy. The trusted-LAN web portal now
also exposes the explicit fixed-account start choice; no web destroy endpoint.
