# Package minigpu acceptance commands

These scripts target **only** `localuser@192.168.69.21` and
`http://192.168.69.21:8090`, after root has migrated minigpu to the packaged
VKMS profile. They never recreate the old `dwconsole-*` transient services.
Run serially: the bridge supports only two simultaneous viewers. Close other
test browsers first; do not steal control from an existing operator.

Root repository Node dependencies and a compatible Chromium must be installed.
Optionally set `DESKTOP_TEST_CHROMIUM` to its absolute path. SSH uses existing
host-key verification, batch authentication and bounded connection/command
timeouts. Passwordless `sudo -n` is required for the package checks/actions.
No agent should execute these commands merely because this file exists.

## 1. Read-only video and screenshot

Reuse the existing display-only smoke; it has no pilot service/socket paths:

```sh
node tests/integration/live-console-smoke.mjs http://192.168.69.21:8090
```

Inspect `artifacts/console-web/live-view.png` before any input. This reused
smoke overwrites that file. Increasing decoded frames do not prove the image
is nonblank or that its greeter/desktop contents are fresh.

## 2. Browser/CLI ownership without key or pointer actions

```sh
node tests/integration/package-live-control-ownership.mjs --allow-control-acquisition
```

Uses `/opt/donkeywork-desktop/current/bin/input-cli` and
`/run/donkeywork-desktop-input/input.sock`. Checks CLI acquire/release initially,
CLI rejection while browser owns, CLI success while browser idle, then browser
reacquisition. The CLI `reset` operation acquires and releases ownership without
issuing key/button/move events. Existing held-state cleanup is still a mutation;
run only when no human is controlling the console. This does not verify actual
kernel key-up delivery.

## 3. Capture-only fault and same-peer recovery

```sh
node tests/integration/package-live-capture-recovery.mjs --allow-capture-interruption
```

Stops then starts only `donkeywork-desktop-capture.service`. The deliberate gap
exceeds seven seconds and waits for explicit `recovering`. The test checks one
offer, one connected peer, unchanged web/input PIDs, retained video without a
reconnection overlay, then advancing decoded frames after capture returns.
It attempts to restore capture in `finally`, including an ambiguous SSH stop
failure. No GDM, host, k3s, input-service or web-service restart is requested.

Optional additional guard recovery, only when separately wanted:

```sh
node tests/integration/package-live-capture-recovery.mjs --allow-capture-interruption --also-test-guard-recovery
```

This additionally stops `donkeywork-desktop-input-guard.service`; the running
package session supervisor must recreate/revalidate it without restarting
web/input or replacing the peer. A guard failure is reported, not hidden behind
an automatic supervisor/GDM restart. Root must inspect/repair a failed guard.

## 4. Visually reviewed greeter click, Escape and focus release

Only after the screenshot confirms the expected 1920×1080 GDM greeter and the
user-selection target at source pixel **(960, 490)**:

```sh
node tests/integration/package-live-console-input-smoke.mjs http://192.168.69.21:8090 --allow-reviewed-greeter-input
```

The script checks seat0 is a greeter before starting and before its actual
remote click. The first click acquires; the second selects the user. Escape
returns to the greeter. It tests release/reacquire, rejects a second viewer,
then holds Shift and moves browser focus off the video to trigger release.
No credentials, login, app launch or arbitrary session switching. A changing
session is ultimately protected by the daemon guard, not a perfect atomic
check in this test. Browser status alone is not proof of physical key-up;
the existing `observe-injected-release.py` can independently observe the
explicitly identified DonkeyWork virtual keyboard when root arranges it.

New tests write private per-run screenshot directories beneath
`artifacts/package-console-tests/` and print their paths. Inspect before/click/
Escape images; transmitted events alone are not visual success. No images are
uploaded. These scripts have been syntax-checked locally; live execution and
acceptance belong to the migration integrator.
