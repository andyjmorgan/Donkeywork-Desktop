#!/bin/bash
set -euo pipefail
set +x
umask 077

export XDG_SESSION_TYPE=x11
export XDG_CURRENT_DESKTOP=ubuntu:GNOME
export XDG_SESSION_DESKTOP=ubuntu
export DESKTOP_SESSION=ubuntu
export GNOME_SESSION_DISABLE_SYSTEMD=1
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=softpipe
export LP_NUM_THREADS=4
exec /usr/bin/gnome-session --session=ubuntu
