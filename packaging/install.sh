#!/usr/bin/env bash
# Internal pilot installer. Never source configuration supplied by the caller.
set -euo pipefail
umask 022

usage() {
  printf '%s\n' 'Usage: install.sh --profile vkms|kms|x11 --listen PRIVATE_IPV4 [options]' \
    '  --source DIR         Extracted release (default: parent of packaging/)' \
    '  --device /dev/dri/cardN  Required for kms; optional x11 validation node' \
    '  --ffmpeg ABS_PATH    External encoder (default /usr/bin/ffmpeg)' \
    '  --control auto|x11  Explicit guarded X11 control for x11/kms capture' \
    '  --dry-run            Validate and print actions; write nothing' \
    '  --root DIR           Staging filesystem prefix; activation forbidden' \
    '  --replace-config     Explicitly replace a differing console.env' \
    '  --configure-vkms     Write VKMS boot/udev configuration; never load/restart' \
    '  --activate           Enable/restart package target; refuses live pilot units'
}
die() { printf 'install: %s\n' "$*" >&2; exit 1; }
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
stage_root='' profile='' listen='' device='' dry=0 activate=0 replace=0 configure=0
ffmpeg=/usr/bin/ffmpeg
control=auto
while (($#)); do
  case "$1" in
    --source|--root|--profile|--listen|--device|--ffmpeg|--control)
      (($# >= 2)) || die "missing value for $1"
      case "$1" in --source) source_dir=$2;; --root) stage_root=$2;; --profile) profile=$2;; --listen) listen=$2;; --device) device=$2;; --ffmpeg) ffmpeg=$2;; --control) control=$2;; esac
      shift 2;;
    --dry-run) dry=1; shift;; --activate) activate=1; shift;;
    --replace-config) replace=1; shift;; --configure-vkms) configure=1; shift;;
    -h|--help) usage; exit 0;; *) die "unknown option $1";;
  esac
