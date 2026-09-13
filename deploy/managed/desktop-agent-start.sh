#!/usr/bin/env bash
set -euo pipefail
# Managed desktops have their own runtime directory; never attach their portal
# or replace the physical console user's activation environment.
[[ ${XDG_RUNTIME_DIR:-} == /run/user/$(id -u) ]] || exit 0
systemctl --user import-environment DISPLAY WAYLAND_DISPLAY XAUTHORITY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE
systemctl --user start app-dev.donkeywork.Desktop.service
