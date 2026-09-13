# Managed desktop extension: Easternkingdoms and Rocky

## Easternkingdoms

Host `192.168.69.17`, Ubuntu 24.04 amd64. Broker URL:
`http://192.168.69.17:8095/?managed` (trusted LAN, no authentication).

Added only the missing distro `xserver-xorg-video-dummy` package and the private
managed runtime/broker under localuser. No reboot or display-manager restart.
Physical LightDM PID 2217 and its July 2 activation timestamp remained unchanged.
The temporary legacy bootstrap desktop on :109 was stopped; broker-created
desktops use independent displays and units.

The initial Terminal search selected Byobu and attached to the user's existing
tmux workspace. The test typed its `echo gnomewebok` string there. This was
disclosed immediately and was not accepted as isolated terminal proof. The
existing tmux server PID 10902 remained alive. Managed runtime now sets
`TMUX_TMPDIR` to its private instance root and `BYOBU_DISABLE=1`; no user shell
configuration was edited. This change applies to newly created sessions.

An attempted Alt+F2 launch also exposed the alpha's unsupported function-key
mapping. It was not counted as successful application input. The EK browser
test instead launches Text Editor via Activities for visual typing proof.

The resulting screenshot visibly shows `echo gnomewebok` in Text Editor (text
input, not a shell command). Browser tests passed concurrent GNOME/Xfce,
1080p → 4K → 1080p for each, individual reconnect, and ending Xfce without
replacing GNOME. Native CLI describe succeeded against the selected GNOME ID.
Nine Python broker/adapter tests passed. Evidence is under
`artifacts/managed-broker-proof/192.168.69.17/`.

The two-GNOME concurrency test also passed: both sessions survived beyond the
startup timeout, duplicate create was idempotent, normal GNOME logout retired
only the test desktop, and the original PID survived without automatic creation.
One GNOME desktop remains: `22267530ec9b4269b87fb2c4871cab79`.

## Rocky office1 preflight — blocked before deployment

Host `192.168.69.27`, Rocky 10.0, GNOME Shell 47.4, GNOME Session 46.
Only Wayland session definitions are installed; `/usr/share/xsessions` is empty.
Neither Xorg nor TigerVNC server is installed. The available-package query
returned Xwayland but neither Xorg server nor TigerVNC server from enabled
distro repositories. No desktop packages or display configuration were changed.
K3s/GDM stayed running and SELinux stayed Enforcing.

The unrelated Datadog repository's metadata signature check initially failed;
the package query was rerun with only that repository disabled for the command.
No repository configuration, keys or signature-verification policy was changed.

Current managed capture/input uses a private Xorg dummy display, X11 capture,
XTEST input and RandR mode changes. Xwayland alone does not supply that desktop
server. Rocky's `gnome-shell --help` advertises headless/virtual-monitor support,
but our daemon has no corresponding Wayland managed capture/input/resize adapter.
Therefore this is not a supported package-only rollout to Rocky. The forward
path is a separate headless-Wayland adapter; no console/VKMS workaround was used.
