# Office3 packaged VKMS acceptance — 2026-09-06

Result: `http://192.168.69.19:8090` visibly serves the actual Rocky GDM greeter through VKMS/H.264 at1920x1080. Reviewed browser mouse selection opened an empty password prompt; Escape returned the user list and Release input returned idle. No credentials entered or authenticated session created.

Root integrator performed authorized service-only installation: release `0.1.0-lab.20260906.5-linux-amd64`, private encoder/source bundle at `/opt/donkeywork-desktop-codecs/ffmpeg-8.0.1`, VKMS/uinput loaded with boot-module persistence, scoped Mutter udev override, GDM-only restart c1→c2 after verifying no user desktop. Staging/vendor-udev backup retained at `/tmp/dwdesktop-office3.7xcXwS`. This acceptance lane made no host configuration changes.

## Evidence

- Active session c2: Wayland, greeter, UID42; unchanged throughout input test.
- HTTP status streaming H.264 Annex-B/libx264 from `/dev/dri/card1`, CRTC38,1920x1080, nominal30FPS. Browser960x640 decoded31frames before first screenshot.
- Reviewed source coordinate960,490: first click acquires, second click selects visible localuser. Empty password prompt and Escape return were visually inspected. Input release returned idle; no browser JS errors.
- Private screenshots: `/tmp/office3-display.png`, `/tmp/office3-click.png`, `/tmp/office3-escape.png`. UI interaction sample30.0FPS/58.4KB/s, not a benchmark.
- Guard state valid1920x1080 with c2 generation. Capture PID417942/input417967/web417946, all NRestarts=0.
- SELinux was already Permissive before this work and remains Permissive; no policy or mode change. Do not claim Enforcing acceptance for this host.
- k3s PID1576/start2026-09-06 12:20:31 IST unchanged.
- All four office nodes Ready after acceptance; API `/readyz` `ok`. Same five pre-existing failed pods only: three old node-debuggers, two kokoro-tts ErrImageNeverPull. No workload/cluster/firewall change or host reboot performed.

## Limits

This proves live greeter display and mouse/keyboard events through the browser. Boot persistence is configured, but reboot recovery and authenticated greeter-to-user handoff were not tested on office3. Output management in this profile depends on GNOME/Mutter.
