#!/usr/bin/env bash
set -euo pipefail
umask 077
[[ $(id -un) == localuser && $EUID != 0 ]] || { echo 'Run as localuser'; exit 1; }
package_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
install -d -m 700 /home/localuser/.local/lib/dwdesktop /home/localuser/.local/state/dwdesktop-console /home/localuser/.config/autostart /home/localuser/.config/systemd/user
install -m 755 "$package_dir/session-agent" /home/localuser/.local/lib/dwdesktop/session-agent
install -m 755 "$package_dir/desktop-agent-start.sh" /home/localuser/.local/lib/dwdesktop/desktop-agent-start
install -m 644 "$package_dir/app-dev.donkeywork.Desktop.service" /home/localuser/.config/systemd/user/app-dev.donkeywork.Desktop.service
install -m 644 "$package_dir/dev.donkeywork.Desktop.desktop" /home/localuser/.config/autostart/dev.donkeywork.Desktop.desktop
if [[ -f /home/localuser/.local/state/dwdesktop-managed/broker.py ]]; then
 install -m 600 "$package_dir/console-socket.fleet" /home/localuser/.local/state/dwdesktop-managed/console-socket
fi
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user daemon-reload
echo 'Desktop autostart installed; no desktop or host restarted.'
