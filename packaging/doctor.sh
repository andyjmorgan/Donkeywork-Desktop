#!/usr/bin/env bash
# Read-only prerequisite report. Does not source /etc configuration or load drivers.
set -uo pipefail
profile='' device='' bin_dir=/opt/donkeywork-desktop/current/bin activation=0 encoder=/usr/bin/ffmpeg control=auto
usage(){ printf '%s\n' 'Usage: doctor.sh --profile vkms|kms|x11 [--device /dev/dri/cardN] [--bin-dir DIR] [--ffmpeg ABS_PATH] [--control auto|x11] [--activation]'; }
while (($#)); do
  case "$1" in
    --profile|--device|--bin-dir|--ffmpeg|--control)
      (($# >= 2)) || { usage; exit 2; }
      case "$1" in --profile) profile=$2;; --device) device=$2;; --bin-dir) bin_dir=$2;; --ffmpeg) encoder=$2;; --control) control=$2;; esac; shift 2;;
    --activation) activation=1; shift;;
    -h|--help) usage; exit 0;; *) usage; exit 2;;
  esac
done
[[ $profile == vkms || $profile == kms || $profile == x11 ]] || { usage; exit 2; }
[[ $control == auto || ($control == x11 && $profile != vkms) ]] || { usage; exit 2; }
[[ $bin_dir == /* ]] || { printf 'bin-dir must be absolute\n' >&2; exit 2; }
[[ $encoder == /* ]] || { printf 'ffmpeg must be absolute\n' >&2; exit 2; }
failures=0
ok(){ printf 'OK   %s\n' "$*"; }
warn(){ printf 'WARN %s\n' "$*"; }
fail(){ printf 'FAIL %s\n' "$*"; failures=$((failures+1)); }
printf 'Read-only DonkeyWork Desktop prerequisite report (%s)\n' "$profile"
uname -smr
if [[ -r /etc/os-release ]]; then grep '^PRETTY_NAME=' /etc/os-release; fi
case $(uname -m) in x86_64|aarch64) ok 'supported package CPU architecture';; *) fail 'architecture has no declared pilot build';; esac
command -v systemctl >/dev/null && ok 'systemd command available' || fail 'systemd required'
if command -v getenforce >/dev/null; then printf 'SELinux: '; getenforce; fi
if command -v ldconfig >/dev/null && ldconfig -p 2>/dev/null | grep 'libdrm.so.2' >/dev/null; then ok 'libdrm runtime available'; else fail 'libdrm runtime unavailable'; fi
architecture=$(uname -m)
if [[ -r $bin_dir/../ARCH ]]; then
  artifact_arch=$(<"$bin_dir/../ARCH")
  if [[ ($architecture == x86_64 && $artifact_arch == amd64) || ($architecture == aarch64 && $artifact_arch == arm64) ]]; then ok 'artifact architecture matches host'; else fail 'artifact ARCH does not match this host'; fi
fi
for name in dwconsole-daemon dwconsole input-daemon input-cli web-launch socket-broker console-web; do
  executable="$bin_dir/$name"
  if [[ ! -x $executable ]]; then fail "package executable missing: $name"; continue; fi
  if command -v readelf >/dev/null; then
    header=$(readelf -h "$executable" 2>/dev/null) || { fail "invalid ELF executable: $name"; continue; }
    if [[ ($architecture == x86_64 && $header == *'Advanced Micro Devices X86-64'*) || ($architecture == aarch64 && $header == *'AArch64'*) ]]; then :; else fail "ELF architecture mismatch: $name"; fi
  else warn 'readelf absent; ELF architecture check unavailable'; fi
  if command -v ldd >/dev/null; then
    dependencies=$(ldd "$executable" 2>&1) || true
    if [[ $dependencies == *'not found'* || $dependencies == *'version '*' not found'* ]]; then fail "missing runtime libraries/ABI: $name"; fi
  else warn 'ldd absent; runtime dependency check unavailable'; fi
done
if command -v python3 >/dev/null; then
  if PYTHONDONTWRITEBYTECODE=1 python3 -c 'import dbus; from gi.repository import GLib' >/dev/null 2>&1; then ok 'Python D-Bus and GLib available'; else fail 'Python dbus/gi bindings missing'; fi
else fail 'Python3 missing'; fi
if [[ -n $encoder && -x $encoder ]]; then
  ownership=$(stat -c '%u' "$encoder")
  permission=$(stat -c '%a' "$encoder")
  if [[ -L $encoder || ! -f $encoder || $ownership != 0 ]] || (( (8#$permission & 8#022) != 0 )); then
    fail 'FFmpeg must be a root-owned regular executable without group/world write permission'
    printf 'Prerequisite failures: %s\n' "$failures"
    exit 1
  fi
  if "$encoder" -hide_banner -encoders 2>/dev/null | grep 'libx264' >/dev/null; then ok "H.264 libx264 encoder: $encoder"; else fail 'selected FFmpeg lacks libx264'; fi
  if [[ $profile == x11 ]] && ! "$encoder" -hide_banner -devices 2>/dev/null | grep 'x11grab' >/dev/null; then fail 'X11 profile requires FFmpeg x11grab (minimal rawvideo encoder is insufficient)'; fi
else fail 'FFmpeg executable missing; no package/repository change attempted'; fi
if [[ $profile == vkms ]]; then
  if command -v modinfo >/dev/null && modinfo -n vkms >/dev/null 2>&1; then ok 'VKMS module available for running kernel'; else fail 'VKMS module unavailable for running kernel'; fi
  if [[ -d /sys/module/vkms ]]; then ok 'VKMS is loaded'; elif ((activation)); then fail 'VKMS is not loaded'; else warn 'VKMS not loaded; doctor will not load it'; fi
  if [[ -c /dev/uinput ]]; then ok 'uinput device exists (root helper permission still required)'; elif ((activation)); then fail 'uinput missing'; else warn 'uinput not present; control cannot start until provided'; fi
  if ((activation)); then
    device=''
    for card in /sys/class/drm/card[0-9]*; do
      [[ ${card##*/} =~ ^card[0-9]+$ ]] || continue
      if [[ $(basename "$(readlink -f "$card/device/driver")") == vkms ]] ||
         { [[ $(basename "$(readlink -f "$card/device/subsystem")") == platform ]] && grep -Fx 'MODALIAS=platform:vkms' "$card/device/uevent" >/dev/null 2>&1; }; then
        [[ -z $device ]] || { fail 'multiple VKMS cards require explicit review'; break; }
        device="/dev/dri/${card##*/}"
      fi
    done
    [[ -n $device ]] || fail 'no VKMS DRM card discovered'
  fi
  rule=/usr/lib/udev/rules.d/61-mutter.rules
  [[ ! -f /etc/udev/rules.d/61-mutter.rules ]] || rule=/etc/udev/rules.d/61-mutter.rules
  if [[ -f $rule ]] && grep -Fq 'TAG+="mutter-device-ignore"' "$rule" && grep -Fq 'platform-vkms' "$rule"; then warn 'Mutter rule may ignore VKMS; explicitly review installer --configure-vkms and existing overrides'; fi
  warn 'GNOME adoption,1080p output, greeter/login and reboot recovery require real validation'
