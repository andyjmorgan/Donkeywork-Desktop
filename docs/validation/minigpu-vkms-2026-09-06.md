# Minigpu monitorless VKMS greeter proof

2026-09-06: actual GDM Wayland greeter captured at 1920x1080 without a
physical monitor, dummy plug, or separately created desktop. Host192.168.69.21,
Ubuntu24.04, kernel6.8.0-137-generic, GNOME46.

Loaded existing kernel module `vkms`; appeared as card2/Virtual-1. Ubuntu
`/usr/lib/udev/rules.d/61-mutter.rules` explicitly tags VKMS
`mutter-device-ignore`. A late TAG removal cleared CURRENT_TAGS but not TAGS,
which Mutter examines. To remove the exclusion, installed a local same-name
copy `/etc/udev/rules.d/61-mutter.rules` retaining every other vendor rule.
Added `/etc/udev/rules.d/99-dwdesktop-vkms.rules` to prefer VKMS. Recreated only
the VKMS module and restarted minigpu GDM. No machine reboot or AMD driver
changes. No easternkingdoms changes.

Mutter explicitly logged card2 selected primary given udev rule. Framebuffer
owner became gnome-shell, XR24 linear. Applied advertised1920x1080@60.000 mode
through GDM's private DisplayConfig D-Bus using temporary method1.
Independent libdrmtap0.5.4 screenshot example captured CRTC37/plane31.
Local private evidence: artifacts/minigpu-vkms-greeter.ppm and reduced preview.
Visual inspection shows Ubuntu GDM localuser selection and current clock.

Initially not proven: dynamic picture changes, streaming, login/logout, lock,
reboot persistence, CPU cost. Module autoload and persistent1080 mode are NOT
configured. This is a successful still-capture proof, not fleet acceptance.

Rollback: stop any capture; move the two task-created /etc/udev/rules.d files
out of the rules directory; reload udev rules; restart minigpu GDM so it drops
VKMS; unload vkms when unused. Vendor rule was never edited. Initial redundant
rule was moved to /tmp/dwdesktop-vkms-first-rule.backup. The local vendor-copy
override needs package-update maintenance if retained.

## H.264 web viewer and idle recovery — 18:28 UTC

Installed capture daemon, root-dropping launcher, Go bridge and viewer at
http://192.168.69.21:8090. Browser smoke test decoded 1920x1080 H.264,
16 -> 47 frames with zero reported drops. Actual browser screenshot inspected:
GDM localuser selection, Ubuntu logo and 18:27 clock visible. Evidence:
`artifacts/console-web/live-view.png` (overwritten by later smoke runs).

The original greeter idled: PowerSaveMode=3 and ScreenSaver active. An idle
inhibitor alone did not recover it. Calling ScreenSaver.SetActive(false) was
followed by a top-bar-only picture; removed that call from the helper and
restarted minigpu GDM to restore the full greeter. No machine reboot.
GDM's persistent org.gnome.desktop.session idle-delay was changed from 300
to 0 during diagnosis; rollback is to set uint32 300 as gdm on its current
private session bus. This pilot also holds a session idle inhibitor. Neither
is a validated viewer-scoped wake/idle lifecycle yet.

Three transient services dwconsole-vkms-{output,capture,web} currently run;
web restarts on capture EOF (verified during initial mode change). Private
GDM bus is discovered from gnome-shell environment and changes on restart.
Services are not boot-persistent. Repeated idle, login/logout, lock and reboot
acceptance remains outstanding. Advancing decoded frames alone does not prove
fresh desktop content. Easternkingdoms was not modified or restarted.
