#!/usr/bin/env bash
# Scoped one-host migration helper. Default is a plan; no service action occurs.
set -euo pipefail
umask 077
source_dir=/tmp/dwdesktop-package-review.zo1dsW/donkeywork-desktop-0.1.0-lab.20260906.5-linux-amd64
encoder=/opt/donkeywork-desktop/bin/ffmpeg
encoder_sha=850bec40438959b185c880b4c7096bf49fe9cb587ed816f52f2e3b5a9b80ead3
pilots=(dwconsole-rocky-output dwconsole-rocky-capture dwconsole-rocky-web)
die(){ printf 'office1 migration: %s\n' "$*" >&2; exit 1; }
usage(){ printf '%s\n' 'office1-migrate.sh [--plan | --execute | --rollback BACKUP_DIR]' \
 'Only office1 package/pilot services. Never GDM, physical GPU, k3s or reboot.'; }
action=${1:---plan}
if [[ $action == --help ]]; then usage; exit 0; fi
[[ $action == --plan || $action == --execute || $action == --rollback ]] || { usage; exit 2; }
if [[ $action == --plan ]]; then
  printf '%s\n' 'Pending explicit migration authorization and successful minigpu reboot validation:' \
    '1. Verify .5 checksum/doctor, active GDM greeter and the three exact old pilot units.' \
    '2. Back up their exact transient unit files and active seat identity root-private.' \
    '3. Stop old web, capture, output; preserve all old binaries and the private encoder.' \
    '4. Install/activate .5 VKMS package using the existing private FFmpeg.' \
    '5. Validate browser image, fresh guard and input at the confirmed greeter.' \
    'Failure: stop/disable package target and restore only the three backed-up units.' \
    'No module-load/udev changes or GDM restart in this migration. Boot persistence remains separate.'
  exit 0
fi
[[ $EUID == 0 && $(hostname -s) == office1 ]] || die 'root on office1 is required'

restore_pilots(){
  local backup=$1 unit
  [[ $backup =~ ^/var/lib/donkeywork-desktop-migration/office1\.[A-Za-z0-9]{6}$ ]] || die 'unexpected backup path'
  [[ -d $backup && ! -L $backup && $(stat -c '%u' "$backup") == 0 ]] || die 'backup must be root-owned regular directory'
  [[ $(loginctl show-seat seat0 -p ActiveSession --value) == "$(<"$backup/active-session")" ]] || die 'active seat changed; review rollback instead of reviving a stale output helper'
  for unit in "${pilots[@]}"; do [[ -f $backup/$unit.service && ! -L $backup/$unit.service ]] || die 'backup incomplete'; done
  systemctl stop donkeywork-desktop-session-supervisor.service donkeywork-desktop-web.service \
    donkeywork-desktop-input.service donkeywork-desktop-broker.service donkeywork-desktop-capture.service || true
  systemctl stop donkeywork-desktop-input-guard.service donkeywork-desktop-vkms-output.service || true
  systemctl stop donkeywork-desktop.target || true
  systemctl disable donkeywork-desktop.target || true
  for unit in "${pilots[@]}"; do
    [[ ! -e /run/systemd/system/$unit.service && ! -L /run/systemd/system/$unit.service ]] || die 'rollback runtime unit path already exists; review it'
    install -m 0644 "$backup/$unit.service" "/run/systemd/system/$unit.service"
    command -v restorecon >/dev/null && restorecon "/run/systemd/system/$unit.service"
  done
  systemctl daemon-reload
  systemctl start dwconsole-rocky-output.service
  systemctl start dwconsole-rocky-capture.service
  systemctl start dwconsole-rocky-web.service
  printf 'Restored three pilot services. New release/config retained inert for inspection; package target disabled.\n'
}

if [[ $action == --rollback ]]; then
  (($# == 2)) || die '--rollback requires exact backup directory'
  restore_pilots "$2"
  exit 0
fi
(($# == 1)) || die 'unexpected execution arguments'
[[ -d $source_dir && $(<"$source_dir/VERSION") == 0.1.0-lab.20260906.5 ]] || die 'staged .5 release missing'
(cd "$source_dir" && sha256sum --check --status SHA256SUMS) || die 'payload integrity failure'
[[ $(sha256sum "$encoder" | cut -d ' ' -f 1) == "$encoder_sha" ]] || die 'private encoder differs from validated build'
[[ ! -e /etc/donkeywork-desktop/console.env && ! -e /opt/donkeywork-desktop/current && ! -L /opt/donkeywork-desktop/current ]] || die 'package state already exists; not a first migration'
active=$(loginctl show-seat seat0 -p ActiveSession --value)
[[ $(loginctl show-session "$active" -p Class --value) == greeter && $(loginctl show-session "$active" -p Type --value) == wayland ]] || die 'expected current Wayland greeter; no logged-in user migration'
for unit in "${pilots[@]}"; do
  systemctl is-active --quiet "$unit.service" || die "pilot not active: $unit"
  [[ $(systemctl show "$unit.service" -p FragmentPath --value) == /run/systemd/transient/$unit.service ]] || die "unexpected pilot unit source: $unit"
done
bash "$source_dir/packaging/doctor.sh" --profile vkms --device auto \
  --bin-dir "$source_dir/bin" --ffmpeg "$encoder" --activation
install -d -m 0700 /var/lib/donkeywork-desktop-migration
backup=$(mktemp -d /var/lib/donkeywork-desktop-migration/office1.XXXXXX)
for unit in "${pilots[@]}"; do
  install -m 0600 "/run/systemd/transient/$unit.service" "$backup/$unit.service"
done
printf '%s\n' "$active" > "$backup/active-session"
systemctl show k3s.service gdm.service -p MainPID -p ActiveEnterTimestamp > "$backup/unrelated-service-baseline.txt"
printf '%s\n' "$encoder_sha  $encoder" > "$backup/encoder.sha256"
printf 'Rollback backup: %s\n' "$backup"
trap 'status=$?; trap - ERR; printf "Migration failed; attempting scoped rollback from %s\n" "$backup" >&2; restore_pilots "$backup"; exit "$status"' ERR
systemctl stop dwconsole-rocky-web.service
systemctl stop dwconsole-rocky-capture.service
systemctl stop dwconsole-rocky-output.service
bash "$source_dir/packaging/install.sh" --source "$source_dir" --profile vkms \
  --listen 192.168.69.27 --ffmpeg "$encoder" --activate
trap - ERR
printf 'Package activated. Pending actual browser/guard/input verification; rollback directory: %s\n' "$backup"
