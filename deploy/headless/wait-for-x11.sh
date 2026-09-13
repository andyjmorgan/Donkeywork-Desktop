#!/bin/bash
set -euo pipefail
for ((attempt = 0; attempt < 100; attempt++)); do
    if timeout 1s xdpyinfo -display :99 >/dev/null 2>&1; then
        exit 0
    fi
    sleep 0.1
done
echo 'Dedicated X11 server did not become ready.' >&2
exit 1
