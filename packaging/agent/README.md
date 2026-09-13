# Device agent installer — 0.1.0-preview6

Manager preview: https://192.168.10.11:30443. Download the Linux package and
manager CA using the links on its Devices page. Package includes amd64 and arm64
Go binaries, an installer, systemd unit and SHA256SUMS. No GPU/display packages.

The preview manager uses a private CA. Verify its SHA256 fingerprint against
deploy/manager/README.md before trusting it. Pass it explicitly to the installer;
no insecure HTTPS bypass is implemented. Checksums detect corruption; release
signatures are not implemented yet. Obtain the archive through trusted HTTPS.

## Interactive

Extract the archive, then run `sudo ./install.sh`. It prompts for manager URL,
hidden enrollment code and optional CA path. Create a named device in the manager
first to get its 24-hour single-use code.

## Headless

```sh
sudo ./install.sh \
  --manager https://192.168.10.11:30443 \
  --code XXXX-XXXX \
  --ca-file /path/to/manager-ca.crt
```

The code argument is supported as requested, but visible in the installer's
process arguments and potentially shell history. For automation use a private
file: replace `--code XXXX-XXXX` with `--code-file /secure/enrollment-code`.
The script never logs the code and forwards argument-supplied codes to the
enrollment binary on stdin, not another command line. No prompts in headless mode.

## Installed footprint

- Dedicated unprivileged account: dwdesktop-agent.
- Private identity: /var/lib/dwdesktop-agent (0700), key/config/certificate 0600.
- Binary: /opt/donkeywork-desktop-agent/0.1.0-preview6/dwdesktop-agent.
- Stable symlink: /opt/donkeywork-desktop-agent/current.
- Unit: /etc/systemd/system/dwdesktop-agent.service, enabled at boot.

Installer needs root only to provision account/files/systemd. Service runs without
privileges or capabilities, with a read-only filesystem, private devices and
no display-manager interaction. It makes outbound HTTPS/mTLS connections only.
Requires Linux, systemd, useradd, sha256sum and tar; no Python dependency on-device.

Re-running setup keeps an existing enrolled identity, consumes no new code, and
restarts only this dedicated service. The initial key is retained if enrollment
fails. If a claim commits but its response/config write is lost, that code stays
dead: do not silently retry as a new identity or overwrite private state.

Use `systemctl status dwdesktop-agent` and `journalctl -u dwdesktop-agent` for
status. Stop/disable with `sudo systemctl disable --now dwdesktop-agent`.
Revocation closes the manager connection; it does not uninstall the host service.
The local service backs off and remains offline until operator recovery. Do not
delete another host's state or implicitly re-enroll after revocation.

## Scope and validation

Working: enrollment, key/certificate binding, outbound mTLS WebSocket,
15-second heartbeat, live online/offline, jittered reconnect and immediate
manager-driven revocation. Separate device endpoint: 192.168.10.11:30444.
Optional session IPC and gateway media are now implemented. Certificates expire
after 30 days; automatic renewal must land before sustained use.

## Existing managed-session pilot integration

The agent automatically detects `/run/dwdesktop-session/broker.sock`. It does
not install a desktop environment or create/login a desktop automatically.
On an already-provisioned localuser pilot, install
`deploy/managed/manager-bridge.tmpfiles.conf` as
`/etc/tmpfiles.d/dwdesktop-session.conf`, run `systemd-tmpfiles --create` for that
file, and launch the existing managed broker with
`--socket /run/dwdesktop-session/broker.sock`. `manage.py ... view` preserves
this integration when the runtime directory is provisioned. Other accounts
require an explicitly reviewed owner substitution in the tmpfiles definition.

The socket is mode0660 in a setgid directory owned by localuser:dwdesktop-agent.
No access to the account's private home or raw worker socket is granted to the
agent. Session lifecycle is still the existing configured-account pilot, not
multi-user password/PAM brokering. The managed broker is still transient;
automatic boot provisioning of that backend is not included in this package.

The manager relays browser WebRTC to the existing device connection. Browser
access requires manager HTTPS plus its advertised WebRTC UDP port; no direct
browser access to device media ports is needed. This does not remove the old
pilot's existing LAN listeners. Public access and TURN are not configured.

Both architectures now run on enrolled pilots: Easternkingdoms and Minigpu amd64,
Spark ARM64. Minigpu and Spark passed browser create/reconnect/input/resize/end
through the manager. Identity-preserving reinstallation was also tested on EK.
No host/GDM/LightDM restart was performed.
