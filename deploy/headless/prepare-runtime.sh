#!/bin/bash
set -euo pipefail
set +x
umask 077

[[ $(id -un) == dwdesktop ]] || { echo 'Expected dedicated dwdesktop account.' >&2; exit 1; }
[[ -d /run/dwdesktop && -O /run/dwdesktop ]] || exit 1
[[ -d /var/lib/dwdesktop && -O /var/lib/dwdesktop ]] || exit 1

authority_tmp=$(mktemp /run/dwdesktop/Xauthority.XXXXXX)
trap 'rm -f -- "$authority_tmp"' EXIT

# The cookie is delivered on stdin, never in xauth argv or diagnostic output.
cookie=$(openssl rand -hex 16)
printf 'add :99 MIT-MAGIC-COOKIE-1 %s\n' "$cookie" |
    xauth -f "$authority_tmp" source -
unset cookie
chmod 0600 "$authority_tmp"
mv -f -- "$authority_tmp" /run/dwdesktop/Xauthority

# This does not remove old X sockets/locks or daemon sockets: unexpected existing
# resources must be inspected rather than silently replaced.
