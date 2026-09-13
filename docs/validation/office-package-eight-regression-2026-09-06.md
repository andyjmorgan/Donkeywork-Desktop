# Office fleet release .8 regression — 2026-09-06

office1, office2 and office3 upgraded sequentially from `0.1.0-lab.20260906.5` to `0.1.0-lab.20260906.8` using the amd64 archive, verified SHA256 `b963d78e94b7564888bafba186cc0e3e51775a53dd99190c914a6181c779d56d` and installer payload checksums.

Installer used existing VKMS profile/listen/encoder paths, with explicit `--control auto --replace-config --activate`; the only config addition was `CONTROL=auto`. Each previous config/unit set and release-link value was backed up before installation. Release .5 and earlier rollback artifacts retained. No GDM restart, reboot, package-manager, firewall, SELinux, module or workload changes were needed.

| Host / viewer | k3s PID unchanged | GDM PID unchanged | Before-.8 backup |
|---|---|---|---|
| office1 `http://192.168.69.27:8090` |1939266|772693|`/var/lib/donkeywork-before-eight.ay8v0A`|
| office2 `http://192.168.69.30:8090` |1783885|957516|`/var/lib/donkeywork-before-eight.c4h9My`|
| office3 `http://192.168.69.19:8090` |1576|416208|`/var/lib/donkeywork-before-eight.1v8wEl`|

Each host's active c2 Wayland greeter was checked before input. Real browser960x640 connected to1920x1080 H.264, acquired control, clicked visible localuser to show an empty password field, pressed Escape to restore the user list, and explicitly released control to idle. Click/Escape screenshots visually reviewed for every host; no credentials or login, no JS errors. This confirms descriptor change did not regress Rocky Wayland greeter input. Short UI samples were30FPS and roughly57–60KB/s, not benchmarks.

Screenshots reused the private earlier paths: `/tmp/office1-package-{click,escape}.png`, `/tmp/office2-{click,escape}.png`, `/tmp/office3-{click,escape}.png`; these now represent .8.

Cluster checked between sequential upgrades and after the final proof: all four office nodes Ready, `/readyz` returned `ok`, only same five pre-existing failed pods (three old node-debuggers, two kokoro-tts ErrImageNeverPull). SELinux remained Enforcing on office1/2 and pre-existing Permissive on office3.

No reboot recovery or authenticated greeter-to-user handoff claim is made by this regression.
