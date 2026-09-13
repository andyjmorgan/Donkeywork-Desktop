# Manager → device → managed desktop: first integrated pilot

## Deployed result

Manager: https://192.168.10.11:30443, attic namespace donkeywork-desktop,
image `dwdesktop-manager:preview-20260910.7`.
Device package: `dwdesktop-agent-0.1.0-preview6.tar.gz`, served by that manager.
Pilot: Easternkingdoms, device `16df48d0-3827-4dc0-963e-04a8f65335e5`.

Open its desktop chooser:
https://192.168.10.11:30443/?device=16df48d0-3827-4dc0-963e-04a8f65335e5&managed

This is the explicitly unauthenticated internal pilot. Anyone with manager
network access can use the configured pilot account. Do not publish it.

## Actual path

Browser HTTPS goes to the manager on TCP30443. Session commands are correlated
over the enrolled device's outbound mTLS WebSocket on TCP30444. The agent talks
HTTP over `/run/dwdesktop-session/broker.sock` to the existing Python broker.
The broker still owns the configured localuser managed-Xorg session lifecycle.

The browser's WebRTC peer is the manager on UDP30445. The agent receives the
existing local H.264 stream and forwards compressed RTP over the same outbound
device connection. Manager forwards RTP into its browser WebRTC track without
decoding or encoding. Keyboard/mouse messages take the reverse path to the
existing input data channel. No separate media-only device connection, no
direct browser-to-device offer, and no new device-facing public port.

The browser selected candidate was explicitly checked as
`192.168.10.11:30445/udp`. This verifies the actual relay path, not a claim about
Internet reachability or arbitrary firewall traversal. A firewall-denied direct
path test and WAN congestion/soak tests have not been performed.

## Browser evidence

`node tests/integration/manager-session-bridge.mjs` passed against the deployed
package after downloading it over verified HTTPS and reinstalling it on EK.

- GNOME environment discovery and explicit creation via the web chooser.
- A distinct session becomes ready and opens through Reconnect.
- More than 30 decoded frames before proceeding; screenshot shows 30 FPS.
- Mouse opens Text Editor and focuses the text area; keyboard renders exactly
  `manager bridge keyboard and mouse through the gateway`. Screenshot inspected.
- Reload reconnects to the same desktop, not a newly created one.
- Actual display resize 1080p → 4K → 1080p, decoded video resumes.
- All desktops returns to the same device chooser.
- End desktop removes only the newly created desktop from the live inventory.
- The pre-existing GNOME desktop remains active, same MainPID 2326633.
- No browser JavaScript errors.

Final test desktop: `915892f8c2b14513808d09944753565b` (ended).
Evidence: `artifacts/manager-session-proof/report.json`, `gnome-input.png`,
`resized-1080p.png`. Screenshots are viewport-sized, not raw 4K images.
Test session runtime/history directories are retained; no user files were deleted.

## Tests and diagnostics

- Go race tests with a real disposable PostgreSQL database: passed, including
  real mTLS enrollment/heartbeat/RPC, a browser SDP offer, and revocation closing
  its bound media viewer. Revoked devices cannot issue further RPC or connect.
- Allowlist and local Unix HTTP adapter positive/negative tests: passed.
- Read-only local media test against existing EK desktop: received RTP.
- Frontend regression suite: 73 tests passed.
- Existing contract suite: 106 tests passed (not proof of the new wire protocol).
- Managed Python broker/runtime unit suite: 9 tests passed.
- Frontend TypeScript/Vite and both agent architectures build in the image.
- `systemd-analyze verify` and `git diff --check`: passed.
- Package reinstall preserves the existing certificate hash and enrollment.
- Dedicated disposable PostgreSQL test container stopped afterward.

The first media attempt failed under the systemd sandbox: WebRTC needs netlink
to enumerate local interfaces. Reproduced under a transient test unit with
the same address-family restriction (`netlinkrib: address family not supported`).
Added AF_NETLINK; kept no capabilities, no root, read-only filesystem, protected
home and private devices. The real browser then decoded video successfully.

Initial UI test response-wait harness stalled after creating desktops; those
test desktops were explicitly ended, and the harness now checks actual inventory
transitions. Initial typing started before the editor had focus and used
Playwright's synthetic shifted-character events. The final test explicitly
focuses the editor and verifies lowercase physical-key input visually.

The manager's first process on rollout failed database migration/startup and
Kubernetes restarted it once. The subsequent process is healthy. Startup DB
retry/readiness handling remains a follow-up; this was observed before the
session bridge as well and is not claimed fixed here.

## Host and deployment footprint

EK: upgraded only dwdesktop-agent and restarted the managed broker/viewer service
to add its Unix listener. Created `/etc/tmpfiles.d/dwdesktop-session.conf` and
the restricted runtime directory. Agent runs as dwdesktop-agent; socket owner is
localuser:dwdesktop-agent, 0660, in a 2710 setgid directory. Existing independent
desktop units stayed running. No host, physical display-manager, or cluster reboot.

Attic: only own manager Deployment/Service/NetworkPolicy changed, adding the
LAN-only UDP30445 media port. PostgreSQL storage and unrelated workloads untouched.
The downloadable installer retains `--code` and `--code-file` headless options.

## Explicit remaining boundaries

Only EK is integrated and browser-validated here; no fleet rollout claim.
The backend remains the configured-account managed desktop pilot, not arbitrary
username/password/PAM selection. No Wayland desktop agent, terminal web channel,
clipboard, audio, TURN, public exposure, user auth or multi-manager routing.
The old local pilot's LAN listeners remain; they were not silently removed.
Managed broker remains a transient user service, not a complete boot installer.
ARM64 cross-build is not execution validation on Spark. Certificates still expire
after 30 days without renewal. Lossy-network recovery, gateway load and extensive
keyboard-layout coverage remain to validate.

Source integrated in the existing dirty main worktree, baseline
`50b41a66ccbab35b85067edd89dec7d6751bf3c3`; no blanket commit of unrelated changes.
