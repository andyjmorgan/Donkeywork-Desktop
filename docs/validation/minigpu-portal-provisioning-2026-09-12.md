# Minigpu: administrator-provisioned Wayland portal capture

## Result

On 2026-09-12, an explicitly provisioned, app-scoped GNOME permission allowed
our unprivileged agent to capture the existing Minigpu Ubuntu Wayland desktop
without anyone accepting a consent dialog. A second process successfully reused
the portal's returned persistent token. This corrects the earlier claim that
administrator provisioning could not be done.

This proves a narrowly scoped GNOME backend bootstrap, not universal Wayland
provisioning, input delivery, greeter access, or a working manager console adapter.

## Target and evidence

- Minigpu: 192.168.69.21, localuser UID1000, active Wayland seat0 session214.
- xdg-desktop-portal 1.18.4-1ubuntu2.24.04.2; GNOME backend46.2-0ubuntu1.
- Actual monitor from Mutter DisplayConfig: Virtual-1, vendor/product/serial
  `unknown:unknown:unknown`, current1920×1080. No display configuration changed.
- Native app identity: `dev.donkeywork.Desktop`, derived by portal from user unit
  `app-dev.donkeywork.Desktop.service`. This is app scoping, not a security
  boundary against malicious code already running as the same desktop user.
- PermissionStore table `remote-desktop`: one fresh random UUID, only this app's
  permission. Backend restore format GNOME/version1, one actual monitor and
  keyboard+pointer permission; clipboard disabled.
- Capture uses standard RemoteDesktop/ScreenCast portals, restricted PipeWire FD
  and GStreamer. No unrestricted PipeWire, KMS or synthetic dialog acceptance.

First correctly encoded provisioned capture completed in426ms. New-process
restore capture completed in325ms. These are whole test process runtimes, not
stream latency or FPS measurements. Both PNGs were1920×1080, granted device mask3.
The first image was visually inspected: Ubuntu desktop, dock, Home icon and
system-report/update notifications, rather than a grey frame or consent dialog.

The initial experiment incorrectly nested a D-Bus variant and timed out after
30seconds. Correcting the wire encoding fixed it; do not omit this failed trial
when reproducing the work.

Revocation test: explicitly deleted only our saved permission, launched a new
process with the stale token, and got a5second timeout with no PNG. No one
accepted the fallback consent dialog. The portal requests/session were closed.
This tests future restore denial, not termination of an already-active share.

A new permission was then explicitly provisioned, and the final build again
captured1920×1080 successfully. Live evidence is private under
`/home/localuser/.local/state/dwdesktop-portal-test/` on Minigpu. `restore3` is the
current private token; never include its contents in logs or source control.
Older token files are retained as trial evidence and are not working grants.
Local inspected screenshot: `/tmp/dwdesktop-minigpu-portal-proof.png`.

## Reproduction and rollback

Run as the desktop user with its XDG_RUNTIME_DIR and session bus. The existing
test binary is `/tmp/dwdesktop-session-agent-preview`. It is not installed or
enabled at boot. Restore files require an owned0700 parent and are atomically
written0600. Only the explicit bootstrap flag provisions permission; ordinary
restore failure never silently grants new access.

```sh
systemd-run --user --unit=app-dev.donkeywork.Desktop --collect --wait --pipe \
  --property=RuntimeMaxSec=20s /tmp/dwdesktop-session-agent-preview share \
  --restore-file /home/localuser/.local/state/dwdesktop-portal-test/restore3 \
  --snapshot /home/localuser/.local/state/dwdesktop-portal-test/next.png \
  --timeout 10s
```

To revoke the remaining grant, run the binary's `revoke --restore-file` action
with that same private path. It checks that the record belongs exclusively to
our app before deleting it. End an active share separately; deletion of a restore
record is not an active-session kill switch. Do not delete the whole portal
permission store. No existing permission was overwritten, so no restoration of
other applications' records is required.

No host, display-manager or cluster restart; no package installation, autostart
change or managed-desktop replacement occurred during this test.

## Tests and remaining integration

`go test -race -count=10 ./desktopportal ./cmd/session-agent` and focused `go vet`
passed. The repeated tests exposed an existing asynchronous D-Bus NameAcquired
race; request waiting now ignores unrelated signals and checks the response's
unique portal owner as well as exact object path/member. Private-file tests
cover atomic replacement,0600 output and rejection of a public parent directory.

Still required: monitor discovery/ambiguity handling in the installer, backend
version compatibility tests, active-share revocation, crash-safe token lifecycle,
logout/login and portal-restart tests, actual input injection proof, H.264 stream,
local service registration and manager/browser integration. Provisioning must
remain an optional backend-specific operator action. A token's presence alone
must never be advertised as an available controllable console.

## References inspected (no upstream implementation copied)

- https://github.com/GNOME/xdg-desktop-portal-gnome/blob/46.2/src/remotedesktop.c
- https://github.com/GNOME/xdg-desktop-portal-gnome/blob/46.2/src/screencast.c
- https://github.com/GNOME/xdg-desktop-portal-gnome/blob/46.2/src/displaystatetracker.c
- https://github.com/flatpak/xdg-desktop-portal/blob/1.18.4/src/restore-token.c
- https://github.com/flatpak/xdg-desktop-portal/blob/1.18.4/src/xdp-utils.c
