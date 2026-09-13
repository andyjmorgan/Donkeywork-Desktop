#!/bin/bash
set -euo pipefail
trap 'printf "Console startup not ready at line %s\n" "$LINENO" >&2' ERR
# Attach the active physical seat, including the GDM greeter. Never spawn a desktop.
session=$(loginctl show-seat seat0 -p ActiveSession --value)
[[ -n "$session" && $(loginctl show-session "$session" -p Type --value) == x11 ]]
[[ $(loginctl show-session "$session" -p Active --value) == yes ]]
seat_uid=$(loginctl show-session "$session" -p User --value)
[[ "$seat_uid" =~ ^[0-9]+$ && "$seat_uid" != 0 ]]
seat_user=$(getent passwd "$seat_uid" | cut -d: -f1)
seat_gid=$(id -g "$seat_user")
display=''
authority=''
for pid in $(pgrep -u "$seat_uid" -x gnome-shell); do
    while IFS= read -r -d '' entry; do
        case "$entry" in
            DISPLAY=*) display=${entry#DISPLAY=} ;;
            XAUTHORITY=*) authority=${entry#XAUTHORITY=} ;;
        esac
    done < "/proc/$pid/environ"
    [[ -n "$display" && -n "$authority" ]] && break
done
[[ "$display" =~ ^:[0-9]+(\.[0-9]+)?$ ]]
[[ "$authority" == "/run/user/$seat_uid/gdm/Xauthority" && -f "$authority" ]]
geometry=$(runuser -u "$seat_user" -- env DISPLAY="$display" XAUTHORITY="$authority" xrandr --query | awk '/^Screen 0:/ {gsub(",", "", $10); print $8, $10; exit}')
read -r width height <<< "$geometry"
[[ "$width" =~ ^[0-9]+$ && "$height" =~ ^[0-9]+$ ]]
/opt/donkeywork-desktop/bin/dwconsole-daemon \
    --socket /run/dwconsole/stream.sock --device /dev/dri/card0 \
    --fps 30 --encoder libx264 --x11-display "$display" \
    --xauthority "$authority" --x11-uid "$seat_uid" --x11-gid "$seat_gid" \
    --width "$width" --height "$height" &
capture_pid=$!
trap 'kill -TERM "$capture_pid" 2>/dev/null || true; wait "$capture_pid" 2>/dev/null || true' EXIT
# On a seat handoff, exit and let systemd clean the entire capture cgroup/socket.
while kill -0 "$capture_pid" 2>/dev/null; do
    [[ $(loginctl show-seat seat0 -p ActiveSession --value) == "$session" ]] || exit 0
    sleep 1
done
wait "$capture_pid"
