# Single-host managed desktop broker — pilot

This pilot supersedes the singleton web lifecycle in managed-session-poc.md.
It is not the authenticated fleet broker contract. HTTP access is restricted
operationally to the trusted LAN; no web identity or account selection exists.

## Inventory and explicit choice

- `GET /api/environments` returns `environments[]` (supported launch adapters)
  and `installedSessions[]` (discovered X11/Wayland desktop entries). Discovery
  does not imply that an arbitrary desktop entry can be launched safely.
- `GET /api/desktops` returns `desktops[]` and `history[]`. Each desktop has
  `id`, `environment`, `user`, `createdAt`, and `state`.
- `POST /api/desktops` accepts exactly `{environment, requestId}`. Creation is
  explicit even when no desktops exist. Existing desktops do not prevent
  creation of another, subject to the four-desktop pilot limit.
- The same request ID and environment reuse the original result. A conflicting
  environment is rejected. IDs are opaque and never reused after logout.
- `POST /api/desktops/<id>/reconnect` checks readiness; it never creates a desktop.
- `POST /api/desktops/<id>/close` ends only the selected desktop and its apps.
- `status`, `offer`, and `resize` routes beneath a desktop ID target that desktop.

The UI always offers environment selection and Create desktop, alongside an
array of existing desktops with individual Reconnect and End desktop actions.
Logout returns to a choice, not automatic replacement. Closing a viewer leaves
the desktop running. Ending a desktop does not stop the broker or its siblings.

## Runtime boundaries

GNOME (Ubuntu) and Xfce use private Xorg dummy displays. Each instance owns its
Xauthority, D-Bus, runtime/config/cache directories, daemon socket, tmux socket,
systemd unit and media ports. The physical desktop may independently use Wayland.
This does not implement capture of a Wayland physical desktop.

Instances run as the same authenticated pilot Unix account. They share its home
and file permissions; these are not mutually untrusted security sandboxes.
Hardware/seat-specific desktop features and browser singleton behaviour require
additional validation. Installed and end-to-end validated are distinct states.

CLI targeting: `manage.py --host HOST --vault --desktop ID cli describe` selects
the instance socket. The same selector supports `terminal`, `status`, `destroy`.
`terminal` remains SSH PTY/tmux, not the proposed native terminal protocol.
The older unselected `create` command remains a legacy singleton launcher;
use the broker UI to create concurrent desktops.

Ports: broker TCP 8095; private bridge TCP 8100–8115, WebRTC UDP 8200–8215,
allocated with displays 110–125. No public exposure is authorized by this pilot.
