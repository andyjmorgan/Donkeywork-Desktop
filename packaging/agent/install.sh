#!/usr/bin/env bash
set -euo pipefail
set +x
umask 077
manager_url='' enrollment_code='' code_file='' ca_file=''
while (($#)); do
 case "$1" in
 --manager|--code|--code-file|--ca-file)
  (($#>=2)) || { echo 'Missing option value' >&2; exit 2; }
  case "$1" in
   --manager) manager_url=$2;; --code) enrollment_code=$2;; --code-file) code_file=$2;; --ca-file) ca_file=$2;;
  esac; shift 2;;
 --help) echo 'Usage: sudo ./install.sh --manager https://HOST:PORT [--code XXXX-XXXX | --code-file PATH] [--ca-file PATH]'; exit 0;;
 *) echo 'Unknown option' >&2; exit 2;;
 esac
done
[[ $EUID == 0 ]] || { echo 'Run installer with sudo.' >&2; exit 1; }
[[ -z $enrollment_code || -z $code_file ]] || { echo 'Choose code or code-file, not both.' >&2; exit 2; }
command -v systemctl >/dev/null
systemctl show-environment >/dev/null
package_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
case $(uname -m) in x86_64) arch=amd64;; aarch64|arm64) arch=arm64;; *) echo 'Unsupported architecture' >&2; exit 1;; esac
(cd "$package_dir" && sha256sum --check --status SHA256SUMS) || { echo 'Package checksum failure' >&2; exit 1; }
state_dir=/var/lib/dwdesktop-agent
[[ ! -L $state_dir ]] || { echo 'State directory must not be a symlink' >&2; exit 1; }
for state_file in device.key device.crt manager-ca.crt device.json; do
 [[ ! -L "$state_dir/$state_file" ]] || { echo 'State files must not be symlinks' >&2; exit 1; }
done
if [[ ! -f $state_dir/device.json ]]; then
 if [[ -z $manager_url ]]; then [[ -t 0 ]] || { echo '--manager required headlessly' >&2; exit 2; }; read -r -p 'Manager HTTPS URL: ' manager_url; fi
 if [[ -z $enrollment_code && -z $code_file ]]; then [[ -t 0 ]] || { echo '--code or --code-file required headlessly' >&2; exit 2; }; read -r -s -p 'Enrollment code: ' enrollment_code; echo; fi
 if [[ -z $ca_file && -t 0 ]]; then read -r -p 'Manager CA PEM path (blank for system trust): ' ca_file; fi
fi
if ! id dwdesktop-agent >/dev/null 2>&1; then useradd --system --home-dir "$state_dir" --shell /usr/sbin/nologin dwdesktop-agent; fi
install -d -m 0700 -o dwdesktop-agent -g dwdesktop-agent "$state_dir"
if [[ ! -f $state_dir/device.json ]]; then
 args=(enroll --state-dir "$state_dir" --manager "$manager_url")
 [[ -z $ca_file ]] || args+=(--ca-file "$ca_file")
 # Keep the code out of child process arguments, even when provided to this script.
 if [[ -n $code_file ]]; then
  "$package_dir/dwdesktop-agent-$arch" "${args[@]}" --code-file "$code_file"
 else
  printf '%s' "$enrollment_code" | "$package_dir/dwdesktop-agent-$arch" "${args[@]}" --code-file /dev/stdin
 fi
 unset enrollment_code
fi
chown -R dwdesktop-agent:dwdesktop-agent "$state_dir"
install -d -m 0755 /opt/donkeywork-desktop-agent/0.1.0-preview6
install -m 0755 "$package_dir/dwdesktop-agent-$arch" /opt/donkeywork-desktop-agent/0.1.0-preview6/dwdesktop-agent
ln -sfn /opt/donkeywork-desktop-agent/0.1.0-preview6 /opt/donkeywork-desktop-agent/current
install -m 0644 "$package_dir/dwdesktop-agent.service" /etc/systemd/system/dwdesktop-agent.service
systemctl daemon-reload
systemctl enable dwdesktop-agent.service
systemctl restart dwdesktop-agent.service
systemctl is-active --quiet dwdesktop-agent.service
echo 'Device service installed and started. Check the manager for Online status.'
