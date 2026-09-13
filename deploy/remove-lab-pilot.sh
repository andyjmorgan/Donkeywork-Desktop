#!/usr/bin/env bash
# Explicit retirement of this project's lab installs; never removes host desktops.
set -euo pipefail
case "${1:-}" in
user)
  test "$(id -un)" = localuser
  mapfile -t units < <(systemctl --user list-units --all --plain --no-legend | awk '{print $1}' | grep -E '^(dwdesktop-|dwconsole-|app-dev\.donkeywork\.Desktop).*\.service$' || true)
  for unit in "${units[@]}"; do systemctl --user stop "$unit"; done
  if test -x /home/localuser/.local/lib/dwdesktop/session-agent && test -f /home/localuser/.local/state/dwdesktop-console/restore; then
    /home/localuser/.local/lib/dwdesktop/session-agent revoke --restore-file /home/localuser/.local/state/dwdesktop-console/restore || echo 'NOTE: grant absent or revoke unavailable; review permission store'
  fi
  ;;
root)
  test "$(id -u)" = 0
  archive=$(mktemp -d /var/lib/dwdesktop-removal-20260913.XXXXXX)
  chmod 700 "$archive"
  echo "Recovery archive: $archive"
  move_path() {
    local path="$1"
    test -e "$path" || test -L "$path" || return 0
    mkdir -p "$archive$(dirname "$path")"
    mv -- "$path" "$archive$path"
    echo "Removed from install: $path"
  }
  mapfile -t units < <({ systemctl list-units --all --plain --no-legend; systemctl list-unit-files --no-legend; } | awk '{print $1}' | grep -E '^(donkeywork-desktop|dwdesktop-|dwconsole-).*\.(service|target)$' | sort -u)
  for unit in "${units[@]}"; do
    systemctl stop "$unit" || ! systemctl is-active --quiet "$unit"
    systemctl disable "$unit" 2>/dev/null || true
  done
  # Prefixes deliberately exclude the unrelated donkeywork-device-client.
  while IFS= read -r -d '' path; do move_path "$path"; done < <(find /etc/systemd/system -maxdepth 2 \( -name 'donkeywork-desktop*' -o -name 'dwdesktop-*' -o -name 'dwconsole-*' \) -print0)
  for directory in /etc/udev/rules.d /etc/modules-load.d /etc/modprobe.d /etc/tmpfiles.d /etc/sudoers.d /etc/pam.d; do
    while IFS= read -r -d '' path; do move_path "$path"; done < <(find "$directory" -maxdepth 1 -type f \( -name '*dwdesktop*' -o -name '*dwconsole*' -o -name '*donkeywork-desktop*' \) -print0)
  done
  while IFS= read -r -d '' path; do move_path "$path"; done < <(find /home/localuser/.config/systemd/user -maxdepth 2 \( -name 'dwdesktop-*' -o -name 'dwconsole-*' -o -name 'app-dev.donkeywork.Desktop*' \) -print0)
  for path in /opt/donkeywork-desktop /opt/donkeywork-desktop-agent /opt/donkeywork-desktop-codecs /etc/donkeywork-desktop /etc/dwdesktop /var/lib/dwdesktop-agent /home/localuser/.local/lib/dwdesktop /home/localuser/.config/dwdesktop /home/localuser/.config/autostart/dev.donkeywork.Desktop.desktop; do move_path "$path"; done
  for config in /etc/gdm/custom.conf /etc/gdm3/custom.conf; do
    backup="$config.pre-dwdesktop-autologin-20260912"
    test -f "$backup" || continue
    # Only restore if all intervening changes are our two autologin lines.
    if diff -qB <(sed '/^AutomaticLoginEnable=true$/d; /^AutomaticLogin=localuser$/d' "$config") "$backup" >/dev/null; then
      move_path "$config"
      cp -a "$backup" "$config"
      move_path "$backup"
      echo "Restored display-manager configuration: $config"
    else
      echo "REVIEW: intervening changes in $config; not overwritten"
    fi
  done
  systemctl daemon-reload
  udevadm control --reload-rules
  # No module unload, udev trigger, display-manager restart or host reboot.
  ;;
verify)
  systemctl --user daemon-reload
  systemctl list-units --all --plain --no-legend | grep -E '^(donkeywork-desktop[.-]|dwdesktop-|dwconsole-)' || true
  systemctl --user list-units --all --plain --no-legend | grep -E '^(dwdesktop-|dwconsole-|app-dev\.donkeywork\.Desktop).*\.service' || true
  ;;
*) echo 'Usage: remove-lab-pilot.sh user|root|verify' >&2; exit 2;;
esac