done
[[ $profile == vkms || $profile == kms || $profile == x11 ]] || die 'explicit supported --profile required'
[[ $control == auto || ($control == x11 && $profile != vkms) ]] || die 'guarded X11 control requires x11/kms profile'
[[ $ffmpeg =~ ^/[A-Za-z0-9_./+-]+$ && $ffmpeg != *'/../'* && $ffmpeg != */.. ]] || die 'ffmpeg must be a plain absolute executable path'
[[ $listen =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]] || die 'explicit private IPv4 --listen required'
IFS=. read -r a b c d <<< "$listen"
for octet in "$a" "$b" "$c" "$d"; do
  [[ $octet == 0 || $octet != 0* ]] || die 'ambiguous IPv4 leading zero'
  ((10#$octet <= 255)) || die 'invalid IPv4 octet'
done
[[ $a == 10 || ($a == 172 && $b -ge 16 && $b -le 31) || ($a == 192 && $b == 168) ]] || die 'listen must be RFC1918 IPv4'
if [[ $profile == vkms ]]; then
  [[ -z $device || $device == auto ]] || die 'VKMS device is discovered by driver; omit --device'
  device=auto
else
  if [[ $profile == x11 && -z $device ]]; then device=/dev/dri/card0; fi
  [[ $device =~ ^/dev/dri/card[0-9]+$ ]] || die 'kms requires explicit --device /dev/dri/cardN'
fi
(( !configure )) || [[ $profile == vkms ]] || die '--configure-vkms requires vkms profile'
[[ $source_dir == /* && -d $source_dir ]] || die '--source must be an existing absolute directory'
source_dir=$(cd -- "$source_dir" && pwd -P)
if [[ -n $stage_root ]]; then
  [[ $stage_root == /* && $stage_root != / && $stage_root != *'/../'* && $stage_root != */.. ]] || die '--root must be an absolute non-root staging path'
  (( !activate )) || die '--root never permits activation'
  stage_root=${stage_root%/}
  [[ ! -L $stage_root ]] || die 'staging root may not be a symlink'
  [[ $(realpath -m -- "$stage_root") == "$stage_root" ]] || die 'staging root must be canonical without symlink ancestors'
elif (( !dry && EUID != 0 )); then die 'live installation requires root'; fi
expected_owner=0
[[ -z $stage_root ]] || expected_owner=$EUID
check_directory_chain() {
  local candidate=$1 boundary=${stage_root:-/} owner mode
  while :; do
    [[ ! -L $candidate ]] || die "symlink ancestor in installation path: $candidate"
    if [[ -e $candidate ]]; then
      [[ -d $candidate ]] || die "non-directory in installation path: $candidate"
      owner=$(stat -c '%u' "$candidate"); mode=$(stat -c '%a' "$candidate")
      [[ $owner == "$expected_owner" ]] || die "installation directory owned by another account: $candidate"
      (( (8#$mode & 8#022) == 0 )) || die "group/world-writable installation directory: $candidate"
    fi
    [[ $candidate == "$boundary" ]] && break
    candidate=${candidate%/*}
    [[ -n $candidate ]] || candidate=/
  done
}

[[ -f $source_dir/VERSION && ! -L $source_dir/VERSION ]] || die 'release VERSION missing'
version=$(<"$source_dir/VERSION")
[[ $version =~ ^[A-Za-z0-9][A-Za-z0-9._+-]{0,79}$ && $version != . && $version != .. ]] || die 'unsafe release VERSION'
for path in bin libexec share/web share/systemd; do [[ -d $source_dir/$path && ! -L $source_dir/$path ]] || die "payload directory missing: $path"; done
for binary in dwconsole-daemon dwconsole input-daemon input-cli web-launch socket-broker console-web; do
  [[ -x $source_dir/bin/$binary ]] || die "payload executable missing: $binary"
done
[[ -f $source_dir/share/web/index.html && -f $source_dir/share/systemd/donkeywork-desktop.target ]] || die 'web assets or package target missing'
[[ -z $(find "$source_dir" -type l -print -quit) ]] || die 'release payload may not contain symlinks'
if [[ -f $source_dir/SHA256SUMS ]]; then
  while read -r hash name; do
    name=${name#\*}
    [[ $hash =~ ^[0-9a-fA-F]{64}$ && $name != /* && $name != ../* && $name != *'/../'* && $name != */.. && $name != *[[:space:]]* ]] || die 'unsafe checksum manifest'
  done < "$source_dir/SHA256SUMS"
  (cd -- "$source_dir" && sha256sum --check --status SHA256SUMS) || die 'release checksum verification failed'
else die 'release SHA256SUMS missing'; fi

destination="$stage_root/opt/donkeywork-desktop"
config_dir="$stage_root/etc/donkeywork-desktop"
unit_dir="$stage_root/etc/systemd/system"
config_text=$(printf '%s\n' "PROFILE=$profile" "LISTEN_IP=$listen" "DRM_DEVICE=$device" \
  'WIDTH=1920' 'HEIGHT=1080' 'BIN_DIR=/opt/donkeywork-desktop/current/bin' \
  'ASSETS=/opt/donkeywork-desktop/current/share/web' 'LIBEXEC_DIR=/opt/donkeywork-desktop/current/libexec' "FFMPEG=$ffmpeg" "CONTROL=$control")
if [[ -e $config_dir/console.env && $replace == 0 ]]; then
  [[ ! -L $config_dir/console.env && $(<"$config_dir/console.env") == "$config_text" ]] || die 'existing console.env differs; use --replace-config explicitly'
fi
for path in "$destination" "$destination/releases" "$config_dir" "$unit_dir"; do
  check_directory_chain "$path"
done
if [[ -e $config_dir/console.env ]]; then
  [[ ! -L $config_dir/console.env && -f $config_dir/console.env ]] || die 'existing configuration must be regular'
  owner=$(stat -c '%u' "$config_dir/console.env"); mode=$(stat -c '%a' "$config_dir/console.env")
  [[ $owner == "$expected_owner" ]] && (( (8#$mode & 8#022) == 0 )) || die 'existing configuration ownership/permissions are unsafe'
fi
[[ ! -e $destination/current || -L $destination/current ]] || die 'current path exists and is not a release symlink'
for unit in "$source_dir"/share/systemd/donkeywork-desktop*; do
  [[ -f $unit ]] || continue
  name=${unit##*/}
  [[ $name =~ ^donkeywork-desktop([a-z0-9.-]*)\.(service|target)$ ]] || die 'unexpected unit name'
  [[ ! -L $unit_dir/$name ]] || die "refusing existing unit symlink: $name"
done
if ((configure)); then
  module_dir="$stage_root/etc/modules-load.d"
  rule_dir="$stage_root/etc/udev/rules.d"
  vendor="$stage_root/usr/lib/udev/rules.d/61-mutter.rules"
  [[ ! -L $module_dir && ! -L $rule_dir ]] || die 'VKMS configuration directories may not be symlinks'
  check_directory_chain "$module_dir"
  check_directory_chain "$rule_dir"
  for managed in "$module_dir/donkeywork-desktop-vkms.conf" "$rule_dir/99-donkeywork-desktop-vkms.rules"; do
    [[ ! -e $managed && ! -L $managed ]] || die "VKMS configuration already exists; review without overwriting: $managed"
  done
  if [[ -f $vendor ]] && grep -Fq 'ENV{ID_PATH}=="platform-vkms", TAG+="mutter-device-ignore"' "$vendor"; then
    [[ ! -e $rule_dir/61-mutter.rules && ! -L $rule_dir/61-mutter.rules ]] || die 'existing local61-mutter.rules requires manual reconciliation'
  fi
fi
if ((activate)); then
  [[ $EUID == 0 ]] || die 'activation requires root'
  command -v systemctl >/dev/null || die 'systemd not available'
  pilots=$(systemctl list-units --state=active,activating,reloading --no-legend --plain 'dwconsole*' 2>/dev/null) || die 'cannot inspect live pilot units'
  [[ -z $pilots ]] || die 'active dwconsole pilot units exist; explicit migration is required before activation'
  bash "$source_dir/packaging/doctor.sh" --profile "$profile" --device "$device" --bin-dir "$source_dir/bin" --ffmpeg "$ffmpeg" --control "$control" --activation || die 'doctor preflight failed; activation refused'
fi
printf 'Release: %s\nProfile: %s\nViewer: http://%s:8090\nDestination: %s/releases/%s\n' "$version" "$profile" "$listen" "$destination" "$version"
reuse=0
if [[ -e $destination/releases/$version ]]; then
  [[ -d $destination/releases/$version && ! -L $destination/releases/$version ]] || die 'existing release is not a regular directory'
  cmp -s "$source_dir/SHA256SUMS" "$destination/releases/$version/SHA256SUMS" || die 'existing VERSION has different contents; use a new version'
  (cd -- "$destination/releases/$version" && sha256sum --check --status SHA256SUMS) || die 'existing release integrity failed; refusing overwrite'
  reuse=1
fi
if ((dry)); then
  printf 'DRY RUN: validated payload; would install release, configuration and package units.\n'
  (( !configure )) || printf 'DRY RUN: would stage explicit VKMS boot and Mutter udev configuration.\n'
  (( !activate )) || printf 'DRY RUN: would enable/restart donkeywork-desktop.target.\n'
  exit 0
fi
mkdir -p -- "$destination/releases" "$config_dir" "$unit_dir"
chmod 0755 -- "$destination" "$destination/releases" "$config_dir"
temporary='no release staging directory (reusing verified release)'
trap 'printf "install interrupted; retained staging path: %s\n" "$temporary" >&2' ERR
if ((!reuse)); then
  temporary=$(mktemp -d "$destination/releases/.install-$version.XXXXXX")
  cp -a -- "$source_dir/." "$temporary/"
  find "$temporary" -type d -exec chmod 0755 {} +
  find "$temporary" -type f -exec chmod a+r,go-w {} +
  if [[ -z $stage_root ]]; then chown -R root:root -- "$temporary"; fi
  mv -- "$temporary" "$destination/releases/$version"
fi
config_tmp=$(mktemp "$config_dir/.console.env.XXXXXX")
printf '%s\n' "$config_text" > "$config_tmp"
chmod 0644 "$config_tmp"
mv -fT -- "$config_tmp" "$config_dir/console.env"
for unit in "$source_dir"/share/systemd/donkeywork-desktop*; do
  [[ -f $unit ]] || continue
  name=${unit##*/}
  [[ $name =~ ^donkeywork-desktop([a-z0-9.-]*)\.(service|target)$ ]] || die 'unexpected unit name'
  [[ ! -L $unit_dir/$name ]] || die "refusing existing unit symlink: $name"
  install -m 0644 -- "$unit" "$unit_dir/$name"
done
if ((configure)); then
  mkdir -p -- "$module_dir" "$rule_dir"
  printf '# DonkeyWork Desktop explicit VKMS setup\nvkms\n' > "$module_dir/donkeywork-desktop-vkms.conf"
  printf '%s\n' '# DonkeyWork Desktop explicit VKMS setup' \
    'SUBSYSTEM=="drm", KERNEL=="card[0-9]*", ENV{DEVTYPE}=="drm_minor", ENV{ID_PATH}=="platform-vkms", TAG-="mutter-device-ignore"' > "$rule_dir/99-donkeywork-desktop-vkms.rules"
  if [[ -f $vendor ]] && grep -Fq 'ENV{ID_PATH}=="platform-vkms", TAG+="mutter-device-ignore"' "$vendor"; then
    { printf '# DonkeyWork Desktop VKMS override; review vendor updates before upgrade.\n';
      sed '/^ENV{ID_PATH}=="platform-vkms", TAG+="mutter-device-ignore"$/d' "$vendor"; } > "$rule_dir/61-mutter.rules"
  fi
  printf 'VKMS configuration staged. No module load, udev reload, GDM restart or reboot performed.\n'
fi
link_dir=$(mktemp -d "$destination/.switch.XXXXXX")
ln -s -- "releases/$version" "$link_dir/current"
mv -fT -- "$link_dir/current" "$destination/current"
rmdir -- "$link_dir"
trap - ERR
if [[ -z $stage_root ]] && command -v restorecon >/dev/null; then
  restorecon -R "$destination" "$config_dir"
  for unit in "$source_dir"/share/systemd/donkeywork-desktop*; do restorecon "$unit_dir/${unit##*/}"; done
  if ((configure)); then restorecon -R "$module_dir" "$rule_dir"; fi
fi
if ((activate)); then
  systemctl daemon-reload
  systemctl enable donkeywork-desktop.target
  systemctl restart donkeywork-desktop.target
else printf 'Installed without activation. Existing live pilot services were not changed.\n'; fi
