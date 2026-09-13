# Persistent VKMS viewer — 2026-09-06

Deployed on minigpu192.168.69.21:8090 at19:37 UTC. One deployment restart
was necessary to replace the old bridge/UI. Refresh existing browser tabs once
to load the updated client; later capture/session transitions do not require it.

## Change

Supervisor no longer stops/restarts web or input for session/guard changes.
It owns only per-session output and guard lifetimes. Persistent root-private
socket-broker provides replacement capture FDs to unprivileged Go through
SCM_RIGHTS on an inherited channel. No public/root web service introduced.

Go retains listener/PeerConnection/track across EOF or capture read timeout.
Old access-unit queues and parser state are discarded; replacement source
identity/configuration checked; only a new keyframe resumes video/input.
Browser retains the last image and displays waiting status instead of closing
a healthy peer after five seconds without frames. Actual ICE/transport failures
still use automatic reconnect.

Fixed1080p pilot: transient1024x768 compositor startup feed is not forwarded;
wait for the supervisor's validated1080p target. General live resolution
negotiation remains separate. Input releases/revalidates independently; no
stale input replay or automatic takeover.

## Live acceptance

`node tests/integration/live-capture-recovery.mjs` executed twice. It opens the
actual viewer and counts SDP offers/PeerConnection instances, then stops ONLY
our capture unit for seven seconds and recreates it with the same configuration.
No GDM restart, logout, keyboard/mouse event or user-app change.

Final run:

- Same web bridge PID throughout.
- Exactly one SDP offer and one PeerConnection; remained connected.
- Recovering status and retained last video raster during7-second gap.
- Decoded frames63→124 after recovery on the same peer.
- Then stopped only guard: supervisor produced fresh valid guard; web AND
  input PIDs remained unchanged, same viewer/offer count.

This fault test covers the former disconnect triggers without fabricating an
actual login/logout. The next real user transition is still a distinct check.

Rust24 tests passed, including4 descriptor-transfer/broker tests. Go race tests
and vet passed (descriptor validation, header/source mismatch, parser reset,
queued-old-frame discard, keyframe resume, input pause/reacquire). Browser73
tests and production build passed. No claim of full fleet/reboot acceptance.
An additional ownership probe after deployment hit HTTP409 (two-viewer limit),
so it was not counted as a live pass; existing viewers were not disconnected.

## Deployment/rollback

New root-owned `/opt/donkeywork-desktop/bin/socket-broker` and transient
`dwconsole-capture-broker.service`; runtime socket
`/run/dwconsole-broker/broker.sock`0700 parent/0600 socket. Updated root launcher
maps broker fd4, input fd3 and initial capture fd0 before privilege drop.
Existing capture/input/guard binaries otherwise unchanged by this deployment.

Stopping the NEW supervisor only stops output/guard, leaving web/input/capture/
broker alive; guard invalidity revokes control. To shut down the entire pilot,
explicitly stop those task units as well. Services are still transient; no host
boot, display-manager or cluster changes. Live Go/React source remains in the
`desktop-console-video` worktree; Rust/supervisor source in the main worktree.