elif [[ $profile == kms ]]; then
  [[ $device =~ ^/dev/dri/card[0-9]+$ && -c $device ]] && ok "selected DRM node exists: $device" || fail 'explicit existing DRM --device required'
  warn 'KMS output and input mapping require live validation; control is disabled unless explicitly selected'
else
  if command -v Xorg >/dev/null || [[ -x /usr/lib/xorg/Xorg || -x /usr/libexec/Xorg ]]; then ok 'Xorg server installed'; else fail 'Xorg server unavailable; Xwayland is not a substitute'; fi
  warn 'X11 capture and optional guarded control require existing-seat verification'
fi
if [[ $control == x11 ]]; then
  command -v xrandr >/dev/null && ok 'XRandR query tool available' || fail 'guarded X11 control requires xrandr'
  [[ -c /dev/uinput ]] && ok 'uinput available for X11 control' || fail 'guarded X11 control requires uinput'
  seat_session=$(loginctl show-seat seat0 -p ActiveSession --value 2>/dev/null || true)
  if [[ $seat_session =~ ^[A-Za-z0-9]+$ ]] && [[ $(loginctl show-session "$seat_session" -p Type --value 2>/dev/null) == x11 ]]; then
    ok 'active seat is X11; runtime guard still verifies capture identity and mapping'
  else fail 'guarded X11 control needs a current X11 seat'; fi
fi
if ((activation)) && [[ $profile == vkms || $profile == kms ]]; then
  minor_hex=$(stat -c '%T' "$device" 2>/dev/null || true)
  if [[ $minor_hex =~ ^[0-9a-fA-F]+$ ]]; then node=$((16#$minor_hex)); else node=unavailable; fi
  state="/sys/kernel/debug/dri/$node/state"
  if [[ ! -r $state ]]; then fail 'selected DRM debug state is not readable';
  elif ! python3 -c 'import re,sys
with open(sys.argv[1]) as source: text=source.read(65537)
blocks=re.findall(r"^crtc\[\d+\]:.*?(?=^\w|\Z)",text,re.M|re.S)
active=[b for b in blocks if re.search(r"^\s+active=1$",b,re.M)]
valid=len(text)<=65536 and len(active)==1 and re.search(r"mode:\s+\"[^\"]*\":\s+\d+\s+\d+\s+1920\s+\d+\s+\d+\s+\d+\s+1080(?:\s|$)",active[0])
sys.exit(0 if valid else 1)' "$state"; then fail 'selected output is not active at1080p';
  else ok 'selected DRM output reports active1080p (pixels still require verification)'; fi
fi
if command -v loginctl >/dev/null; then loginctl list-sessions --no-legend 2>/dev/null || warn 'logind session inventory unavailable'; fi
printf 'Prerequisite failures: %s. This report is not captured-frame or fleet acceptance.\n' "$failures"
(( failures == 0 ))
