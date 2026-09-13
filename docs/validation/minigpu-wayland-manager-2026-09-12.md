# Existing Wayland console through the manager

## Accepted scope

Minigpu's existing logged-in Wayland desktop is accessible from the internal
manager alongside its managed GNOME desktop. No new desktop is created by
opening the console. This remains a single-monitor, single-viewer GNOME46 pilot.

Stable navigation:
https://192.168.10.11:30443/?device=fd6b6a59-40fc-41ac-84ef-b498ca8d6880&managed

Select **Existing Wayland console → Open console**. The console ID changes when
the portal agent restarts; use the device page rather than bookmarking that ID.

## Implemented path

Browser WebRTC ↔ attic manager ↔ existing authenticated outbound device WSS ↔
managed-broker Unix adapter ↔ in-session portal service. Video is encoded once
on Minigpu using GStreamer/libx264; the gateway relays H.264 RTP unchanged.
Keyboard, absolute pointer, buttons and wheel use public RemoteDesktop portal
methods. Browser source geometry and encoder output use the same logical size.
There is no browser-to-device media route or new enrollment mechanism.

Private socket mode0600 and an owned0700 directory limit access to the desktop
user. The broker validates socket ownership/mode and SO_PEERCRED. Requests only
target its configured socket. Console end/resize return409; the UI hides those
actions instead of treating the console as a disposable managed desktop.

One renewable one-second input lease, ordered generation/sequence, bounded
coordinates/events and held-input reset on release/disconnect/expiry. Failure
to reset closes the portal session. A new viewer cannot acquire until the old
viewer has released held input. This is not multi-controller arbitration.

## Actual browser validation

`node tests/integration/manager-wayland-console.mjs` passed against the deployed
manager image `dwdesktop-manager:preview-20260912.2`.

- Navigated from Minigpu's console card and decoded1920×1080 H.264 video.
- Browser reported30FPS. This is decoded frame rate, not30 unique scene changes
  per second; the capture pipeline repeats idle frames.
- Acquired input and clicked Activities, typed Calculator, launched it, and
  entered `123+456` using browser mouse/physical keyboard events.
- Visually inspected `calculator.png`: the real desktop Calculator shows579.
  No CLI/SSH application launch was used for this interaction.
- Reloaded, decoded again and reacquired control without a consent prompt.
- Selected ICE remote was192.168.10.11:30445: the attic gateway, not Minigpu.
- Explicit release followed by unrenewed lease expired; out-of-order sequence2
  immediately after acquisition was rejected. These were separate transport
  negative tests, not evidence of GUI application interaction.
- No browser JS errors. Evidence/report: `artifacts/manager-wayland-console/`.

The first browser trial exposed an idle capture stall: damage-driven PipeWire
was not feeding videorate after its first frame. A100ms source keepalive plus
copying the retained buffer resolved it. The next input trial clicked before
the UI had transitioned to live; the test now waits for both decoded frames and
the live UI, rather than counting early frames alone. These failures are not
silently counted as successful acceptance.

Go race tests for the manager and portal packages passed (database integration
requires its separate environment and was not newly exercised). Focused go vet
passed. Frontend73 tests and TypeScript/build passed. Added portal validation
tests cover key mapping, coordinate/button/wheel bounds, malformed fields, stale
desktop paths, origin rejection and prohibited console destruction/resize.

## Deployment and preservation

Minigpu changes only:

- Installed GStreamer ugly plugin package plus liba52, libdvdread, libmpeg2 and
  libsidplay dependencies; no upgrades/removals/system-service restarts.
- Installed `~/.local/lib/dwdesktop/session-agent` and user unit
  `app-dev.donkeywork.Desktop.service`. Unit is started, **not enabled at login**.
- Existing private `dwdesktop-portal-test/restore3` holds the current permission;
  `console.sock` is in that same0700 directory. Never publish the token.
- Added managed-root `console-socket` config and updated broker.py. Original is
  backed up as `broker.pre-portal-20260912.py` in that root.
- Restarted only our portal adapter and our managed web broker during deployment.
  Existing managed desktop `9d790bb758a04280a0932a0b4a81d183` remains ready;
  physical Wayland session214 remains active and dwdesktop-agent remains active.

Attic: deployed manager UI image .2, leaving DB/PVC/configuration intact. All
three attic nodes Ready. No host, GDM or cluster restart. Existing managed media
uses the unchanged gateway/backend route.

## Limits and rollback

Not claimed: autostart after fresh login/reboot, lock/unlock/logout continuity,
clipboard, portal display resize, multi-monitor selection, multiple viewers,
non-GNOME unattended permission provisioning or long-run fleet reliability.
Portal Session.Closed or session-bus loss stops the adapter and removes its
inventory entry. Opening a console never bootstraps permissions automatically.
If the operator stops sharing, do not auto-create a replacement desktop.

Rollback: stop our portal user unit, move the managed-root `console-socket` file
aside, and optionally restore the broker backup and restart only our broker.
This does not require terminating managed desktops. Manager .10 remains the
previous UI image if rollback is needed. Permission revocation is a separate
explicit operation; stopping the viewer alone is not permission revocation.
