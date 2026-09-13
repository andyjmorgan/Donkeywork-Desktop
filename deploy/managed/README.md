# Managed desktop pilot

Ubuntu-only proof on Spark `192.168.69.28` (ARM64) and Minigpu
`192.168.69.21` (amd64), running as the password-authenticated `localuser`.
Read [the lifecycle/security boundary](../../contracts/managed-session-poc.md).

## Current web broker

Use [Spark](http://192.168.69.28:8095/?managed) or
[Minigpu](http://192.168.69.21:8095/?managed). These are unauthenticated LAN URLs.
Easternkingdoms is also deployed at [192.168.69.17](http://192.168.69.17:8095/?managed).
Rocky 10 is not supported by this X11 adapter; see the
[host extension findings](../../docs/validation/managed-ek-rocky-2026-09-09.md).
The broker offers GNOME (Ubuntu) and Xfce, a list of existing desktops, Reconnect
per desktop, and Create desktop even while others exist. Logout never creates
a replacement automatically. See [the multi-desktop contract](../../contracts/managed-broker-pilot.md).

GNOME requires `gnome-session`, Ubuntu's session definition and `gnome-shell`
with X11 support, in addition to the Xorg prerequisites below. We launch a
private X11 desktop; the physical desktop may stay on Wayland. Per-instance
launch overrides are private files, not changes to distro desktop entries.

Use a broker ID to select the CLI/terminal target:

```sh
python3 deploy/managed/manage.py --host 192.168.69.21 --vault --desktop ID cli describe
python3 deploy/managed/manage.py --host 192.168.69.21 --vault --desktop ID terminal
python3 deploy/managed/manage.py --host 192.168.69.21 --vault --desktop ID destroy
```

The unselected create/status/destroy examples below refer to the **legacy
singleton**, not desktops created in the broker. Use the web UI for new desktop
creation. `view` deploys/restarts the broker, without ending managed desktops.

## Prerequisites

On the controller: Python 3, Paramiko, dwvault, trusted SSH known_hosts entries.
On the pilot: password SSH with PAM, a running user systemd manager, Xorg,
`xserver-xorg-video-dummy`, `xfce4-session`, `xfwm4`, `xfce4-panel`, `xfdesktop4`,
`xfce4-settings`, `xfce4-terminal`, `dbus-x11`, `xauth`, `x11-utils`,
`x11-xserver-utils`, `tmux`, and `ffmpeg` with libx264.
Use `apt-get install --no-install-recommends` to avoid pulling another display
manager and physical-seat locking software. Do not change the system's DM.

Build existing `device/core` and `cli` using `cargo build --release --locked
--manifest-path .../Cargo.toml`. The adapter uploads the local amd64 binaries to
Minigpu and uses Spark's native builds in its existing checkout. Source/build
synchronization is an explicit operator prerequisite, not hidden cross-compiling.

## Commands (repository root)

```sh
python3 deploy/managed/manage.py --host 192.168.69.28 --vault create
python3 deploy/managed/manage.py --host 192.168.69.28 --vault status
python3 deploy/managed/manage.py --host 192.168.69.28 --vault cli describe
python3 deploy/managed/manage.py --host 192.168.69.28 --vault terminal
python3 deploy/managed/manage.py --host 192.168.69.28 --vault view
python3 deploy/managed/verify.py 192.168.69.28 --browser
python3 deploy/managed/manage.py --host 192.168.69.28 --vault destroy
```

Substitute `.21` for Minigpu. Omit `--vault` for an interactive password prompt.
Terminal detach: Ctrl+B, then D. `cli text` reads text from stdin. Desktop CLI
contexts/snapshots/output paths are on the target host, not the controller.
See [CLI commands](../../cli/README.md). `exec` is an explicit diagnostic command
escape hatch; it is not used as evidence of desktop clicks or keystrokes.

`verify.py` creates fresh private evidence paths, clicks the desktop terminal
launcher, types a non-secret diagnostic command, optionally launches a browser
through that terminal, resizes 1080p→4K→1080p, detaches/reattaches, and encodes
three seconds of H.264 and checks all 90 frames decode. Screenshots must still
be visually inspected: protocol ACK is not proof of application behavior.
The test assumes the default pilot Xfce dock geometry and is not a general
computer-use agent. It does not destroy the desktop automatically.

## Runtime and troubleshooting

Unit: `dwdesktop-managed-poc.service` in the account's user manager.
Runtime: `/home/localuser/.local/state/dwdesktop-managed`, private mode 0700.
Display `:109`; no physical GPU or input devices auto-added by Xorg.
Logs: `desktop.log`, `xorg.log`, `Xorg.log`, `core.log`, and user journal.
The unit is transient, not boot-enabled. Destroy retains configs, browser
profile and evidence for inspection; it kills only the managed process tree.
Saved Xfce settings can survive recreation. This is intentional account state,
not an assertion that destroy wipes user data.

System autostart entries are masked in this desktop's private config. A physical
seat locker caused a real crash during the initial test; masking it prevents
that component from being started here without changing console autostart.

Minigpu's Snap Firefox fails against the private bus/cgroup. Spark reuses its
existing `/opt/firefox/firefox`. Minigpu uses Mozilla's unmodified native
Firefox 155.0.1 under the managed directory, separate from the installed Snap.
No sandbox-disabling Firefox flags are used. Each host uses a dedicated
`browser-profile` and `--no-remote` to prevent attaching to the console browser.
Minigpu tarball source:
`https://download-installer.cdn.mozilla.net/pub/firefox/releases/155.0.1/linux-x86_64/en-US/firefox-155.0.1.tar.xz`
SHA256 (matched Mozilla's published SHA256SUMS over HTTPS):
`642ab731354a5ca790b894d4556dfb5028c61d0c24eb10d10e10a111a69c89bf`.
This pilot download is not an automatic browser update mechanism.

## Managed web viewer

Logout behavior (updated): the portal remains online and asks **Start new
desktop?** It does not recreate an OS desktop until that button is clicked.
If a desktop already exists, reconnect reuses it. The desktop unit has
`Restart=no` and a persistent user-unit definition, but is not boot-enabled.
The portal's independent supervisor recovers video/input adapters only—not
logged-out desktops. The explicit start button runs the fixed local-user unit;
it accepts no account, password, command or path. This is still an unauthenticated
trusted-LAN pilot. CLI `create` likewise reuses a live instance.

- Spark: `http://192.168.69.28:8095/?managed`
- Minigpu: `http://192.168.69.21:8095/?managed`

**Trusted LAN only, no web authentication yet.** Network reachability currently
permits desktop control. Do not publish these endpoints through a tunnel or
public port-forward. Existing physical-console web URLs remain separate.

Build the bridge from `console-web/`:

```sh
go build -o ../artifacts/managed-web/amd64/dwconsole-web .
GOARCH=arm64 go build -o ../artifacts/managed-web/arm64/dwconsole-web .
```

Build assets with `npm --prefix console-ui run build` from the repository root,
then run `manage.py ... view`. `view-stop` stops just the web service. The portal
stays up after desktop logout; explicit CLI destroy stops both. The portal is transient,
not boot-enabled. No physical-console service or binary is replaced.

Video is H.264/WebRTC at 30 FPS, not a PNG loop. The existing keyboard, absolute
pointer, wheel, scaling, focus/release and single-controller behavior is reused.
First click acquires; subsequent clicks go to the desktop. 1080p/4K buttons
change the actual private desktop mode. Media reconnects automatically after
resize; apps and PTYs remain alive. The UI shows measured video FPS and payload
bytes/sec. Native terminal access remains the authenticated CLI/tmux path.

Live browser test (real Chromium, not an HTTP-health assertion):

```sh
node tests/integration/managed-web-pilot.mjs 192.168.69.21
node tests/integration/managed-web-pilot.mjs 192.168.69.28
```

Evidence is retained under `artifacts/managed-web-proof/HOST/`. Review screenshots
as well as JSON results. Unit checks: `python3 -m unittest discover -s
deploy/managed -p 'test_*.py'`, existing Rust core tests, `go test ./...` in
console-web, root `npm test`, and console-ui's tests.
